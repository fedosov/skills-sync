import Foundation

enum SidebarSkillGroupKind: Hashable {
    case global
    case project(name: String)
    case unknownProject
    case archived
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
    func archiveCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState
    func restoreArchivedSkillToGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState
    func makeGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState
    func renameSkill(skill: SkillRecord, newTitle: String) async throws -> SyncState
    func repairCodexFrontmatter(skill: SkillRecord) async throws -> SyncState
}

extension SyncEngine: SyncEngineControlling { }

@MainActor
final class AppViewModel: ObservableObject {
    @Published var state: SyncState = .empty {
        didSet {
            if state.generatedAt != oldValue.generatedAt {
                previewCache.removeAll()
                validationCache.removeAll()
            }
            prunePreviewCacheToCurrentState()
        }
    }
    @Published var searchText: String = "" {
        didSet {
            scheduleUIPreferencesSave()
        }
    }
    @Published var scopeFilter: ScopeFilter = .all {
        didSet {
            saveUIStateNow(sidebarWidth: nil)
        }
    }
    @Published var selectedSkillIDs: Set<String> = [] {
        didSet {
            scheduleUIPreferencesSave()
        }
    }
    @Published var alertMessage: String?
    @Published var localBanner: InlineBannerPresentation?
    @Published var autoMigrateToCanonicalSource: Bool = false {
        didSet {
            guard isPreferencesLoaded else { return }
            currentSettings = SyncAppSettings(
                version: 2,
                autoMigrateToCanonicalSource: autoMigrateToCanonicalSource,
                workspaceDiscoveryRoots: workspaceDiscoveryRoots,
                windowState: currentSettings.windowState,
                uiState: currentSettings.uiState
            )
            preferencesStore.saveSettings(currentSettings)
        }
    }
    @Published var workspaceDiscoveryRoots: [String] = [] {
        didSet {
            guard isPreferencesLoaded else { return }
            currentSettings = SyncAppSettings(
                version: 2,
                autoMigrateToCanonicalSource: autoMigrateToCanonicalSource,
                workspaceDiscoveryRoots: workspaceDiscoveryRoots,
                windowState: currentSettings.windowState,
                uiState: currentSettings.uiState
            )
            preferencesStore.saveSettings(currentSettings)
        }
    }

