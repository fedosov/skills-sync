import XCTest
@testable import SkillsSyncApp

final class SyncPresentationTests: XCTestCase {
    private var settingsTempDir: URL?

    override func tearDownWithError() throws {
        unsetenv("SKILLS_SYNC_GROUP_DIR")
        if let settingsTempDir {
            try? FileManager.default.removeItem(at: settingsTempDir)
        }
        settingsTempDir = nil
    }

    func testSyncStatusPresentationUsesEmpatheticTitlesAndSymbols() {
        XCTAssertEqual(SyncHealthStatus.ok.presentation.title, "Healthy")
        XCTAssertEqual(SyncHealthStatus.ok.presentation.symbol, "checkmark.circle.fill")

        XCTAssertEqual(SyncHealthStatus.syncing.presentation.title, "Sync in progress")
        XCTAssertEqual(SyncHealthStatus.syncing.presentation.symbol, "arrow.triangle.2.circlepath")

        XCTAssertEqual(SyncHealthStatus.failed.presentation.title, "Needs attention")
        XCTAssertEqual(SyncHealthStatus.failed.presentation.symbol, "exclamationmark.triangle.fill")

        XCTAssertEqual(SyncHealthStatus.unknown.presentation.title, "Waiting for first sync")
        XCTAssertEqual(SyncHealthStatus.unknown.presentation.symbol, "clock.badge.questionmark")
    }

    func testRelativeTimeFallbacksForMissingAndInvalidValues() {
        XCTAssertEqual(SyncFormatting.relativeTime(nil), "Never synced")
        XCTAssertEqual(SyncFormatting.relativeTime("invalid-date"), "Time unavailable")
    }

    func testApplyFiltersCombinesScopeAndSearch() {
        let skills = [
            makeSkill(id: "g-1", name: "Alpha", scope: "global"),
            makeSkill(id: "p-1", name: "Alpha Project", scope: "project"),
            makeSkill(id: "p-2", name: "Build Helper", scope: "project"),
            makeSkill(id: "g-2", name: "Zed", scope: "global")
        ]

        let filtered = AppViewModel.applyFilters(to: skills, query: "alpha", scopeFilter: .project)

        XCTAssertEqual(filtered.map(\.id), ["p-1"])
    }

    func testSyncFailureBannerIncludesRecoveryAndOptionalDetails() {
        let noDetails = InlineBannerPresentation.syncFailure(errorDetails: nil)
        XCTAssertEqual(noDetails.title, "Sync couldn't complete.")
        XCTAssertEqual(noDetails.recoveryActionTitle, "Sync now")
        XCTAssertEqual(noDetails.role, .error)
        XCTAssertTrue(noDetails.message.contains("Try Sync now. If this persists, open the app for details."))

        let withDetails = InlineBannerPresentation.syncFailure(errorDetails: "Connection timed out")
        XCTAssertTrue(withDetails.message.contains("Connection timed out"))
        XCTAssertTrue(withDetails.message.contains("Try Sync now."))
    }

    @MainActor
    func testFilteredSkillsRespectsScopeFilterInViewModel() {
        let viewModel = AppViewModel()
        viewModel.state = SyncState(
            version: 1,
            generatedAt: "2026-01-01T00:00:00Z",
            sync: .empty,
            summary: .empty,
            skills: [
                makeSkill(id: "g-1", name: "Global Skill", scope: "global"),
                makeSkill(id: "p-1", name: "Project Skill", scope: "project")
            ],
            topSkills: []
        )

        viewModel.scopeFilter = .all
        XCTAssertEqual(Set(viewModel.filteredSkills.map(\.id)), Set(["g-1", "p-1"]))

        viewModel.scopeFilter = .global
        XCTAssertEqual(viewModel.filteredSkills.map(\.id), ["g-1"])

        viewModel.scopeFilter = .project
        XCTAssertEqual(viewModel.filteredSkills.map(\.id), ["p-1"])
    }

    @MainActor
    func testSelectionDropsMissingIDsWhenStateChanges() {
        let viewModel = AppViewModel()
        viewModel.state = SyncState(
            version: 1,
            generatedAt: "2026-01-01T00:00:00Z",
            sync: .empty,
            summary: .empty,
            skills: [
                makeSkill(id: "g-1", name: "Global Skill", scope: "global"),
                makeSkill(id: "p-1", name: "Project Skill", scope: "project")
            ],
            topSkills: []
        )
        viewModel.selectedSkillIDs = Set(["g-1", "missing"])

        viewModel.pruneSelectionToCurrentSkills()

        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-1"]))
    }

