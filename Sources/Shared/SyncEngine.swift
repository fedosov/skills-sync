import CryptoKit
import Foundation

enum SyncTrigger {
    case manual
    case widget
    case delete
    case makeGlobal
    case rename
}

enum SyncEngineError: LocalizedError {
    case conflicts([SyncConflict])
    case deleteRequiresConfirmation
    case deletionBlockedProtectedPath
    case deletionOutsideAllowedRoots
    case deletionTargetMissing
    case makeGlobalRequiresConfirmation
    case makeGlobalOnlyForProject
    case makeGlobalBlockedProtectedPath
    case makeGlobalOutsideAllowedRoots
    case makeGlobalSourceMissing
    case makeGlobalTargetExists
    case renameRequiresNonEmptyTitle
    case renameRequiresExistingSource
    case renameBlockedProtectedPath
    case renameOutsideAllowedRoots
    case renameConflictTargetExists
    case renameNoOp
    case migrationFailed(skillKey: String, reason: String)

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
        case .makeGlobalRequiresConfirmation:
            return "make_global requires confirmed=true"
        case .makeGlobalOnlyForProject:
            return "make_global is only allowed for project skills"
        case .makeGlobalBlockedProtectedPath:
            return "Make global blocked for protected path"
        case .makeGlobalOutsideAllowedRoots:
            return "Make global blocked: source outside project roots"
        case .makeGlobalSourceMissing:
            return "Make global source does not exist"
        case .makeGlobalTargetExists:
            return "Make global target already exists"
        case .renameRequiresNonEmptyTitle:
            return "rename requires a non-empty title that produces a valid key"
        case .renameRequiresExistingSource:
            return "rename source does not exist"
        case .renameBlockedProtectedPath:
            return "Rename blocked for protected path"
        case .renameOutsideAllowedRoots:
            return "Rename blocked: source outside allowed roots"
        case .renameConflictTargetExists:
            return "Rename blocked: target already exists"
        case .renameNoOp:
            return "Rename is a no-op: generated key is unchanged"
        case let .migrationFailed(skillKey, reason):
            return "Migration failed for \(skillKey): \(reason)"
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

private enum MigrationRollbackOperation {
    case move(from: URL, to: URL)
    case restoreSymlink(path: URL, backup: URL)
    case restoreCanonical(path: URL, backup: URL)
}

struct SyncEngine {
    private let environment: SyncEngineEnvironment
    private let store: SyncStateStore
    private let preferencesStore: SyncPreferencesStore
    private let fileManager: FileManager
    private let protectedSegments: Set<String> = [".system"]

    init(
        environment: SyncEngineEnvironment = .current,
        store: SyncStateStore = SyncStateStore(),
        preferencesStore: SyncPreferencesStore = SyncPreferencesStore(),
        fileManager: FileManager = .default
    ) {
        self.environment = environment
        self.store = store
        self.preferencesStore = preferencesStore
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

    func makeGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        guard confirmed else {
            throw SyncEngineError.makeGlobalRequiresConfirmation
        }
        guard skill.scope == "project" else {
            throw SyncEngineError.makeGlobalOnlyForProject
        }
        let skillKey = skill.skillKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !skillKey.isEmpty else {
            throw SyncEngineError.makeGlobalOutsideAllowedRoots
        }
        guard !isProtectedSkillKey(skillKey) else {
            throw SyncEngineError.makeGlobalBlockedProtectedPath
        }

        let source = URL(fileURLWithPath: skill.canonicalSourcePath, isDirectory: true)
        if isProtectedPath(source) {
            throw SyncEngineError.makeGlobalBlockedProtectedPath
        }

        let workspaces = workspaceCandidates()
        let roots = allowedProjectRoots(workspaces: workspaces)
        let isAllowed = roots.contains { isRelativeTo(source, base: $0) }
        guard isAllowed else {
            throw SyncEngineError.makeGlobalOutsideAllowedRoots
        }

        let sourceExists = fileManager.fileExists(atPath: source.path) || source.isSymbolicLink
        guard sourceExists else {
            throw SyncEngineError.makeGlobalSourceMissing
        }

        let destination = preferredGlobalDestination(for: skillKey)
        let destinationExists = fileManager.fileExists(atPath: destination.path) || destination.isSymbolicLink
        guard !destinationExists else {
            throw SyncEngineError.makeGlobalTargetExists
        }

        try fileManager.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try fileManager.moveItem(at: source, to: destination)
        return try await runSync(trigger: .makeGlobal)
    }

