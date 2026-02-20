import Foundation

enum SidebarSkillGroupKind: Hashable {
    case global
    case project(name: String)
    case unknownProject
}

struct SidebarSkillGroup: Identifiable, Hashable {
    let id: String
    let title: String
    let skills: [SkillRecord]
    let kind: SidebarSkillGroupKind
}

protocol SyncEngineControlling {
    func runSync(trigger: SyncTrigger) async throws -> SyncState
    func openInZed(skill: SkillRecord) throws
    func revealInFinder(skill: SkillRecord) throws
    func deleteCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState
    func makeGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState
}

extension SyncEngine: SyncEngineControlling { }

@MainActor
final class AppViewModel: ObservableObject {
    @Published var state: SyncState = .empty
    @Published var searchText: String = ""
    @Published var scopeFilter: ScopeFilter = .all
    @Published var selectedSkillIDs: Set<String> = []
    @Published var alertMessage: String?
    @Published var localBanner: InlineBannerPresentation?
    @Published var autoMigrateToCanonicalSource: Bool = false {
        didSet {
            guard isPreferencesLoaded else { return }
            preferencesStore.saveSettings(
                SyncAppSettings(version: 1, autoMigrateToCanonicalSource: autoMigrateToCanonicalSource)
            )
        }
    }

    private let store: SyncStateStore
    private let preferencesStore: SyncPreferencesStore
    private let makeEngine: () -> any SyncEngineControlling
    private var timer: Timer?
    private var isPreferencesLoaded = false

    var selectedSkills: [SkillRecord] {
        state.skills.filter { selectedSkillIDs.contains($0.id) }
    }

    var singleSelectedSkill: SkillRecord? {
        guard selectedSkillIDs.count == 1 else {
            return nil
        }
        return selectedSkills.first
    }

    init(
        store: SyncStateStore = SyncStateStore(),
        preferencesStore: SyncPreferencesStore = SyncPreferencesStore(),
        makeEngine: @escaping () -> any SyncEngineControlling = { SyncEngine() }
    ) {
        self.store = store
        self.preferencesStore = preferencesStore
        self.makeEngine = makeEngine
        autoMigrateToCanonicalSource = preferencesStore.loadSettings().autoMigrateToCanonicalSource
        isPreferencesLoaded = true
    }

    var filteredSkills: [SkillRecord] {
        Self.applyFilters(to: state.skills, query: searchText, scopeFilter: scopeFilter)
    }

    nonisolated static func sidebarGroups(from skills: [SkillRecord]) -> [SidebarSkillGroup] {
        var globalSkills: [SkillRecord] = []
        var projectSkillsByName: [String: [SkillRecord]] = [:]
        var unknownProjectSkills: [SkillRecord] = []

        for skill in skills {
            if skill.scope == "project" {
                if let projectName = projectName(from: skill.workspace) {
                    projectSkillsByName[projectName, default: []].append(skill)
                } else {
                    unknownProjectSkills.append(skill)
                }
            } else {
                globalSkills.append(skill)
            }
        }

        var groups: [SidebarSkillGroup] = []
        if !globalSkills.isEmpty {
            let sorted = sortSkillsForSidebar(globalSkills)
            groups.append(
                SidebarSkillGroup(
                    id: "global",
                    title: "Global Skills (\(sorted.count))",
                    skills: sorted,
                    kind: .global
                )
            )
        }

        let sortedProjectNames = projectSkillsByName.keys.sorted {
            $0.localizedCaseInsensitiveCompare($1) == .orderedAscending
        }
        for projectName in sortedProjectNames {
            let sorted = sortSkillsForSidebar(projectSkillsByName[projectName] ?? [])
            groups.append(
                SidebarSkillGroup(
                    id: "project:\(projectName)",
                    title: "\(projectName) (\(sorted.count))",
                    skills: sorted,
                    kind: .project(name: projectName)
                )
            )
        }

        if !unknownProjectSkills.isEmpty {
            let sorted = sortSkillsForSidebar(unknownProjectSkills)
            groups.append(
                SidebarSkillGroup(
                    id: "project:unknown",
                    title: "Unknown Project (\(sorted.count))",
                    skills: sorted,
                    kind: .unknownProject
                )
            )
        }

        return groups
    }

    nonisolated static func applyFilters(to skills: [SkillRecord], query: String, scopeFilter: ScopeFilter) -> [SkillRecord] {
        let base = skills.sorted { lhs, rhs in
            if lhs.scope != rhs.scope {
                return lhs.scope == "global"
            }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }

        let scoped = base.filter(scopeFilter.includes)
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            return scoped
        }