    @MainActor
    func testSingleSelectedSkillAndSelectedSkillsAreComputedFromSelection() {
        let g1 = makeSkill(id: "g-1", name: "Global Skill", scope: "global")
        let p1 = makeSkill(id: "p-1", name: "Project Skill", scope: "project")
        let viewModel = AppViewModel()
        viewModel.state = SyncState(
            version: 1,
            generatedAt: "2026-01-01T00:00:00Z",
            sync: .empty,
            summary: .empty,
            skills: [g1, p1],
            topSkills: []
        )

        viewModel.selectedSkillIDs = Set(["g-1"])
        XCTAssertEqual(viewModel.singleSelectedSkill?.id, "g-1")
        XCTAssertEqual(viewModel.selectedSkills.map(\.id), ["g-1"])

        viewModel.selectedSkillIDs = Set(["g-1", "p-1"])
        XCTAssertNil(viewModel.singleSelectedSkill)
        XCTAssertEqual(Set(viewModel.selectedSkills.map(\.id)), Set(["g-1", "p-1"]))
    }

    @MainActor
    func testDeleteSelectedSkillsNowDeletesAllAndClearsSelection() async {
        var currentSkills = [
            makeSkill(id: "g-1", name: "One", scope: "global"),
            makeSkill(id: "g-2", name: "Two", scope: "global"),
            makeSkill(id: "g-3", name: "Three", scope: "global")
        ]
        let engine = MockSyncEngine { skill in
            currentSkills.removeAll(where: { $0.id == skill.id })
            return Self.makeState(skills: currentSkills)
        }
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: currentSkills)
        viewModel.selectedSkillIDs = Set(currentSkills.map(\.id))

        await viewModel.deleteSelectedSkillsNow()