    func renameSkill(skill: SkillRecord, newTitle: String) async throws -> SyncState {
        let newKey = normalizedSkillKey(from: newTitle)
        guard !newKey.isEmpty else {
            throw SyncEngineError.renameRequiresNonEmptyTitle
        }
        guard newKey != skill.skillKey else {
            throw SyncEngineError.renameNoOp
        }
        guard !isProtectedSkillKey(skill.skillKey), !isProtectedSkillKey(newKey) else {
            throw SyncEngineError.renameBlockedProtectedPath
        }

        let source = URL(fileURLWithPath: skill.canonicalSourcePath, isDirectory: true)
        if isProtectedPath(source) {
            throw SyncEngineError.renameBlockedProtectedPath
        }

        let workspaces = workspaceCandidates()
        let roots = allowedDeleteRoots(workspaces: workspaces)
        let isAllowed = roots.contains { isRelativeTo(source, base: $0) }
        guard isAllowed else {
            throw SyncEngineError.renameOutsideAllowedRoots
        }

        let sourceExists = fileManager.fileExists(atPath: source.path) || source.isSymbolicLink
        guard sourceExists else {
            throw SyncEngineError.renameRequiresExistingSource
        }

        let destination: URL
        if skill.scope == "project" {
            guard let workspace = skill.workspace?.trimmingCharacters(in: .whitespacesAndNewlines), !workspace.isEmpty else {
                throw SyncEngineError.renameOutsideAllowedRoots
            }
            let workspaceURL = URL(fileURLWithPath: workspace, isDirectory: true)
            destination = workspaceURL
                .appendingPathComponent(".claude/skills", isDirectory: true)
                .appendingPathComponent(newKey, isDirectory: true)
        } else {
            destination = preferredGlobalDestination(for: newKey)
        }

        if isProtectedPath(destination) {
            throw SyncEngineError.renameBlockedProtectedPath
        }
        if standardizedPath(source) == standardizedPath(destination) {
            throw SyncEngineError.renameNoOp
        }

        let destinationExists = fileManager.fileExists(atPath: destination.path) || destination.isSymbolicLink
        guard !destinationExists else {
            throw SyncEngineError.renameConflictTargetExists
        }

        try fileManager.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try fileManager.moveItem(at: source, to: destination)

        let skillFile = destination.appendingPathComponent("SKILL.md")
        do {
            try updateSkillTitle(at: skillFile, newTitle: newTitle.trimmingCharacters(in: .whitespacesAndNewlines))
        } catch {
            if (fileManager.fileExists(atPath: destination.path) || destination.isSymbolicLink)
                && !(fileManager.fileExists(atPath: source.path) || source.isSymbolicLink) {
                try? fileManager.moveItem(at: destination, to: source)
            }
            throw error
        }

        return try await runSync(trigger: .rename)
    }