    private let store: SyncStateStore
    private let preferencesStore: SyncPreferencesStore
    private let makeEngine: () -> any SyncEngineControlling
    private let makeAutoSyncCoordinator: (@escaping (AutoSyncEvent) -> Void) -> any AutoSyncCoordinating
    private let previewParser: SkillPreviewParser
    private let skillValidator: SkillValidator
    private let autoSyncDebounceSeconds: TimeInterval
    private var isPreferencesLoaded = false
    private var currentSettings: SyncAppSettings = .default
    private var pendingUIPreferencesSave: DispatchWorkItem?
    private var pendingAutoSyncWorkItem: DispatchWorkItem?
    private var previewCache: [String: CachedSkillPreview] = [:]
    private var validationCache: [String: CachedSkillValidation] = [:]
    private var autoSyncCoordinator: (any AutoSyncCoordinating)?
    private var isSyncInFlight = false
    private var hasPendingAutoSync = false

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
        makeEngine: @escaping () -> any SyncEngineControlling = { SyncEngine() },
        makeAutoSyncCoordinator: @escaping (@escaping (AutoSyncEvent) -> Void) -> any AutoSyncCoordinating = {
            AutoSyncCoordinator(onEvent: $0)
        },
        previewParser: SkillPreviewParser = SkillPreviewParser(),
        skillValidator: SkillValidator = SkillValidator(),
        autoSyncDebounceSeconds: TimeInterval = 1.5
    ) {
        self.store = store
        self.preferencesStore = preferencesStore
        self.makeEngine = makeEngine
        self.makeAutoSyncCoordinator = makeAutoSyncCoordinator
        self.previewParser = previewParser
        self.skillValidator = skillValidator
        self.autoSyncDebounceSeconds = autoSyncDebounceSeconds
        let settings = preferencesStore.loadSettings()
        currentSettings = settings
        autoMigrateToCanonicalSource = settings.autoMigrateToCanonicalSource
        workspaceDiscoveryRoots = Self.normalizedWorkspaceRoots(settings.workspaceDiscoveryRoots)
        if let restoredScope = ScopeFilter(rawValue: settings.uiState?.scopeFilter ?? "") {
            scopeFilter = restoredScope
        }
        searchText = settings.uiState?.searchText ?? ""
        selectedSkillIDs = Set(settings.uiState?.selectedSkillIDs ?? [])
        isPreferencesLoaded = true
    }

    var filteredSkills: [SkillRecord] {
        Self.applyFilters(to: state.skills, query: searchText, scopeFilter: scopeFilter)
    }

    nonisolated static func sidebarGroups(from skills: [SkillRecord]) -> [SidebarSkillGroup] {
        var globalSkills: [SkillRecord] = []
        var projectSkillsByName: [String: [SkillRecord]] = [:]
        var unknownProjectSkills: [SkillRecord] = []
        var archivedSkills: [SkillRecord] = []

        for skill in skills {
            if skill.status == .archived {
                archivedSkills.append(skill)
                continue
            }
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

        if !archivedSkills.isEmpty {
            let sorted = sortSkillsForSidebar(archivedSkills)
            groups.append(
                SidebarSkillGroup(
                    id: "archived",
                    title: "Archived Skills (\(sorted.count))",
                    skills: sorted,
                    kind: .archived
                )
            )
        }

        return groups
    }

    nonisolated static func applyFilters(to skills: [SkillRecord], query: String, scopeFilter: ScopeFilter) -> [SkillRecord] {
        let base = skills.sorted { lhs, rhs in
            if lhs.status != rhs.status {
                return lhs.status == .active
            }
            if lhs.scope != rhs.scope {
                return lhs.scope == "global"
            }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }

        let statusFiltered: [SkillRecord]
        if scopeFilter == .all {
            statusFiltered = base
        } else {
            statusFiltered = base.filter { $0.status == .active }
        }
        let scoped = statusFiltered.filter(scopeFilter.includes)
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
        autoSyncCoordinator?.stop()
        let coordinator = makeAutoSyncCoordinator { [weak self] event in
            Task { @MainActor in
                self?.handleAutoSyncEvent(event)
            }
        }
        autoSyncCoordinator = coordinator
        coordinator.start()
        scheduleAutoSync(reason: .appStarted)
    }

    func stop() {
        autoSyncCoordinator?.stop()
        autoSyncCoordinator = nil
        saveUIStateNow(sidebarWidth: nil)
        pendingUIPreferencesSave?.cancel()
        pendingUIPreferencesSave = nil
        pendingAutoSyncWorkItem?.cancel()
        pendingAutoSyncWorkItem = nil
        hasPendingAutoSync = false
        isSyncInFlight = false
    }

    func load() {
        state = store.loadState()
        pruneSelectionToCurrentSkills()
    }

    func displayTitle(for skill: SkillRecord) -> String {
        previewData(for: skill).displayTitle
    }

    func preview(for skill: SkillRecord) async -> SkillPreviewData {
        previewData(for: skill)
    }

    func warmupTitles(for skills: [SkillRecord]) async {
        for skill in skills {
            _ = previewData(for: skill)
        }
    }

    func warmupValidation(for skills: [SkillRecord]) async {
        for skill in skills {
            _ = validationData(for: skill)
        }
    }

    func validation(for skill: SkillRecord) -> SkillValidationResult {
        validationData(for: skill)
    }

    func hasValidationWarnings(for skill: SkillRecord) -> Bool {
        validationData(for: skill).hasWarnings
    }

    func pruneSelectionToCurrentSkills() {
        let validIDs = Set(state.skills.map(\.id))
        selectedSkillIDs = selectedSkillIDs.intersection(validIDs)
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

    func moveToTrash(skill: SkillRecord) {
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

    func archive(skill: SkillRecord) {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.archiveCanonicalSource(skill: skill, confirmed: true)
                selectedSkillIDs.remove(skill.id)
                pruneSelectionToCurrentSkills()
                localBanner = InlineBannerPresentation(
                    title: "Archived",
                    message: "\(skill.name) was archived.",
                    symbol: "archivebox.fill",
                    role: .warning,
                    recoveryActionTitle: nil
                )
            } catch {
                load()
                alertMessage = error.localizedDescription
            }
        }
    }

    func restoreToGlobal(skill: SkillRecord) {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.restoreArchivedSkillToGlobal(skill: skill, confirmed: true)
                selectedSkillIDs.remove(skill.id)
                pruneSelectionToCurrentSkills()
                localBanner = InlineBannerPresentation(
                    title: "Restored",
                    message: "\(skill.name) was restored to global skills.",
                    symbol: "arrow.uturn.backward.circle.fill",
                    role: .success,
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

    func rename(skill: SkillRecord, newTitle: String) {
        Task {
            do {
                let engine = makeEngine()
                let trimmedTitle = newTitle.trimmingCharacters(in: .whitespacesAndNewlines)
                state = try await engine.renameSkill(skill: skill, newTitle: trimmedTitle)
                let expectedKey = normalizedSkillKey(from: trimmedTitle)
                if let renamed = state.skills.first(where: {
                    $0.scope == skill.scope
                        && ($0.workspace ?? "") == (skill.workspace ?? "")
                        && $0.skillKey == expectedKey
                }) {
                    selectedSkillIDs = Set([renamed.id])
                } else {
                    selectedSkillIDs.remove(skill.id)
                    pruneSelectionToCurrentSkills()
                }
                localBanner = InlineBannerPresentation(
                    title: "Skill renamed",
                    message: "Updated to \(trimmedTitle).",
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

    func repairCodexFrontmatter(skill: SkillRecord) {
        Task {
            do {
                let engine = makeEngine()
                state = try await engine.repairCodexFrontmatter(skill: skill)
                previewCache.removeAll()
                validationCache.removeAll()
                pruneSelectionToCurrentSkills()
                localBanner = InlineBannerPresentation(
                    title: "Codex frontmatter repaired",
                    message: "\(skill.name) was repaired and synced.",
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

    func deleteSelectedSkills() {
        Task {
            await trashSelectedSkillsNow()
        }
    }

    func archiveSelectedSkills() {
        Task {
            await archiveSelectedSkillsNow()
        }
    }

    func addWorkspaceDiscoveryRoot(_ candidate: String) {
        guard let normalized = Self.normalizedWorkspaceRoot(candidate) else { return }
        if workspaceDiscoveryRoots.contains(normalized) {
            return
        }
        workspaceDiscoveryRoots.append(normalized)
    }

    func removeWorkspaceDiscoveryRoot(_ root: String) {
        guard let normalized = Self.normalizedWorkspaceRoot(root) else { return }
        workspaceDiscoveryRoots.removeAll(where: { $0 == normalized })
    }

    func trashSelectedSkillsNow() async {
        let skillsToTrash = selectedSkills.filter { $0.status == .active }
        guard !skillsToTrash.isEmpty else {
            return
        }

        let total = skillsToTrash.count
        var successCount = 0
        var removedIDs: Set<String> = []
        var failures: [(name: String, error: String)] = []

        for skill in skillsToTrash {
            do {
                let engine = makeEngine()
                state = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
                successCount += 1
                removedIDs.insert(skill.id)
            } catch {
                failures.append((name: skill.name, error: error.localizedDescription))
            }
        }

        selectedSkillIDs.subtract(removedIDs)
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

    func archiveSelectedSkillsNow() async {
        let selected = selectedSkills
        guard !selected.isEmpty else {
            return
        }

        let skillsToArchive = selected.filter { $0.status == .active }
        let skippedArchived = selected.count - skillsToArchive.count
        guard !skillsToArchive.isEmpty else {
            localBanner = InlineBannerPresentation(
                title: "Nothing to archive",
                message: "Only active skills can be archived.",
                symbol: "archivebox",
                role: .info,
                recoveryActionTitle: nil
            )
            return
        }

        let total = skillsToArchive.count
        var successCount = 0
        var archivedIDs: Set<String> = []
        var failures: [(name: String, error: String)] = []

        for skill in skillsToArchive {
            do {
                let engine = makeEngine()
                state = try await engine.archiveCanonicalSource(skill: skill, confirmed: true)
                successCount += 1
                archivedIDs.insert(skill.id)
            } catch {
                failures.append((name: skill.name, error: error.localizedDescription))
            }
        }

        selectedSkillIDs.subtract(archivedIDs)
        pruneSelectionToCurrentSkills()

        if successCount > 0 {
            var message = "Archived \(successCount) of \(total) selected skills."
            if skippedArchived > 0 {
                message += " Skipped \(skippedArchived) already archived."
            }
            localBanner = InlineBannerPresentation(
                title: "Archived",
                message: message,
                symbol: "archivebox.fill",
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

    func delete(skill: SkillRecord) {
        moveToTrash(skill: skill)
    }

    func deleteSelectedSkillsNow() async {
        await trashSelectedSkillsNow()
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

    private func previewData(for skill: SkillRecord) -> SkillPreviewData {
        let signature = previewSignature(for: skill)
        if let cached = previewCache[skill.id], cached.signature == signature {
            return cached.preview
        }

        let preview = previewParser.parse(skill: skill)
        previewCache[skill.id] = CachedSkillPreview(signature: signature, preview: preview)
        return preview
    }

    private func prunePreviewCacheToCurrentState() {
        let valid = Set(state.skills.map(\.id))
        previewCache = previewCache.filter { valid.contains($0.key) }
        validationCache = validationCache.filter { valid.contains($0.key) }
    }

    private func previewSignature(for skill: SkillRecord) -> String {
        let targets = skill.targetPaths.sorted().joined(separator: "|")
        return "\(skill.id)|\(skill.name)|\(skill.exists)|\(skill.packageType)|\(skill.canonicalSourcePath)|\(targets)"
    }

    private func validationData(for skill: SkillRecord) -> SkillValidationResult {
        let signature = validationSignature(for: skill)
        if let cached = validationCache[skill.id], cached.signature == signature {
            return cached.validation
        }

        let validation = skillValidator.validate(skill: skill)
        validationCache[skill.id] = CachedSkillValidation(signature: signature, validation: validation)
        return validation
    }

    private func validationSignature(for skill: SkillRecord) -> String {
        let targets = skill.targetPaths.sorted().joined(separator: "|")
        return "\(skill.id)|\(skill.name)|\(skill.exists)|\(skill.packageType)|\(skill.canonicalSourcePath)|\(targets)"
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

    func restoredWindowState() -> AppWindowState? {
        currentSettings.windowState
    }

    func restoredSidebarWidth() -> Double? {
        WindowStateGeometry.clampSidebarWidth(currentSettings.uiState?.sidebarWidth)
    }

    func persistWindowSnapshot(frame: CGRect, isZoomed: Bool, sidebarWidth: Double?) {
        guard isPreferencesLoaded else { return }

        let windowState = AppWindowState(
            x: frame.origin.x,
            y: frame.origin.y,
            width: frame.size.width,
            height: frame.size.height,
            isMaximized: isZoomed
        )
        let uiState = AppUIState(
            sidebarWidth: WindowStateGeometry.clampSidebarWidth(sidebarWidth),
            scopeFilter: scopeFilter.rawValue,
            searchText: searchText,
            selectedSkillIDs: Array(selectedSkillIDs).sorted()
        )

        currentSettings = SyncAppSettings(
            version: 2,
            autoMigrateToCanonicalSource: autoMigrateToCanonicalSource,
            workspaceDiscoveryRoots: workspaceDiscoveryRoots,
            windowState: windowState,
            uiState: uiState
        )
        preferencesStore.saveSettings(currentSettings)
    }

    func persistUIState(sidebarWidth: Double?) {
        saveUIStateNow(sidebarWidth: sidebarWidth)
    }

    private func scheduleUIPreferencesSave() {
        guard isPreferencesLoaded else { return }
        pendingUIPreferencesSave?.cancel()

        let work = DispatchWorkItem { [weak self] in
            Task { @MainActor in
                self?.saveUIStateNow(sidebarWidth: nil)
            }
        }
        pendingUIPreferencesSave = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: work)
    }

    private func handleAutoSyncEvent(_ event: AutoSyncEvent) {
        switch event {
        case .skillsFilesystemChanged:
            scheduleAutoSync(reason: .skillsFilesystemChanged)
        case .workspaceWatchListChanged:
            scheduleAutoSync(reason: .workspaceWatchListChanged)
        case .runtimeStateChanged:
            load()
        }
    }

    private func scheduleAutoSync(reason: AutoSyncReason) {
        pendingAutoSyncWorkItem?.cancel()

        let work = DispatchWorkItem { [weak self] in
            Task { @MainActor in
                await self?.runAutoSyncIfNeeded()
            }
        }
        pendingAutoSyncWorkItem = work

        let delay: TimeInterval
        switch reason {
        case .appStarted:
            delay = 0
        case .skillsFilesystemChanged, .workspaceWatchListChanged:
            delay = autoSyncDebounceSeconds
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }

    private func runAutoSyncIfNeeded() async {
        if isSyncInFlight {
            hasPendingAutoSync = true
            return
        }

        isSyncInFlight = true
        defer {
            isSyncInFlight = false
            if hasPendingAutoSync {
                hasPendingAutoSync = false
                scheduleAutoSync(reason: .skillsFilesystemChanged)
            }
        }

        do {
            let engine = makeEngine()
            state = try await engine.runSync(trigger: .autoFilesystem)
            autoSyncCoordinator?.refreshWatchedPaths()
        } catch {
            load()
            alertMessage = error.localizedDescription
        }
    }

    private func saveUIStateNow(sidebarWidth: Double?) {
        guard isPreferencesLoaded else { return }

        let uiState = AppUIState(
            sidebarWidth: WindowStateGeometry.clampSidebarWidth(sidebarWidth ?? currentSettings.uiState?.sidebarWidth),
            scopeFilter: scopeFilter.rawValue,
            searchText: searchText,
            selectedSkillIDs: Array(selectedSkillIDs).sorted()
        )
        currentSettings = SyncAppSettings(
            version: 2,
            autoMigrateToCanonicalSource: autoMigrateToCanonicalSource,
            workspaceDiscoveryRoots: workspaceDiscoveryRoots,
            windowState: currentSettings.windowState,
            uiState: uiState
        )
        preferencesStore.saveSettings(currentSettings)
    }

    nonisolated private static func normalizedWorkspaceRoots(_ roots: [String]) -> [String] {
        var normalized: [String] = []
        var seen: Set<String> = []
        for root in roots {
            guard let value = normalizedWorkspaceRoot(root) else { continue }
            guard !seen.contains(value) else { continue }
            seen.insert(value)
            normalized.append(value)
        }
        return normalized
    }

    nonisolated private static func normalizedWorkspaceRoot(_ root: String) -> String? {
        let trimmed = root.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        guard trimmed.hasPrefix("/") else { return nil }
        return URL(fileURLWithPath: trimmed, isDirectory: true).standardizedFileURL.path
    }
}

private enum AutoSyncReason {
    case appStarted
    case skillsFilesystemChanged
    case workspaceWatchListChanged
}

private struct CachedSkillPreview {
    let signature: String
    let preview: SkillPreviewData
}

private struct CachedSkillValidation {
    let signature: String
    let validation: SkillValidationResult
}
