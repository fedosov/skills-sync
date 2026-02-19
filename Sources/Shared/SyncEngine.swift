import CryptoKit
import Foundation

enum SyncTrigger {
    case manual
    case widget
    case delete
}

enum SyncEngineError: LocalizedError {
    case conflicts([SyncConflict])
    case deleteRequiresConfirmation
    case deletionBlockedProtectedPath
    case deletionOutsideAllowedRoots
    case deletionTargetMissing

    var errorDescription: String? {
        switch self {
        case let .conflicts(items):
            return "Detected \(items.count) skill conflict(s)"
        case .deleteRequiresConfirmation:
            return "delete_canonical_source requires confirmed=true"
        case .deletionBlockedProtectedPath:
            return "Deletion blocked for protected path"
        case .deletionOutsideAllowedRoots:
            return "Deletion blocked: target outside allowed roots"
        case .deletionTargetMissing:
            return "Deletion target does not exist"
        }
    }
}

struct SyncConflict: Equatable {
    let scope: String
    let workspace: String?
    let skillKey: String
}

protocol SyncShellRunning {
    func run(_ command: [String]) throws
}

struct DefaultSyncShellRunner: SyncShellRunning {
    func run(_ command: [String]) throws {
        guard let executable = command.first else {
            return
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [executable] + command.dropFirst()
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            throw NSError(
                domain: "SkillsSync",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: "Command failed: \(command.joined(separator: " "))"]
            )
        }
    }
}

struct SyncEngineEnvironment {
    var homeDirectory: URL
    var devRoot: URL
    var worktreesRoot: URL
    var runtimeDirectory: URL
    var shellRunner: SyncShellRunning

    nonisolated(unsafe) static var testOverride: SyncEngineEnvironment?

    static var current: SyncEngineEnvironment {
        if let testOverride {
            return testOverride
        }
        let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        return SyncEngineEnvironment(
            homeDirectory: home,
            devRoot: home.appendingPathComponent("Dev", isDirectory: true),
            worktreesRoot: home.appendingPathComponent(".codex/worktrees", isDirectory: true),
            runtimeDirectory: home.appendingPathComponent(".config/ai-agents/skillssync", isDirectory: true),
            shellRunner: DefaultSyncShellRunner()
        )
    }
}

private struct SkillPackage {
    let scope: String
    let workspace: URL?
    let sourceRoot: URL
    let skillKey: String
    let name: String
    let canonicalPath: URL
    let packageType: String
    let packageHash: String
}

private struct SyncCoreResult {
    let entries: [SkillRecord]
    let conflictCount: Int
}

struct SyncEngine {
    private let environment: SyncEngineEnvironment
    private let store: SyncStateStore
    private let fileManager: FileManager
    private let protectedSegments: Set<String> = [".system"]

    init(
        environment: SyncEngineEnvironment = .current,
        store: SyncStateStore = SyncStateStore(),
        fileManager: FileManager = .default
    ) {
        self.environment = environment
        self.store = store
        self.fileManager = fileManager
    }

    func runSync(trigger: SyncTrigger) async throws -> SyncState {
        let startedAt = Date()
        let previousState = store.loadState()

        do {
            let result = try runCoreSync()
            let finishedAt = Date()
            let state = makeState(
                status: .ok,
                entries: result.entries,
                conflictCount: result.conflictCount,
                startedAt: startedAt,
                finishedAt: finishedAt,
                error: nil
            )
            try store.saveState(state)
            return state
        } catch let error as SyncEngineError {
            let finishedAt = Date()
            let conflictCount: Int
            if case let .conflicts(conflicts) = error {
                conflictCount = conflicts.count
            } else {
                conflictCount = 0
            }
            let failed = makeFailedState(
                previous: previousState,
                startedAt: startedAt,
                finishedAt: finishedAt,
                error: error.localizedDescription,
                conflictCount: conflictCount
            )
            try? store.saveState(failed)
            throw error
        } catch {
            let finishedAt = Date()
            let failed = makeFailedState(
                previous: previousState,
                startedAt: startedAt,
                finishedAt: finishedAt,
                error: "Unexpected sync error: \(error.localizedDescription)",
                conflictCount: 0
            )
            try? store.saveState(failed)
            throw error
        }
    }