        XCTAssertEqual(viewModel.state.skills.count, 0)
        XCTAssertTrue(viewModel.selectedSkillIDs.isEmpty)
        XCTAssertEqual(viewModel.localBanner?.message, "Deleted 3 of 3 selected skills.")
        XCTAssertNil(viewModel.alertMessage)
    }

    @MainActor
    func testDeleteSelectedSkillsNowContinuesOnPartialFailure() async {
        var currentSkills = [
            makeSkill(id: "g-1", name: "One", scope: "global"),
            makeSkill(id: "g-2", name: "Two", scope: "global"),
            makeSkill(id: "g-3", name: "Three", scope: "global")
        ]
        let engine = MockSyncEngine { skill in
            if skill.id == "g-2" {
                throw MockDeleteError()
            }
            currentSkills.removeAll(where: { $0.id == skill.id })
            return Self.makeState(skills: currentSkills)
        }
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: currentSkills)
        viewModel.selectedSkillIDs = Set(currentSkills.map(\.id))

        await viewModel.deleteSelectedSkillsNow()

        XCTAssertEqual(viewModel.state.skills.map(\.id), ["g-2"])
        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-2"]))
        XCTAssertEqual(viewModel.localBanner?.message, "Deleted 2 of 3 selected skills.")
        XCTAssertTrue(viewModel.alertMessage?.contains("Two") == true)
        XCTAssertTrue(viewModel.alertMessage?.contains("Mock delete error") == true)
    }

    @MainActor
    func testDeleteSelectedSkillsNowReportsFailureWhenAllFail() async {
        let skills = [
            makeSkill(id: "g-1", name: "One", scope: "global"),
            makeSkill(id: "g-2", name: "Two", scope: "global")
        ]
        let engine = MockSyncEngine { _ in
            throw MockDeleteError()
        }
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: skills)
        viewModel.selectedSkillIDs = Set(skills.map(\.id))

        await viewModel.deleteSelectedSkillsNow()

        XCTAssertEqual(Set(viewModel.state.skills.map(\.id)), Set(["g-1", "g-2"]))
        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-1", "g-2"]))
        XCTAssertNil(viewModel.localBanner)
        XCTAssertTrue(viewModel.alertMessage?.contains("Mock delete error") == true)
    }

    @MainActor
    func testMakeGlobalUpdatesStateAndShowsBannerOnSuccess() async {
        let projectSkill = makeSkill(id: "p-1", name: "Project Skill", scope: "project")
        let globalSkill = SkillRecord(
            id: "g-1",
            name: "Project Skill",
            scope: "global",
            workspace: nil,
            canonicalSourcePath: "/tmp/g-1",
            targetPaths: ["/tmp/target/g-1"],
            exists: true,
            isSymlinkCanonical: true,
            packageType: "dir",
            skillKey: projectSkill.skillKey,
            symlinkTarget: "/tmp/g-1"
        )
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onMakeGlobal: { _ in
                Self.makeState(skills: [globalSkill])
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [projectSkill])
        viewModel.selectedSkillIDs = Set([projectSkill.id])

        viewModel.makeGlobal(skill: projectSkill)
        for _ in 0..<50 where viewModel.localBanner?.title != "Made global" {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertEqual(viewModel.state.skills.map(\.scope), ["global"])
        XCTAssertTrue(viewModel.selectedSkillIDs.isEmpty)
        XCTAssertEqual(viewModel.localBanner?.title, "Made global")
    }

    @MainActor
    func testMakeGlobalLoadsAndShowsAlertOnFailure() async {
        let projectSkill = makeSkill(id: "p-1", name: "Project Skill", scope: "project")
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onMakeGlobal: { _ in
                throw MockDeleteError()
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [projectSkill])

        viewModel.makeGlobal(skill: projectSkill)
        for _ in 0..<50 where viewModel.alertMessage?.contains("Mock delete error") != true {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertTrue(viewModel.alertMessage?.contains("Mock delete error") == true)
    }

    @MainActor
    func testAutoMigrationToggleDefaultsToOff() throws {
        try prepareSettingsDirectory()

        let viewModel = AppViewModel()

        XCTAssertFalse(viewModel.autoMigrateToCanonicalSource)
    }

    @MainActor
    func testAutoMigrationTogglePersistsBetweenViewModelInstances() throws {
        try prepareSettingsDirectory()

        let first = AppViewModel()
        first.autoMigrateToCanonicalSource = true

        let second = AppViewModel()
        XCTAssertTrue(second.autoMigrateToCanonicalSource)
    }

    @MainActor
    func testWorkspaceDiscoveryRootsLoadFromSettings() throws {
        try prepareSettingsDirectory()
        let settings = SyncAppSettings(
            version: 2,
            autoMigrateToCanonicalSource: false,
            workspaceDiscoveryRoots: ["/Users/me/Work", "/Users/me/Sandbox"],
            windowState: nil,
            uiState: nil
        )
        SyncPreferencesStore().saveSettings(settings)

        let viewModel = AppViewModel()

        XCTAssertEqual(viewModel.workspaceDiscoveryRoots, ["/Users/me/Work", "/Users/me/Sandbox"])
    }

    @MainActor
    func testWorkspaceDiscoveryRootsAddRemovePersistsBetweenViewModelInstances() throws {
        try prepareSettingsDirectory()

        let first = AppViewModel()
        first.addWorkspaceDiscoveryRoot(" /Users/me/Work ")
        first.addWorkspaceDiscoveryRoot("/Users/me/Work")
        first.addWorkspaceDiscoveryRoot("/Users/me/Sandbox")
        first.removeWorkspaceDiscoveryRoot("/Users/me/Work")

        let second = AppViewModel()
        XCTAssertEqual(second.workspaceDiscoveryRoots, ["/Users/me/Sandbox"])
    }

    @MainActor
    func testViewModelRestoresUIStateFromSettings() throws {
        try prepareSettingsDirectory()
        let settings = SyncAppSettings(
            version: 2,
            autoMigrateToCanonicalSource: true,
            workspaceDiscoveryRoots: [],
            windowState: nil,
            uiState: AppUIState(
                sidebarWidth: 333,
                scopeFilter: ScopeFilter.project.rawValue,
                searchText: "restored-query",
                selectedSkillIDs: ["g-1", "p-1"]
            )
        )
        SyncPreferencesStore().saveSettings(settings)

        let viewModel = AppViewModel()

        XCTAssertTrue(viewModel.autoMigrateToCanonicalSource)
        XCTAssertEqual(viewModel.scopeFilter, .project)
        XCTAssertEqual(viewModel.searchText, "restored-query")
        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-1", "p-1"]))
    }

    @MainActor
    func testViewModelPersistsUIStateAfterChanges() async throws {
        try prepareSettingsDirectory()
        let viewModel = AppViewModel()

        viewModel.scopeFilter = .global
        viewModel.searchText = "new-query"
        viewModel.selectedSkillIDs = Set(["g-1"])

        try? await Task.sleep(nanoseconds: 400_000_000)
        let loaded = SyncPreferencesStore().loadSettings()

        XCTAssertEqual(loaded.version, 2)
        XCTAssertEqual(loaded.uiState?.scopeFilter, ScopeFilter.global.rawValue)
        XCTAssertEqual(loaded.uiState?.searchText, "new-query")
        XCTAssertEqual(Set(loaded.uiState?.selectedSkillIDs ?? []), Set(["g-1"]))
    }

    func testSidebarGroupsAllScopeContainsGlobalAndProjectSections() {
        let skills = [
            makeSkill(id: "g-1", name: "Global One", scope: "global"),
            makeSkill(id: "p-1", name: "Project One", scope: "project", workspace: "/tmp/zeta-app")
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["Global Skills (1)", "zeta-app (1)"])
    }

    func testSidebarGroupsProjectSectionsUseWorkspaceLastPathComponent() {
        let skills = [
            makeSkill(id: "p-1", name: "Project One", scope: "project", workspace: "/Users/me/Dev/alpha-app"),
            makeSkill(id: "p-2", name: "Project Two", scope: "project", workspace: "/Users/me/Work/beta-app")
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["alpha-app (1)", "beta-app (1)"])
    }

    func testSidebarGroupsProjectSkillWithoutWorkspaceGoesToUnknownProject() {
        let skills = [
            SkillRecord(
                id: "p-1",
                name: "Project One",
                scope: "project",
                workspace: nil,
                canonicalSourcePath: "/tmp/p-1",
                targetPaths: ["/tmp/target/p-1"],
                exists: true,
                isSymlinkCanonical: true,
                packageType: "dir",
                skillKey: "project-one",
                symlinkTarget: "/tmp/p-1"
            )
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["Unknown Project (1)"])
    }

    func testSidebarGroupsProjectSectionsAreSortedAlphabetically() {
        let skills = [
            makeSkill(id: "p-1", name: "Project One", scope: "project", workspace: "/tmp/zeta-app"),
            makeSkill(id: "p-2", name: "Project Two", scope: "project", workspace: "/tmp/alpha-app")
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["alpha-app (1)", "zeta-app (1)"])
    }

    func testSidebarGroupsSkillsAreSortedByNameThenPathWithinGroup() {
        let skills = [
            makeSkill(id: "p-1", name: "Build", scope: "project", workspace: "/tmp/alpha-app", sourcePath: "/tmp/zeta"),
            makeSkill(id: "p-2", name: "Alpha", scope: "project", workspace: "/tmp/alpha-app", sourcePath: "/tmp/second"),
            makeSkill(id: "p-3", name: "Build", scope: "project", workspace: "/tmp/alpha-app", sourcePath: "/tmp/alpha")
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups[0].skills.map(\.id), ["p-2", "p-3", "p-1"])
    }

    func testSidebarGroupsProjectScopeDoesNotContainGlobalSection() {
        let skills = AppViewModel.applyFilters(
            to: [
                makeSkill(id: "g-1", name: "Global One", scope: "global"),
                makeSkill(id: "p-1", name: "Project One", scope: "project", workspace: "/tmp/alpha-app")
            ],
            query: "",
            scopeFilter: .project
        )

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["alpha-app (1)"])
    }

    func testSidebarGroupsGlobalScopeDoesNotContainProjectSection() {
        let skills = AppViewModel.applyFilters(
            to: [
                makeSkill(id: "g-1", name: "Global One", scope: "global"),
                makeSkill(id: "p-1", name: "Project One", scope: "project", workspace: "/tmp/alpha-app")
            ],
            query: "",
            scopeFilter: .global
        )

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.map(\.title), ["Global Skills (1)"])
    }

    @MainActor
    func testSidebarUsesParsedTitleWhenAvailable() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try writeFile(dir.appendingPathComponent("SKILL.md"), contents: """
        ---
        title: Parsed Sidebar Title
        ---

        # Heading
        """)

        let skill = makeSkill(id: "g-1", name: "Fallback Name", scope: "global", sourcePath: dir.path)
        let viewModel = AppViewModel()

        XCTAssertEqual(viewModel.displayTitle(for: skill), "Parsed Sidebar Title")
    }

    @MainActor
    func testDetailPreviewShowsBothContentAndSymlinkRelations() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try writeFile(dir.appendingPathComponent("resources/implementation-playbook.md"), contents: "hello")
        try writeFile(dir.appendingPathComponent("SKILL.md"), contents: """
        ---
        name: detail-preview
        ---

        Use `resources/implementation-playbook.md`.
        """)

        let skill = makeSkill(id: "g-1", name: "Fallback Name", scope: "global", sourcePath: dir.path)
        let viewModel = AppViewModel()
        let preview = await viewModel.preview(for: skill)

        XCTAssertTrue(preview.relations.contains(where: { $0.kind == .content }))
        XCTAssertTrue(preview.relations.contains(where: { $0.kind == .symlink }))
    }

    @MainActor
    func testValidationReturnsIssuesAndWarningFlagForInvalidSkill() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        let skill = makeSkill(id: "g-1", name: "Invalid Skill", scope: "global", sourcePath: dir.path)
        let viewModel = AppViewModel()
        let result = viewModel.validation(for: skill)

        XCTAssertTrue(result.issues.contains(where: { $0.code == "missing_skill_md" }))
        XCTAssertTrue(viewModel.hasValidationWarnings(for: skill))
    }

    func testSkillsSyncAppContainsValidationSectionAndWarningIndicators() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let appFile = repoRoot.appendingPathComponent("Sources/App/SkillsSyncApp.swift")
        let source = try String(contentsOf: appFile, encoding: .utf8)

        XCTAssertTrue(source.contains("Section(\"Validation\")"))
        XCTAssertTrue(source.contains("exclamationmark.triangle.fill"))
        XCTAssertTrue(source.contains("validation issue(s)"))
        XCTAssertTrue(source.contains("No validation warnings"))
        XCTAssertTrue(source.contains("You can click issues and the repair prompt will be copied."))
        XCTAssertTrue(source.contains("SkillRepairPromptBuilder.prompt"))
        XCTAssertTrue(source.contains("NSPasteboard.general"))
    }

    func testSkillsSyncAppContainsWorkspaceRootsControlsInHealthPopover() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let appFile = repoRoot.appendingPathComponent("Sources/App/SkillsSyncApp.swift")
        let source = try String(contentsOf: appFile, encoding: .utf8)

        XCTAssertTrue(source.contains("Workspace search roots"))
        XCTAssertTrue(source.contains("Add Root"))
        XCTAssertTrue(source.contains("Remove Root"))
    }

    private func makeSkill(
        id: String,
        name: String,
        scope: String,
        workspace: String? = nil,
        sourcePath: String? = nil
    ) -> SkillRecord {
        SkillRecord(
            id: id,
            name: name,
            scope: scope,
            workspace: workspace ?? (scope == "project" ? "/tmp/project" : nil),
            canonicalSourcePath: sourcePath ?? "/tmp/\(id)",
            targetPaths: ["/tmp/target/\(id)"],
            exists: true,
            isSymlinkCanonical: true,
            packageType: "dir",
            skillKey: name.lowercased(),
            symlinkTarget: "/tmp/\(id)"
        )
    }

    private static func makeState(skills: [SkillRecord]) -> SyncState {
        SyncState(
            version: 1,
            generatedAt: "2026-01-01T00:00:00Z",
            sync: .empty,
            summary: .empty,
            skills: skills,
            topSkills: []
        )
    }

    private func prepareSettingsDirectory() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        settingsTempDir = dir
        setenv("SKILLS_SYNC_GROUP_DIR", dir.path, 1)
    }

    private func writeFile(_ path: URL, contents: String) throws {
        try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try XCTUnwrap(contents.data(using: .utf8)).write(to: path)
    }
}

private struct MockDeleteError: LocalizedError {
    var errorDescription: String? { "Mock delete error" }
}

private final class MockSyncEngine: SyncEngineControlling {
    private let onDelete: (SkillRecord) async throws -> SyncState
    private let onMakeGlobal: (SkillRecord) async throws -> SyncState

    init(
        onDelete: @escaping (SkillRecord) async throws -> SyncState,
        onMakeGlobal: @escaping (SkillRecord) async throws -> SyncState = { _ in .empty }
    ) {
        self.onDelete = onDelete
        self.onMakeGlobal = onMakeGlobal
    }

    func runSync(trigger: SyncTrigger) async throws -> SyncState {
        .empty
    }

    func openInZed(skill: SkillRecord) throws { }

    func revealInFinder(skill: SkillRecord) throws { }

    func deleteCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onDelete(skill)
    }

    func makeGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onMakeGlobal(skill)
    }
}
