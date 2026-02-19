import XCTest
@testable import SkillsSyncApp

final class SyncPresentationTests: XCTestCase {
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
            topSkills: [],
            lastCommandResult: nil
        )

        viewModel.scopeFilter = .all
        XCTAssertEqual(Set(viewModel.filteredSkills.map(\.id)), Set(["g-1", "p-1"]))

        viewModel.scopeFilter = .global
        XCTAssertEqual(viewModel.filteredSkills.map(\.id), ["g-1"])

        viewModel.scopeFilter = .project
        XCTAssertEqual(viewModel.filteredSkills.map(\.id), ["p-1"])
    }

    private func makeSkill(id: String, name: String, scope: String) -> SkillRecord {
        SkillRecord(
            id: id,
            name: name,
            scope: scope,
            workspace: scope == "project" ? "/tmp/project" : nil,
            canonicalSourcePath: "/tmp/\(id)",
            targetPaths: ["/tmp/target/\(id)"],
            exists: true,
            isSymlinkCanonical: true,
            packageType: "dir",
            skillKey: name.lowercased(),
            symlinkTarget: "/tmp/\(id)"
        )
    }
}