    func openInZed(skill: SkillRecord) throws {
        try environment.shellRunner.run(["open", "-a", "Zed", skill.canonicalSourcePath])
    }

    func revealInFinder(skill: SkillRecord) throws {
        try environment.shellRunner.run(["open", "-R", skill.canonicalSourcePath])
    }

    func deleteCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        guard confirmed else {
            throw SyncEngineError.deleteRequiresConfirmation
        }

        let target = URL(fileURLWithPath: skill.canonicalSourcePath, isDirectory: true)
        if isProtectedPath(target) {
            throw SyncEngineError.deletionBlockedProtectedPath
        }

        let roots = allowedDeleteRoots(workspaces: workspaceCandidates())
        let isAllowed = roots.contains { isRelativeTo(target, base: $0) }
        guard isAllowed else {
            throw SyncEngineError.deletionOutsideAllowedRoots
        }

        let exists = fileManager.fileExists(atPath: target.path) || (try? target.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
        guard exists else {
            throw SyncEngineError.deletionTargetMissing
        }

        _ = try moveToTrash(target)
        return try await runSync(trigger: .delete)
    }

    private func runCoreSync() throws -> SyncCoreResult {
        try ensureDirectories()

        var conflicts: [SyncConflict] = []
        var entries: [SkillRecord] = []

        let globalCandidates = discoverGlobalPackages()
        let globalResolution = resolveScopeCandidates(globalCandidates, scope: "global", workspace: nil)
        conflicts.append(contentsOf: globalResolution.conflicts)

        let workspaces = workspaceCandidates()
        var projectResolvedByWorkspace: [URL: [String: SkillPackage]] = [:]
        for workspace in workspaces {
            let candidates = discoverProjectPackages(workspace: workspace)
            let resolution = resolveScopeCandidates(candidates, scope: "project", workspace: workspace)
            conflicts.append(contentsOf: resolution.conflicts)
            projectResolvedByWorkspace[workspace] = resolution.canonical
        }

        if !conflicts.isEmpty {
            throw SyncEngineError.conflicts(conflicts)
        }

        let oldManagedLinks = loadManagedLinksManifest()
        var newManagedLinks: Set<String> = []

        let globalTargetRoots = globalTargets()
        for (skillKey, package) in globalResolution.canonical.sorted(by: { $0.key < $1.key }) {
            var targetPaths: [String] = []
            for targetRoot in globalTargetRoots {
                let target = targetRoot.appendingPathComponent(skillKey, isDirectory: package.packageType == "dir")
                if standardizedPath(target) == standardizedPath(package.canonicalPath) {
                    targetPaths.append(target.path)
                    continue
                }
                try createOrUpdateSymlink(at: target, destination: package.canonicalPath)
                newManagedLinks.insert(standardizedPath(target))
                targetPaths.append(target.path)
            }
            entries.append(createSkillEntry(scope: "global", workspace: nil, skillKey: skillKey, package: package, targetPaths: targetPaths))
        }

        for workspace in workspaces.sorted(by: { $0.path < $1.path }) {
            guard let canonical = projectResolvedByWorkspace[workspace] else { continue }
            let targetRoots = projectTargets(for: workspace)
            for (skillKey, package) in canonical.sorted(by: { $0.key < $1.key }) {
                var targetPaths: [String] = []
                for targetRoot in targetRoots {
                    let target = targetRoot.appendingPathComponent(skillKey, isDirectory: package.packageType == "dir")
                    if standardizedPath(target) == standardizedPath(package.canonicalPath) {
                        targetPaths.append(target.path)
                        continue
                    }
                    try createOrUpdateSymlink(at: target, destination: package.canonicalPath)
                    newManagedLinks.insert(standardizedPath(target))
                    targetPaths.append(target.path)
                }

                entries.append(
                    createSkillEntry(
                        scope: "project",
                        workspace: workspace.path,
                        skillKey: skillKey,
                        package: package,
                        targetPaths: targetPaths
                    )
                )
            }
        }

        cleanupStaleLinks(oldManagedLinks: oldManagedLinks, newManagedLinks: newManagedLinks)
        saveManagedLinksManifest(newManagedLinks)

        return SyncCoreResult(entries: entries.sorted(by: sortEntries), conflictCount: 0)
    }