    private func runCoreSync() throws -> SyncCoreResult {
        try ensureDirectories()
        let autoMigrateToCanonical = preferencesStore.loadSettings().autoMigrateToCanonicalSource

        var conflicts: [SyncConflict] = []
        var entries: [SkillRecord] = []

        let globalCandidates = discoverGlobalPackages()
        let globalResolution = resolveScopeCandidates(globalCandidates, scope: "global", workspace: nil)
        conflicts.append(contentsOf: globalResolution.conflicts)
        var globalCanonical = globalResolution.canonical

        let workspaces = workspaceCandidates()
        var projectCandidatesByWorkspace: [URL: [SkillPackage]] = [:]
        var projectResolvedByWorkspace: [URL: [String: SkillPackage]] = [:]
        for workspace in workspaces {
            let candidates = discoverProjectPackages(workspace: workspace)
            projectCandidatesByWorkspace[workspace] = candidates
            let resolution = resolveScopeCandidates(candidates, scope: "project", workspace: workspace)
            conflicts.append(contentsOf: resolution.conflicts)
            projectResolvedByWorkspace[workspace] = resolution.canonical
        }

        if !conflicts.isEmpty {
            throw SyncEngineError.conflicts(conflicts)
        }

        if autoMigrateToCanonical {
            let backupRoot = environment.runtimeDirectory
                .appendingPathComponent("migration-backups", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
            try fileManager.createDirectory(at: backupRoot, withIntermediateDirectories: true)
            do {
                globalCanonical = try migrateScopeCandidatesToClaude(
                    candidates: globalCandidates,
                    scope: "global",
                    workspace: nil,
                    backupRoot: backupRoot
                )

                for workspace in workspaces {
                    let candidates = projectCandidatesByWorkspace[workspace] ?? []
                    let migrated = try migrateScopeCandidatesToClaude(
                        candidates: candidates,
                        scope: "project",
                        workspace: workspace,
                        backupRoot: backupRoot
                    )
                    projectResolvedByWorkspace[workspace] = migrated
                }
                try? fileManager.removeItem(at: backupRoot)
            } catch {
                throw error
            }
        }

        let oldManagedLinks = loadManagedLinksManifest()
        var newManagedLinks: Set<String> = []

        let globalTargetRoots = globalTargets()
        for (skillKey, package) in globalCanonical.sorted(by: { $0.key < $1.key }) {
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
        result += discoverDirPackages(root: workspace.appendingPathComponent(".codex/skills", isDirectory: true), scope: "project", workspace: workspace)
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

            let rootPath = standardizedPath(root)
            let urlPath = standardizedPath(url)
            guard urlPath.hasPrefix(rootPath + "/") else {
                continue
            }
            let relative = String(urlPath.dropFirst(rootPath.count + 1))
            let keyPath = (relative as NSString).deletingLastPathComponent
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
        if standardizedPath(package.sourceRoot) == standardizedPath(workspace.appendingPathComponent(".codex/skills", isDirectory: true)) {
            return 2
        }
        return 999
    }

    private func migrateScopeCandidatesToClaude(
        candidates: [SkillPackage],
        scope: String,
        workspace: URL?,
        backupRoot: URL
    ) throws -> [String: SkillPackage] {
        var byKey: [String: [SkillPackage]] = [:]
        for candidate in candidates {
            byKey[candidate.skillKey, default: []].append(candidate)
        }

        var canonicalByKey: [String: SkillPackage] = [:]
        let canonicalRoot = scope == "global"
            ? claudeSkillsRoot()
            : workspace!.appendingPathComponent(".claude/skills", isDirectory: true)

        for (skillKey, options) in byKey.sorted(by: { $0.key < $1.key }) {
            let hashes = Set(options.map(\.packageHash))
            if hashes.count > 1 {
                throw SyncEngineError.conflicts([SyncConflict(scope: scope, workspace: workspace?.path, skillKey: skillKey)])
            }

            let desiredCanonicalPath = canonicalRoot.appendingPathComponent(skillKey, isDirectory: true)
            let currentCanonical = options.first {
                standardizedPath($0.canonicalPath) == standardizedPath(desiredCanonicalPath)
            }

            let skillBackupRoot = backupRoot.appendingPathComponent(skillKey, isDirectory: true)
            try fileManager.createDirectory(at: skillBackupRoot, withIntermediateDirectories: true)
            var rollbackOps: [MigrationRollbackOperation] = []

            do {
                let effectiveCanonical: SkillPackage
                if let currentCanonical {
                    if let replacement = preferredCanonicalReplacement(
                        options: options,
                        currentCanonical: currentCanonical,
                        scope: scope,
                        workspace: workspace
                    ) {
                        let replacedCanonicalBackup = try moveSourceToCanonical(
                            source: replacement.canonicalPath,
                            canonical: desiredCanonicalPath,
                            backupRoot: skillBackupRoot
                        )
                        if let replacedCanonicalBackup {
                            rollbackOps.append(.restoreCanonical(path: desiredCanonicalPath, backup: replacedCanonicalBackup))
                        }
                        rollbackOps.append(.move(from: replacement.canonicalPath, to: desiredCanonicalPath))
                        effectiveCanonical = SkillPackage(
                            scope: replacement.scope,
                            workspace: replacement.workspace,
                            sourceRoot: canonicalRoot,
                            skillKey: replacement.skillKey,
                            name: desiredCanonicalPath.lastPathComponent,
                            canonicalPath: desiredCanonicalPath,
                            packageType: replacement.packageType,
                            packageHash: replacement.packageHash
                        )
                    } else {
                        effectiveCanonical = currentCanonical
                    }
                } else {
                    guard let source = options.min(by: { lhs, rhs in
                        sourcePriority(scope: scope, package: lhs, workspace: workspace) < sourcePriority(scope: scope, package: rhs, workspace: workspace)
                    }) else {
                        continue
                    }
                    let replacedCanonicalBackup = try moveSourceToCanonical(
                        source: source.canonicalPath,
                        canonical: desiredCanonicalPath,
                        backupRoot: skillBackupRoot
                    )
                    if let replacedCanonicalBackup {
                        rollbackOps.append(.restoreCanonical(path: desiredCanonicalPath, backup: replacedCanonicalBackup))
                    }
                    rollbackOps.append(.move(from: source.canonicalPath, to: desiredCanonicalPath))
                    effectiveCanonical = SkillPackage(
                        scope: source.scope,
                        workspace: source.workspace,
                        sourceRoot: canonicalRoot,
                        skillKey: source.skillKey,
                        name: desiredCanonicalPath.lastPathComponent,
                        canonicalPath: desiredCanonicalPath,
                        packageType: source.packageType,
                        packageHash: source.packageHash
                    )
                }

                for option in options {
                    if standardizedPath(option.canonicalPath) == standardizedPath(desiredCanonicalPath) {
                        continue
                    }
                    if let backupPath = try replacePathWithSymlink(
                        path: option.canonicalPath,
                        destination: desiredCanonicalPath,
                        backupRoot: skillBackupRoot
                    ) {
                        rollbackOps.append(.restoreSymlink(path: option.canonicalPath, backup: backupPath))
                    }
                }

                canonicalByKey[skillKey] = effectiveCanonical
                try? fileManager.removeItem(at: skillBackupRoot)
            } catch {
                rollbackMigrationOperations(operations: rollbackOps)
                throw SyncEngineError.migrationFailed(skillKey: skillKey, reason: error.localizedDescription)
            }
        }

        return canonicalByKey
    }

    private func preferredCanonicalReplacement(
        options: [SkillPackage],
        currentCanonical: SkillPackage,
        scope: String,
        workspace: URL?
    ) -> SkillPackage? {
        guard currentCanonical.packageType == "dir" else {
            return nil
        }

        let currentIsHealthy = isHealthyCanonicalPayload(currentCanonical)
        let currentUsesSymlinkMain = isMainSkillFileSymlink(currentCanonical)
        guard !currentIsHealthy || currentUsesSymlinkMain else {
            return nil
        }

        let replacements = options.filter { candidate in
            standardizedPath(candidate.canonicalPath) != standardizedPath(currentCanonical.canonicalPath)
                && hasRegularReadableMainSkillFile(candidate)
        }

        return replacements.min { lhs, rhs in
            let lp = sourcePriority(scope: scope, package: lhs, workspace: workspace)
            let rp = sourcePriority(scope: scope, package: rhs, workspace: workspace)
            if lp != rp {
                return lp < rp
            }
            return lhs.canonicalPath.path < rhs.canonicalPath.path
        }
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

        for root in customWorkspaceDiscoveryRoots() {
            candidates.append(contentsOf: discoverWorkspaces(in: root, depth: 0, maxDepth: 3))
        }

        let byPath = Dictionary(uniqueKeysWithValues: candidates.map { (standardizedPath($0), $0) })
        return byPath.values.sorted { $0.path < $1.path }
    }

    private func hasWorkspaceSkills(_ repo: URL) -> Bool {
        let claude = repo.appendingPathComponent(".claude/skills", isDirectory: true)
        let agents = repo.appendingPathComponent(".agents/skills", isDirectory: true)
        let codex = repo.appendingPathComponent(".codex/skills", isDirectory: true)
        return fileManager.fileExists(atPath: claude.path)
            || fileManager.fileExists(atPath: agents.path)
            || fileManager.fileExists(atPath: codex.path)
    }

    private func customWorkspaceDiscoveryRoots() -> [URL] {
        let configured = preferencesStore.loadSettings().workspaceDiscoveryRoots
        var roots: [URL] = []
        var seen: Set<String> = []
        for raw in configured {
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, trimmed.hasPrefix("/") else { continue }
            let normalized = URL(fileURLWithPath: trimmed, isDirectory: true).standardizedFileURL
            let key = standardizedPath(normalized)
            guard !seen.contains(key) else { continue }
            seen.insert(key)
            roots.append(normalized)
        }
        return roots
    }

    private func discoverWorkspaces(in root: URL, depth: Int, maxDepth: Int) -> [URL] {
        guard fileManager.fileExists(atPath: root.path) else {
            return []
        }

        var result: [URL] = []
        if hasWorkspaceSkills(root) {
            result.append(root)
        }

        guard depth < maxDepth else {
            return result
        }

        let keys: [URLResourceKey] = [.isDirectoryKey, .isSymbolicLinkKey]
        guard let children = try? fileManager.contentsOfDirectory(at: root, includingPropertiesForKeys: keys, options: [.skipsHiddenFiles]) else {
            return result
        }

        for child in children {
            let values = try? child.resourceValues(forKeys: Set(keys))
            guard values?.isDirectory == true else { continue }
            if values?.isSymbolicLink == true {
                continue
            }
            result.append(contentsOf: discoverWorkspaces(in: child, depth: depth + 1, maxDepth: maxDepth))
        }

        return result
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

    private func moveSourceToCanonical(source: URL, canonical: URL, backupRoot: URL) throws -> URL? {
        guard standardizedPath(source) != standardizedPath(canonical) else {
            return nil
        }

        let replacedCanonicalBackup = try backupOccupiedCanonicalPathIfNeeded(canonical: canonical, backupRoot: backupRoot)
        do {
            try fileManager.createDirectory(at: canonical.deletingLastPathComponent(), withIntermediateDirectories: true)
            try fileManager.moveItem(at: source, to: canonical)
            return replacedCanonicalBackup
        } catch {
            if let replacedCanonicalBackup {
                try? fileManager.moveItem(at: replacedCanonicalBackup, to: canonical)
            }
            throw error
        }
    }

    private func backupOccupiedCanonicalPathIfNeeded(canonical: URL, backupRoot: URL) throws -> URL? {
        guard fileManager.fileExists(atPath: canonical.path) || canonical.isSymbolicLink else {
            return nil
        }

        if !canonical.isSymbolicLink && !isDirectory(canonical) {
            throw NSError(
                domain: "SkillsSync",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "Canonical path is occupied at \(canonical.path)"]
            )
        }

        let backupPath = backupRoot.appendingPathComponent("occupied-canonical", isDirectory: canonical.hasDirectoryPath)
        if fileManager.fileExists(atPath: backupPath.path) || backupPath.isSymbolicLink {
            try fileManager.removeItem(at: backupPath)
        }
        try fileManager.moveItem(at: canonical, to: backupPath)
        return backupPath
    }

    private func replacePathWithSymlink(path: URL, destination: URL, backupRoot: URL) throws -> URL? {
        guard standardizedPath(path) != standardizedPath(destination) else {
            return nil
        }

        if path.isSymbolicLink,
           let existingDestination = try? fileManager.destinationOfSymbolicLink(atPath: path.path),
           standardizedPath(URL(fileURLWithPath: existingDestination)) == standardizedPath(destination) {
            return nil
        }

        guard fileManager.fileExists(atPath: path.path) || path.isSymbolicLink else {
            return nil
        }

        let backupPath = backupRoot.appendingPathComponent(sha1Hex(path.path), isDirectory: path.hasDirectoryPath)
        if fileManager.fileExists(atPath: backupPath.path) || backupPath.isSymbolicLink {
            try fileManager.removeItem(at: backupPath)
        }

        try fileManager.moveItem(at: path, to: backupPath)
        do {
            try fileManager.createSymbolicLink(at: path, withDestinationURL: destination)
        } catch {
            try? fileManager.moveItem(at: backupPath, to: path)
            throw error
        }
        return backupPath
    }

    private func rollbackMigrationOperations(operations: [MigrationRollbackOperation]) {
        for operation in operations.reversed() {
            switch operation {
            case let .move(from, to):
                if fileManager.fileExists(atPath: to.path) || to.isSymbolicLink {
                    try? fileManager.moveItem(at: to, to: from)
                }
            case let .restoreSymlink(path, backup):
                if fileManager.fileExists(atPath: path.path) || path.isSymbolicLink {
                    try? fileManager.removeItem(at: path)
                }
                if fileManager.fileExists(atPath: backup.path) || backup.isSymbolicLink {
                    try? fileManager.moveItem(at: backup, to: path)
                }
            case let .restoreCanonical(path, backup):
                if fileManager.fileExists(atPath: path.path) || path.isSymbolicLink {
                    try? fileManager.removeItem(at: path)
                }
                if fileManager.fileExists(atPath: backup.path) || backup.isSymbolicLink {
                    try? fileManager.moveItem(at: backup, to: path)
                }
            }
        }
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

    private func normalizedSkillKey(from title: String) -> String {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else {
            return ""
        }

        var result = ""
        var previousWasDash = false
        for scalar in trimmed.unicodeScalars {
            let value = scalar.value
            let isDigit = (48...57).contains(value)
            let isLowercaseLatin = (97...122).contains(value)
            if isDigit || isLowercaseLatin {
                result.append(Character(scalar))
                previousWasDash = false
            } else if !previousWasDash {
                result.append("-")
                previousWasDash = true
            }
        }

        return result.trimmingCharacters(in: CharacterSet(charactersIn: "-"))
    }

    private func updateSkillTitle(at skillFile: URL, newTitle: String) throws {
        let contents = try String(contentsOf: skillFile, encoding: .utf8)
        let updated = updatedSkillContents(contents, withTitle: newTitle)
        try updated.write(to: skillFile, atomically: true, encoding: .utf8)
    }

    private func updatedSkillContents(_ original: String, withTitle title: String) -> String {
        let normalized = original.replacingOccurrences(of: "\r\n", with: "\n")
        if normalized.hasPrefix("---\n"),
           let fmEnd = normalized.range(of: "\n---", range: normalized.index(normalized.startIndex, offsetBy: 4)..<normalized.endIndex) {
            let fmStart = normalized.index(normalized.startIndex, offsetBy: 4)
            let fmRaw = String(normalized[fmStart..<fmEnd.lowerBound])
            var lines = fmRaw.components(separatedBy: "\n")
            var replaced = false
            for index in lines.indices {
                let key = lines[index]
                    .split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
                    .first?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                    .lowercased()
                if key == "title" {
                    lines[index] = "title: \(title)"
                    replaced = true
                    break
                }
            }
            if !replaced {
                lines.append("title: \(title)")
            }
            let body = String(normalized[fmEnd.upperBound...])
            return "---\n\(lines.joined(separator: "\n"))\n---\(body)"
        }

        return """
        ---
        title: \(title)
        ---

        \(normalized)
        """
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
            roots.append(workspace.appendingPathComponent(".codex/skills", isDirectory: true))
        }
        return roots
    }

    private func allowedProjectRoots(workspaces: [URL]) -> [URL] {
        var roots: [URL] = []
        for workspace in workspaces {
            roots.append(workspace.appendingPathComponent(".claude/skills", isDirectory: true))
            roots.append(workspace.appendingPathComponent(".agents/skills", isDirectory: true))
            roots.append(workspace.appendingPathComponent(".codex/skills", isDirectory: true))
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

    private func preferredGlobalRoot() -> URL {
        claudeSkillsRoot()
    }

    private func preferredGlobalDestination(for skillKey: String) -> URL {
        preferredGlobalRoot().appendingPathComponent(skillKey, isDirectory: true)
    }

    private func globalTargets() -> [URL] {
        [claudeSkillsRoot(), agentsSkillsRoot(), codexSkillsRoot()]
    }

    private func projectTargets(for workspace: URL) -> [URL] {
        [
            workspace.appendingPathComponent(".claude/skills", isDirectory: true),
            workspace.appendingPathComponent(".agents/skills", isDirectory: true),
            workspace.appendingPathComponent(".codex/skills", isDirectory: true)
        ]
    }

    private func hashDirectory(_ directory: URL) -> String? {
        guard let enumerator = fileManager.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey],
            options: [.skipsHiddenFiles]
        ) else {
            return nil
        }

        let files = (enumerator.allObjects as? [URL] ?? [])
            .filter { file in
                guard let values = try? file.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey]) else {
                    return false
                }
                return values.isRegularFile == true || values.isSymbolicLink == true
            }
            .sorted { $0.path < $1.path }

        var digest = SHA256()
        if files.isEmpty {
            digest.update(data: Data("<empty>".utf8))
        } else {
            for file in files {
                let relative = file.path.replacingOccurrences(of: directory.path + "/", with: "")
                digest.update(data: Data(relative.utf8))
                digest.update(data: Data([0]))
                if file.isSymbolicLink {
                    if let resolved = resolvedSymlinkDestination(for: file),
                       fileManager.fileExists(atPath: resolved.path),
                       !isDirectory(resolved),
                       let bytes = try? Data(contentsOf: resolved) {
                        digest.update(data: bytes)
                    } else {
                        digest.update(data: Data("<broken-symlink>".utf8))
                    }
                } else if let bytes = try? Data(contentsOf: file) {
                    digest.update(data: bytes)
                }
                digest.update(data: Data([0]))
            }
        }
        let hash = digest.finalize()
        return hash.map { String(format: "%02x", $0) }.joined()
    }