        return scoped.filter { skill in
            skill.name.localizedCaseInsensitiveContains(trimmedQuery)
                || skill.scope.localizedCaseInsensitiveContains(trimmedQuery)
                || (skill.workspace?.localizedCaseInsensitiveContains(trimmedQuery) ?? false)
                || skill.canonicalSourcePath.localizedCaseInsensitiveContains(trimmedQuery)
        }
    }

    nonisolated private static func projectName(from workspace: String?) -> String? {
        guard let workspace else { return nil }
        let trimmed = workspace.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        let lastPathComponent = URL(fileURLWithPath: trimmed).lastPathComponent
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !lastPathComponent.isEmpty, lastPathComponent != "/" else {
            return nil
        }
        return lastPathComponent
    }

    nonisolated private static func sortSkillsForSidebar(_ skills: [SkillRecord]) -> [SkillRecord] {
        skills.sorted { lhs, rhs in
            let nameOrder = lhs.name.localizedCaseInsensitiveCompare(rhs.name)
            if nameOrder != .orderedSame {
                return nameOrder == .orderedAscending
            }
            return lhs.canonicalSourcePath.localizedCaseInsensitiveCompare(rhs.canonicalSourcePath) == .orderedAscending
        }
    }

    func start() {
        load()
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.load()
            }
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    func load() {
        state = store.loadState()
        pruneSelectionToCurrentSkills()
    }

    func pruneSelectionToCurrentSkills() {
        let validIDs = Set(state.skills.map(\.id))
        selectedSkillIDs = selectedSkillIDs.intersection(validIDs)
    }

    func refreshSources() {
        load()
        let sourceCount = state.skills.filter { $0.scope == "global" }.count
        localBanner = InlineBannerPresentation(
            title: "Sources refreshed",
            message: "Loaded \(sourceCount) source skills.",
            symbol: "arrow.clockwise.circle.fill",
            role: .info,
            recoveryActionTitle: nil
        )
    }

    func syncNow() {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.runSync(trigger: .manual)
                localBanner = InlineBannerPresentation(
                    title: "Sync completed",
                    message: "Skills were synchronized successfully.",
                    symbol: "checkmark.circle.fill",
                    role: .success,
                    recoveryActionTitle: nil
                )
            } catch {
                load()
                alertMessage = error.localizedDescription
            }
        }
    }

    func open(skill: SkillRecord) {
        do {
            let engine = makeEngine()
            try engine.openInZed(skill: skill)
            localBanner = InlineBannerPresentation(
                title: "Opened in Zed",
                message: "\(skill.name) was opened in Zed.",
                symbol: "checkmark.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        } catch {
            alertMessage = error.localizedDescription
        }
    }

    func reveal(skill: SkillRecord) {
        do {
            let engine = makeEngine()
            try engine.revealInFinder(skill: skill)
            localBanner = InlineBannerPresentation(
                title: "Revealed in Finder",
                message: "\(skill.name) was revealed in Finder.",
                symbol: "checkmark.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        } catch {
            alertMessage = error.localizedDescription
        }
    }

    func delete(skill: SkillRecord) {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
                selectedSkillIDs.remove(skill.id)
                pruneSelectionToCurrentSkills()
                localBanner = InlineBannerPresentation(
                    title: "Moved to Trash",
                    message: "\(skill.name) was moved to Trash.",
                    symbol: "checkmark.circle.fill",
                    role: .warning,
                    recoveryActionTitle: nil
                )
            } catch {
                load()
                alertMessage = error.localizedDescription
            }
        }
    }

    func makeGlobal(skill: SkillRecord) {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.makeGlobal(skill: skill, confirmed: true)
                selectedSkillIDs.remove(skill.id)
                pruneSelectionToCurrentSkills()
                localBanner = InlineBannerPresentation(
                    title: "Made global",
                    message: "\(skill.name) was moved to global skills.",
                    symbol: "checkmark.circle.fill",
                    role: .warning,
                    recoveryActionTitle: nil
                )
            } catch {
                load()
                alertMessage = error.localizedDescription
            }
        }
    }

    func deleteSelectedSkills() {
        Task {
            await deleteSelectedSkillsNow()
        }
    }

    func deleteSelectedSkillsNow() async {
        let skillsToDelete = selectedSkills
        guard !skillsToDelete.isEmpty else {
            return
        }

        let total = skillsToDelete.count
        var successCount = 0
        var deletedIDs: Set<String> = []
        var failures: [(name: String, error: String)] = []

        for skill in skillsToDelete {
            do {
                let engine = makeEngine()
                state = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
                successCount += 1
                deletedIDs.insert(skill.id)
            } catch {
                failures.append((name: skill.name, error: error.localizedDescription))
            }
        }

        selectedSkillIDs.subtract(deletedIDs)
        pruneSelectionToCurrentSkills()

        if successCount > 0 {
            localBanner = InlineBannerPresentation(
                title: "Moved to Trash",
                message: "Deleted \(successCount) of \(total) selected skills.",
                symbol: "checkmark.circle.fill",
                role: failures.isEmpty ? .warning : .info,
                recoveryActionTitle: nil
            )
        } else {
            localBanner = nil
        }

        guard !failures.isEmpty else {
            return
        }
        alertMessage = bulkDeleteFailureMessage(total: total, failures: failures)
    }

    private func bulkDeleteFailureMessage(total: Int, failures: [(name: String, error: String)]) -> String {
        let maxLines = 5
        let lines = failures.prefix(maxLines).map { failure in
            "\(failure.name): \(failure.error)"
        }
        var message = "Failed to delete \(failures.count) of \(total) selected skills."
        if !lines.isEmpty {
            message += "\n\n" + lines.joined(separator: "\n")
        }
        if failures.count > maxLines {
            message += "\n...and \(failures.count - maxLines) more."
        }
        return message
    }
}