    private func createSkillEntry(
        scope: String,
        workspace: String?,
        skillKey: String,
        package: SkillPackage,
        targetPaths: [String]
    ) -> SkillRecord {
        SkillRecord(
            id: skillEntryID(scope: scope, workspace: workspace, skillKey: skillKey),
            name: package.name,
            scope: scope,
            workspace: workspace,
            canonicalSourcePath: package.canonicalPath.path,
            targetPaths: targetPaths.sorted(),
            exists: fileManager.fileExists(atPath: package.canonicalPath.path) || package.canonicalPath.isSymbolicLink,
            isSymlinkCanonical: package.canonicalPath.isSymbolicLink,
            packageType: package.packageType,
            skillKey: skillKey,
            symlinkTarget: package.canonicalPath.path
        )
    }

    private func makeState(
        status: SyncHealthStatus,
        entries: [SkillRecord],
        conflictCount: Int,
        startedAt: Date,
        finishedAt: Date,
        error: String?
    ) -> SyncState {
        let topSkillIDs = entries
            .sorted(by: sortEntries)
            .prefix(6)
            .map(\.id)

        return SyncState(
            version: 1,
            generatedAt: iso8601(finishedAt),
            sync: SyncMetadata(
                status: status,
                lastStartedAt: iso8601(startedAt),
                lastFinishedAt: iso8601(finishedAt),
                durationMs: Int(finishedAt.timeIntervalSince(startedAt) * 1000),
                error: error
            ),
            summary: SyncSummary(
                globalCount: entries.filter { $0.scope == "global" }.count,
                projectCount: entries.filter { $0.scope == "project" }.count,
                conflictCount: conflictCount
            ),
            skills: entries,
            topSkills: topSkillIDs
        )
    }

    private func makeFailedState(
        previous: SyncState,
        startedAt: Date,
        finishedAt: Date,
        error: String,
        conflictCount: Int
    ) -> SyncState {
        SyncState(
            version: 1,
            generatedAt: iso8601(finishedAt),
            sync: SyncMetadata(
                status: .failed,
                lastStartedAt: iso8601(startedAt),
                lastFinishedAt: iso8601(finishedAt),
                durationMs: Int(finishedAt.timeIntervalSince(startedAt) * 1000),
                error: error
            ),
            summary: SyncSummary(
                globalCount: previous.summary.globalCount,
                projectCount: previous.summary.projectCount,
                conflictCount: conflictCount
            ),
            skills: previous.skills,
            topSkills: previous.topSkills
        )
    }

    private func discoverGlobalPackages() -> [SkillPackage] {
        var result: [SkillPackage] = []
        result += discoverDirPackages(root: claudeSkillsRoot(), scope: "global", workspace: nil)
        result += discoverDirPackages(root: agentsSkillsRoot(), scope: "global", workspace: nil)
        result += discoverDirPackages(root: codexSkillsRoot(), scope: "global", workspace: nil)
        return result
    }

