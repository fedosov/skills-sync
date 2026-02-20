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

    func testApplyFiltersHidesArchivedOutsideAllScope() {
        let skills = [
            makeSkill(id: "g-1", name: "Active Global", scope: "global", status: .active),
            makeSkill(id: "a-1", name: "Archived Skill", scope: "global", status: .archived)
        ]

        XCTAssertEqual(AppViewModel.applyFilters(to: skills, query: "", scopeFilter: .all).map(\.id), ["g-1", "a-1"])
        XCTAssertEqual(AppViewModel.applyFilters(to: skills, query: "", scopeFilter: .global).map(\.id), ["g-1"])
        XCTAssertTrue(AppViewModel.applyFilters(to: skills, query: "", scopeFilter: .project).isEmpty)
    }

    func testSyncFailureBannerIncludesRecoveryAndOptionalDetails() {
        let noDetails = InlineBannerPresentation.syncFailure(errorDetails: nil)
        XCTAssertEqual(noDetails.title, "Sync couldn't complete.")
        XCTAssertNil(noDetails.recoveryActionTitle)
        XCTAssertEqual(noDetails.role, .error)
        XCTAssertTrue(noDetails.message.contains("Automatic sync will retry. If this persists, open the app for details."))

        let withDetails = InlineBannerPresentation.syncFailure(errorDetails: "Connection timed out")
        XCTAssertTrue(withDetails.message.contains("Connection timed out"))
        XCTAssertTrue(withDetails.message.contains("Automatic sync will retry."))
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
            symlinkTarget: "/tmp/g-1",
            status: .active,
            archivedAt: nil,
            archivedBundlePath: nil,
            archivedOriginalScope: nil,
            archivedOriginalWorkspace: nil
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
    func testRenameUpdatesStateAndShowsBannerOnSuccess() async {
        let oldSkill = makeSkill(id: "g-1", name: "Old Skill", scope: "global", sourcePath: "/tmp/old-skill")
        let renamedSkill = SkillRecord(
            id: "g-2",
            name: "new-skill-name",
            scope: "global",
            workspace: nil,
            canonicalSourcePath: "/tmp/new-skill-name",
            targetPaths: ["/tmp/target/new-skill-name"],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: "new-skill-name",
            symlinkTarget: "/tmp/new-skill-name",
            status: .active,
            archivedAt: nil,
            archivedBundlePath: nil,
            archivedOriginalScope: nil,
            archivedOriginalWorkspace: nil
        )
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onMakeGlobal: { _ in .empty },
            onRename: { _, _ in
                Self.makeState(skills: [renamedSkill])
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [oldSkill])
        viewModel.selectedSkillIDs = Set([oldSkill.id])

        viewModel.rename(skill: oldSkill, newTitle: "New Skill Name")
        for _ in 0..<50 where viewModel.localBanner?.title != "Skill renamed" {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertEqual(viewModel.state.skills.map(\.skillKey), ["new-skill-name"])
        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-2"]))
        XCTAssertEqual(viewModel.localBanner?.title, "Skill renamed")
    }

    @MainActor
    func testRenameShowsAlertOnFailure() async {
        let oldSkill = makeSkill(id: "g-1", name: "Old Skill", scope: "global", sourcePath: "/tmp/old-skill")
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onMakeGlobal: { _ in .empty },
            onRename: { _, _ in
                throw MockDeleteError()
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [oldSkill])
        viewModel.selectedSkillIDs = Set([oldSkill.id])

        viewModel.rename(skill: oldSkill, newTitle: "New Skill Name")
        for _ in 0..<50 where viewModel.alertMessage?.contains("Mock delete error") != true {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertTrue(viewModel.alertMessage?.contains("Mock delete error") == true)
    }

    @MainActor
    func testRenameKeepsSelectionOnRenamedSkill() async {
        let oldSkill = makeSkill(id: "g-1", name: "Old Skill", scope: "global", sourcePath: "/tmp/old-skill")
        let renamedSkill = SkillRecord(
            id: "g-2",
            name: "new-skill-name",
            scope: "global",
            workspace: nil,
            canonicalSourcePath: "/tmp/new-skill-name",
            targetPaths: ["/tmp/target/new-skill-name"],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: "new-skill-name",
            symlinkTarget: "/tmp/new-skill-name",
            status: .active,
            archivedAt: nil,
            archivedBundlePath: nil,
            archivedOriginalScope: nil,
            archivedOriginalWorkspace: nil
        )
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onMakeGlobal: { _ in .empty },
            onRename: { _, _ in
                Self.makeState(skills: [renamedSkill])
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [oldSkill])
        viewModel.selectedSkillIDs = Set([oldSkill.id])

        viewModel.rename(skill: oldSkill, newTitle: "New Skill Name")
        for _ in 0..<50 where viewModel.selectedSkillIDs != Set(["g-2"]) {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertEqual(viewModel.selectedSkillIDs, Set(["g-2"]))
        XCTAssertEqual(viewModel.singleSelectedSkill?.skillKey, "new-skill-name")
    }

    @MainActor
    func testApplyValidationFixUpdatesStateAndShowsBannerOnSuccess() async {
        let skill = makeSkill(id: "g-1", name: "Fixable", scope: "global", sourcePath: "/tmp/fixable")
        let fixedSkill = makeSkill(id: "g-1", name: "Fixable", scope: "global", sourcePath: "/tmp/fixable")
        let issue = SkillValidationIssue(code: "missing_frontmatter_name", message: "Frontmatter `name` is required")
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onApplyValidationFix: { _, _ in
                Self.makeState(skills: [fixedSkill])
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [skill])

        viewModel.applyValidationFix(skill: skill, issue: issue)
        for _ in 0..<50 where viewModel.localBanner?.title != "Validation issue fixed" {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertEqual(viewModel.state.skills.count, 1)
        XCTAssertEqual(viewModel.localBanner?.title, "Validation issue fixed")
        XCTAssertTrue(viewModel.localBanner?.message.contains("Fixable") == true)
    }

    @MainActor
    func testApplyValidationFixShowsAlertOnFailure() async {
        let skill = makeSkill(id: "g-1", name: "Fixable", scope: "global", sourcePath: "/tmp/fixable")
        let issue = SkillValidationIssue(code: "missing_frontmatter_name", message: "Frontmatter `name` is required")
        let engine = MockSyncEngine(
            onDelete: { _ in .empty },
            onApplyValidationFix: { _, _ in
                throw MockDeleteError()
            }
        )
        let viewModel = AppViewModel(makeEngine: { engine })
        viewModel.state = Self.makeState(skills: [skill])

        viewModel.applyValidationFix(skill: skill, issue: issue)
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

    @MainActor
    func testStopPersistsPendingUIStateImmediately() throws {
        try prepareSettingsDirectory()
        let viewModel = AppViewModel()

        viewModel.scopeFilter = .project
        viewModel.searchText = "last-tab"
        viewModel.selectedSkillIDs = Set(["g-1"])
        viewModel.stop()

        let loaded = SyncPreferencesStore().loadSettings()
        XCTAssertEqual(loaded.uiState?.scopeFilter, ScopeFilter.project.rawValue)
        XCTAssertEqual(loaded.uiState?.searchText, "last-tab")
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
                symlinkTarget: "/tmp/p-1",
                status: .active,
                archivedAt: nil,
                archivedBundlePath: nil,
                archivedOriginalScope: nil,
                archivedOriginalWorkspace: nil
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

    func testSidebarGroupsPlaceArchivedSectionAtBottom() {
        let skills = [
            makeSkill(id: "g-1", name: "Global", scope: "global", status: .active),
            makeSkill(id: "p-1", name: "Project", scope: "project", workspace: "/tmp/workspace-a", status: .active),
            makeSkill(id: "a-1", name: "Archived", scope: "global", status: .archived)
        ]

        let groups = AppViewModel.sidebarGroups(from: skills)

        XCTAssertEqual(groups.last?.id, "archived")
        XCTAssertEqual(groups.last?.title, "Archived Skills (1)")
        XCTAssertEqual(groups.last?.skills.map(\.id), ["a-1"])
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

    func testSkillDetailModuleContainsValidationSectionAndWarningIndicators() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let detailFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillDetailView.swift")
        let panelFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillValidationPanel.swift")
        let overviewFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillOverviewCard.swift")
        let detailSource = try String(contentsOf: detailFile, encoding: .utf8)
        let panelSource = try String(contentsOf: panelFile, encoding: .utf8)
        let overviewSource = try String(contentsOf: overviewFile, encoding: .utf8)

        XCTAssertTrue(detailSource.contains("Section(\"Validation\")"))
        XCTAssertTrue(detailSource.contains("exclamationmark.triangle.fill"))
        XCTAssertTrue(detailSource.contains("validation issue(s)"))
        XCTAssertTrue(detailSource.contains("No validation warnings"))
        XCTAssertTrue(panelSource.contains("Codex visibility"))
        XCTAssertTrue(overviewSource.contains("Codex visible"))
        XCTAssertTrue(overviewSource.contains("Codex hidden"))
        XCTAssertTrue(panelSource.contains("Button(\"Fix\")"))
        XCTAssertFalse(panelSource.contains("Repair for Codex"))
        XCTAssertTrue(panelSource.contains("Select an issue to copy a repair prompt."))
        XCTAssertTrue(detailSource.contains("SkillRepairPromptBuilder.prompt"))
        XCTAssertTrue(detailSource.contains("NSPasteboard.general"))
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
        XCTAssertFalse(source.contains("Button(\"Refresh\")"))
        XCTAssertFalse(source.contains("Button(\"Sync Now\")"))
        XCTAssertFalse(source.contains("Run Sync Now to discover skills"))
        XCTAssertFalse(source.contains("onSyncNow"))
    }

    @MainActor
    func testStartRunsInitialAutoSyncAndStartsCoordinator() async {
        let expected = Self.makeState(skills: [makeSkill(id: "g-1", name: "Auto", scope: "global")])
        let engine = MockSyncEngine(
            onRunSync: { trigger in
                XCTAssertEqual(trigger, .autoFilesystem)
                return expected
            },
            onDelete: { _ in .empty }
        )
        let coordinator = MockAutoSyncCoordinator()
        let viewModel = AppViewModel(
            makeEngine: { engine },
            makeAutoSyncCoordinator: { callback in
                coordinator.onEvent = callback
                return coordinator
            }
        )

        viewModel.start()
        for _ in 0..<100 where viewModel.state.skills != expected.skills {
            await Task.yield()
            try? await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertEqual(engine.runSyncTriggers, [.autoFilesystem])
        XCTAssertEqual(viewModel.state.skills, expected.skills)
        XCTAssertEqual(coordinator.startCalls, 1)
    }

    @MainActor
    func testAutoSyncDebouncesFilesystemEvents() async {
        let expected = Self.makeState(skills: [makeSkill(id: "g-1", name: "Auto", scope: "global")])
        let engine = MockSyncEngine(
            onRunSync: { _ in expected },
            onDelete: { _ in .empty }
        )
        let coordinator = MockAutoSyncCoordinator()
        let viewModel = AppViewModel(
            makeEngine: { engine },
            makeAutoSyncCoordinator: { callback in
                coordinator.onEvent = callback
                return coordinator
            },
            autoSyncDebounceSeconds: 0.03
        )

        viewModel.start()
        coordinator.emit(.skillsFilesystemChanged)
        coordinator.emit(.skillsFilesystemChanged)
        coordinator.emit(.skillsFilesystemChanged)
        try? await Task.sleep(nanoseconds: 100_000_000)

        XCTAssertEqual(engine.runSyncTriggers.filter { $0 == .autoFilesystem }.count, 2)
    }

    @MainActor
    func testAutoSyncQueuesOnePendingRunWhileInFlight() async {
        let startSecondRun = expectation(description: "second sync started")
        var invocationCount = 0
        let engine = MockSyncEngine(
            onRunSync: { _ in
                invocationCount += 1
                if invocationCount == 1 {
                    try? await Task.sleep(nanoseconds: 80_000_000)
                } else if invocationCount == 2 {
                    startSecondRun.fulfill()
                }
                return .empty
            },
            onDelete: { _ in .empty }
        )
        let coordinator = MockAutoSyncCoordinator()
        let viewModel = AppViewModel(
            makeEngine: { engine },
            makeAutoSyncCoordinator: { callback in
                coordinator.onEvent = callback
                return coordinator
            },
            autoSyncDebounceSeconds: 0.01
        )

        viewModel.start()
        coordinator.emit(.skillsFilesystemChanged)
        coordinator.emit(.skillsFilesystemChanged)
        coordinator.emit(.skillsFilesystemChanged)
        await fulfillment(of: [startSecondRun], timeout: 1.0)

        XCTAssertEqual(engine.runSyncTriggers.filter { $0 == .autoFilesystem }.count, 2)
    }

    @MainActor
    func testStopStopsAutoSyncCoordinator() {
        let coordinator = MockAutoSyncCoordinator()
        let viewModel = AppViewModel(
            makeEngine: { MockSyncEngine(onDelete: { _ in .empty }) },
            makeAutoSyncCoordinator: { callback in
                coordinator.onEvent = callback
                return coordinator
            }
        )

        viewModel.start()
        viewModel.stop()

        XCTAssertEqual(coordinator.stopCalls, 1)
    }

    private func makeSkill(
        id: String,
        name: String,
        scope: String,
        workspace: String? = nil,
        sourcePath: String? = nil,
        status: SkillLifecycleStatus = .active
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
            symlinkTarget: "/tmp/\(id)",
            status: status,
            archivedAt: status == .archived ? "2026-02-20T12:00:00Z" : nil,
            archivedBundlePath: status == .archived ? "/tmp/archive/\(id)" : nil,
            archivedOriginalScope: status == .archived ? "global" : nil,
            archivedOriginalWorkspace: nil
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
    private let onRunSync: (SyncTrigger) async throws -> SyncState
    private let onDelete: (SkillRecord) async throws -> SyncState
    private let onArchive: (SkillRecord) async throws -> SyncState
    private let onRestore: (SkillRecord) async throws -> SyncState
    private let onMakeGlobal: (SkillRecord) async throws -> SyncState
    private let onRename: (SkillRecord, String) async throws -> SyncState
    private let onApplyValidationFix: (SkillRecord, SkillValidationIssue) async throws -> SyncState
    private(set) var runSyncTriggers: [SyncTrigger] = []

    init(
        onRunSync: @escaping (SyncTrigger) async throws -> SyncState = { _ in .empty },
        onDelete: @escaping (SkillRecord) async throws -> SyncState,
        onArchive: @escaping (SkillRecord) async throws -> SyncState = { _ in .empty },
        onRestore: @escaping (SkillRecord) async throws -> SyncState = { _ in .empty },
        onMakeGlobal: @escaping (SkillRecord) async throws -> SyncState = { _ in .empty },
        onRename: @escaping (SkillRecord, String) async throws -> SyncState = { _, _ in .empty },
        onApplyValidationFix: @escaping (SkillRecord, SkillValidationIssue) async throws -> SyncState = { _, _ in .empty }
    ) {
        self.onRunSync = onRunSync
        self.onDelete = onDelete
        self.onArchive = onArchive
        self.onRestore = onRestore
        self.onMakeGlobal = onMakeGlobal
        self.onRename = onRename
        self.onApplyValidationFix = onApplyValidationFix
    }

    func runSync(trigger: SyncTrigger) async throws -> SyncState {
        runSyncTriggers.append(trigger)
        return try await onRunSync(trigger)
    }

    func openInZed(skill: SkillRecord) throws { }

    func revealInFinder(skill: SkillRecord) throws { }

    func deleteCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onDelete(skill)
    }

    func archiveCanonicalSource(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onArchive(skill)
    }

    func restoreArchivedSkillToGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onRestore(skill)
    }

    func makeGlobal(skill: SkillRecord, confirmed: Bool) async throws -> SyncState {
        try await onMakeGlobal(skill)
    }

    func renameSkill(skill: SkillRecord, newTitle: String) async throws -> SyncState {
        try await onRename(skill, newTitle)
    }

    func applyValidationFix(skill: SkillRecord, issue: SkillValidationIssue) async throws -> SyncState {
        try await onApplyValidationFix(skill, issue)
    }
}

private final class MockAutoSyncCoordinator: AutoSyncCoordinating {
    var onEvent: ((AutoSyncEvent) -> Void)?
    private(set) var startCalls = 0
    private(set) var stopCalls = 0
    private(set) var refreshCalls = 0

    func start() {
        startCalls += 1
    }

    func stop() {
        stopCalls += 1
    }

    func refreshWatchedPaths() {
        refreshCalls += 1
    }

    func emit(_ event: AutoSyncEvent) {
        onEvent?(event)
    }
}