    private func mainSkillFileURL(for package: SkillPackage) -> URL {
        if package.packageType == "dir" {
            return package.canonicalPath.appendingPathComponent("SKILL.md")
        }
        return package.canonicalPath
    }

    private func isHealthyCanonicalPayload(_ package: SkillPackage) -> Bool {
        let mainFile = mainSkillFileURL(for: package)
        guard fileManager.fileExists(atPath: mainFile.path), !isDirectory(mainFile) else {
            return false
        }
        if mainFile.isSymbolicLink && isBrokenSymlink(mainFile) {
            return false
        }
        guard (try? String(contentsOf: mainFile, encoding: .utf8)) != nil else {
            return false
        }
        return true
    }

    private func hasRegularReadableMainSkillFile(_ package: SkillPackage) -> Bool {
        let mainFile = mainSkillFileURL(for: package)
        guard fileManager.fileExists(atPath: mainFile.path),
              !isDirectory(mainFile),
              !mainFile.isSymbolicLink else {
            return false
        }
        return (try? String(contentsOf: mainFile, encoding: .utf8)) != nil
    }

    private func isMainSkillFileSymlink(_ package: SkillPackage) -> Bool {
        mainSkillFileURL(for: package).isSymbolicLink
    }

    private func isBrokenSymlink(_ url: URL) -> Bool {
        guard url.isSymbolicLink else {
            return false
        }
        guard let destination = resolvedSymlinkDestination(for: url) else {
            return true
        }
        return !fileManager.fileExists(atPath: destination.path)
    }

    private func resolvedSymlinkDestination(for url: URL) -> URL? {
        guard let raw = try? fileManager.destinationOfSymbolicLink(atPath: url.path) else {
            return nil
        }
        if raw.hasPrefix("/") {
            return URL(fileURLWithPath: raw)
        }
        return url.deletingLastPathComponent().appendingPathComponent(raw).standardizedFileURL
    }

    private func isDirectory(_ url: URL) -> Bool {
        var isDir: ObjCBool = false
        return fileManager.fileExists(atPath: url.path, isDirectory: &isDir) && isDir.boolValue
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