    private func discoverProjectPackages(workspace: URL) -> [SkillPackage] {
        var result: [SkillPackage] = []
        result += discoverDirPackages(root: workspace.appendingPathComponent(".claude/skills", isDirectory: true), scope: "project", workspace: workspace)
        result += discoverDirPackages(root: workspace.appendingPathComponent(".agents/skills", isDirectory: true), scope: "project", workspace: workspace)
        return result
    }

    private func discoverDirPackages(root: URL, scope: String, workspace: URL?) -> [SkillPackage] {
        guard fileManager.fileExists(atPath: root.path) else {
            return []
        }
        guard let enumerator = fileManager.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles]) else {
            return []
        }

        var seen: Set<String> = []
        var packages: [SkillPackage] = []
        for case let url as URL in enumerator {
            guard url.lastPathComponent == "SKILL.md" else { continue }

            let relative = url.path.replacingOccurrences(of: root.path + "/", with: "")
            let keyPath = URL(fileURLWithPath: relative).deletingLastPathComponent().path
            let skillKey = keyPath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            if skillKey.isEmpty || isProtectedSkillKey(skillKey) || seen.contains(skillKey) {
                continue
            }
            seen.insert(skillKey)

            let canonicalPath = url.deletingLastPathComponent()
            guard let packageHash = hashDirectory(canonicalPath) else {
                continue
            }
            packages.append(
                SkillPackage(
                    scope: scope,
                    workspace: workspace,
                    sourceRoot: root,
                    skillKey: skillKey,
                    name: canonicalPath.lastPathComponent,
                    canonicalPath: canonicalPath,
                    packageType: "dir",
                    packageHash: packageHash
                )
            )
        }

        return packages
    }

    private func resolveScopeCandidates(
        _ packages: [SkillPackage],
        scope: String,
        workspace: URL?
    ) -> (canonical: [String: SkillPackage], conflicts: [SyncConflict]) {
        var byKey: [String: [SkillPackage]] = [:]
        for package in packages {
            byKey[package.skillKey, default: []].append(package)
        }

        var canonical: [String: SkillPackage] = [:]
        var conflicts: [SyncConflict] = []
        for (skillKey, candidates) in byKey {
            let hashes = Set(candidates.map(\.packageHash))
            if hashes.count > 1 {
                conflicts.append(SyncConflict(scope: scope, workspace: workspace?.path, skillKey: skillKey))
                continue
            }
            canonical[skillKey] = candidates.min { lhs, rhs in
                let lp = sourcePriority(scope: scope, package: lhs, workspace: workspace)
                let rp = sourcePriority(scope: scope, package: rhs, workspace: workspace)
                if lp != rp { return lp < rp }
                if lhs.sourceRoot.path != rhs.sourceRoot.path { return lhs.sourceRoot.path < rhs.sourceRoot.path }
                return lhs.canonicalPath.path < rhs.canonicalPath.path
            }
        }

        return (canonical, conflicts)
    }

    private func sourcePriority(scope: String, package: SkillPackage, workspace: URL?) -> Int {
        if scope == "global" {
            let orderedRoots = [claudeSkillsRoot(), agentsSkillsRoot(), codexSkillsRoot()]
            for (index, root) in orderedRoots.enumerated() where standardizedPath(root) == standardizedPath(package.sourceRoot) {
                return index
            }
            return 999
        }

        guard let workspace else {
            return 999
        }
        if standardizedPath(package.sourceRoot) == standardizedPath(workspace.appendingPathComponent(".claude/skills", isDirectory: true)) {
            return 0
        }
        if standardizedPath(package.sourceRoot) == standardizedPath(workspace.appendingPathComponent(".agents/skills", isDirectory: true)) {
            return 1
        }
        return 999
    }

    private func workspaceCandidates() -> [URL] {
        var candidates: [URL] = []

        if let devRepos = try? fileManager.contentsOfDirectory(at: environment.devRoot, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            for repo in devRepos where hasWorkspaceSkills(repo) {
                candidates.append(repo)
            }
        }

        if let ownerDirs = try? fileManager.contentsOfDirectory(at: environment.worktreesRoot, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            for owner in ownerDirs {
                guard let repos = try? fileManager.contentsOfDirectory(at: owner, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) else {
                    continue
                }
                for repo in repos where hasWorkspaceSkills(repo) {
                    candidates.append(repo)
                }
            }
        }

        let byPath = Dictionary(uniqueKeysWithValues: candidates.map { (standardizedPath($0), $0) })
        return byPath.values.sorted { $0.path < $1.path }
    }

    private func hasWorkspaceSkills(_ repo: URL) -> Bool {
        let claude = repo.appendingPathComponent(".claude/skills", isDirectory: true)
        let agents = repo.appendingPathComponent(".agents/skills", isDirectory: true)
        return fileManager.fileExists(atPath: claude.path) || fileManager.fileExists(atPath: agents.path)
    }

    private func createOrUpdateSymlink(at target: URL, destination: URL) throws {
        if fileManager.fileExists(atPath: target.path) || target.isSymbolicLink {
            if target.isSymbolicLink,
               let existingDestination = try? fileManager.destinationOfSymbolicLink(atPath: target.path),
               standardizedPath(URL(fileURLWithPath: existingDestination)) == standardizedPath(destination) {
                return
            }
            try fileManager.removeItem(at: target)
        }

        let parent = target.deletingLastPathComponent()
        try fileManager.createDirectory(at: parent, withIntermediateDirectories: true)
        try fileManager.createSymbolicLink(at: target, withDestinationURL: destination)
    }

    private func ensureDirectories() throws {
        let dirs = [
            claudeSkillsRoot(),
            agentsSkillsRoot(),
            codexSkillsRoot(),
            runtimeSkillsRoot(),
            runtimePromptsRoot(),
            environment.runtimeDirectory
        ]
        for dir in dirs {
            try fileManager.createDirectory(at: dir, withIntermediateDirectories: true)
        }
    }

    private func cleanupStaleLinks(oldManagedLinks: Set<String>, newManagedLinks: Set<String>) {
        for stale in oldManagedLinks.subtracting(newManagedLinks) {
            let url = URL(fileURLWithPath: stale)
            if url.isSymbolicLink {
                try? fileManager.removeItem(at: url)
            }
        }
    }

    private func loadManagedLinksManifest() -> Set<String> {
        let manifestURL = environment.runtimeDirectory.appendingPathComponent(".skill-sync-manifest.json")
        guard let data = try? Data(contentsOf: manifestURL),
              let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let links = payload["managed_links"] as? [String] else {
            return []
        }
        return Set(links.map { standardizedPath(URL(fileURLWithPath: $0)) })
    }

    private func saveManagedLinksManifest(_ managedLinks: Set<String>) {
        let manifestURL = environment.runtimeDirectory.appendingPathComponent(".skill-sync-manifest.json")
        let payload: [String: Any] = [
            "version": 1,
            "generated_at": iso8601(Date()),
            "managed_links": Array(managedLinks).sorted()
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted, .sortedKeys]) else {
            return
        }
        try? data.write(to: manifestURL, options: [.atomic])
    }

    private func moveToTrash(_ path: URL) throws -> URL {
        let trash = environment.homeDirectory.appendingPathComponent(".Trash", isDirectory: true)
        try fileManager.createDirectory(at: trash, withIntermediateDirectories: true)

        var destination = trash.appendingPathComponent(path.lastPathComponent, isDirectory: path.hasDirectoryPath)
        var index = 1
        while fileManager.fileExists(atPath: destination.path) || destination.isSymbolicLink {
            destination = trash.appendingPathComponent("\(path.lastPathComponent).\(index)", isDirectory: path.hasDirectoryPath)
            index += 1
        }

        try fileManager.moveItem(at: path, to: destination)
        return destination
    }

    private func isProtectedPath(_ path: URL) -> Bool {
        let components = Set(path.standardized.pathComponents)
        return !protectedSegments.isDisjoint(with: components)
    }

    private func isProtectedSkillKey(_ key: String) -> Bool {
        let segments = Set(key.split(separator: "/").map(String.init))
        return !protectedSegments.isDisjoint(with: segments)
    }

    private func isRelativeTo(_ path: URL, base: URL) -> Bool {
        let basePath = standardizedPath(base)
        let candidate = standardizedPath(path)
        return candidate == basePath || candidate.hasPrefix(basePath + "/")
    }

    private func skillEntryID(scope: String, workspace: String?, skillKey: String) -> String {
        let workspaceValue = workspace ?? "global"
        let digest = sha1Hex("\(scope)|\(workspaceValue)|\(skillKey)").prefix(12)
        return "skill-\(digest)"
    }

    private func allowedDeleteRoots(workspaces: [URL]) -> [URL] {
        var roots = [
            claudeSkillsRoot(),
            agentsSkillsRoot(),
            codexSkillsRoot()
        ]
        for workspace in workspaces {
            roots.append(workspace.appendingPathComponent(".claude/skills", isDirectory: true))
            roots.append(workspace.appendingPathComponent(".agents/skills", isDirectory: true))
        }
        return roots
    }

    private func sortEntries(lhs: SkillRecord, rhs: SkillRecord) -> Bool {
        if lhs.scope != rhs.scope {
            return lhs.scope == "global"
        }
        if lhs.name != rhs.name {
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
        return (lhs.workspace ?? "") < (rhs.workspace ?? "")
    }

    private func claudeSkillsRoot() -> URL {
        environment.homeDirectory.appendingPathComponent(".claude/skills", isDirectory: true)
    }

    private func agentsSkillsRoot() -> URL {
        environment.homeDirectory.appendingPathComponent(".agents/skills", isDirectory: true)
    }

    private func codexSkillsRoot() -> URL {
        environment.homeDirectory.appendingPathComponent(".codex/skills", isDirectory: true)
    }

    private func runtimeSkillsRoot() -> URL {
        environment.homeDirectory.appendingPathComponent(".config/ai-agents/skillssync", isDirectory: true)
    }

    private func runtimePromptsRoot() -> URL {
        environment.homeDirectory.appendingPathComponent(".config/ai-agents/prompts", isDirectory: true)
    }

    private func globalTargets() -> [URL] {
        [claudeSkillsRoot(), agentsSkillsRoot(), codexSkillsRoot()]
    }

    private func projectTargets(for workspace: URL) -> [URL] {
        [
            workspace.appendingPathComponent(".claude/skills", isDirectory: true),
            workspace.appendingPathComponent(".agents/skills", isDirectory: true)
        ]
    }

    private func hashDirectory(_ directory: URL) -> String? {
        guard let enumerator = fileManager.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return nil
        }

        let files = (enumerator.allObjects as? [URL] ?? [])
            .filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
            .sorted { $0.path < $1.path }

        var digest = SHA256()
        if files.isEmpty {
            digest.update(data: Data("<empty>".utf8))
        } else {
            for file in files {
                let relative = file.path.replacingOccurrences(of: directory.path + "/", with: "")
                digest.update(data: Data(relative.utf8))
                digest.update(data: Data([0]))
                if let bytes = try? Data(contentsOf: file) {
                    digest.update(data: bytes)
                }
                digest.update(data: Data([0]))
            }
        }
        let hash = digest.finalize()
        return hash.map { String(format: "%02x", $0) }.joined()
    }

    private func standardizedPath(_ url: URL) -> String {
        url.standardizedFileURL.path
    }

    private func iso8601(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: date)
    }

    private func sha1Hex(_ value: String) -> String {
        let digest = Insecure.SHA1.hash(data: Data(value.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

private extension URL {
    var isSymbolicLink: Bool {
        (try? resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
    }
}
