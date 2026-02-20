import XCTest
@testable import SkillsSyncApp

final class SkillDetailViewTests: XCTestCase {
    func testValidationExpandedByDefaultWhenWarningsExist() {
        let expanded = SkillDetailPresentation.defaultValidationExpanded(issuesCount: 2)
        XCTAssertTrue(expanded)
    }

    func testValidationCollapsedByDefaultWhenNoWarnings() {
        let expanded = SkillDetailPresentation.defaultValidationExpanded(issuesCount: 0)
        XCTAssertFalse(expanded)
    }

    func testDeepDiveCollapsedByDefault() {
        XCTAssertFalse(SkillDetailPresentation.defaultDeepDiveExpanded)
    }

    func testValidationSummaryUsesPositiveMessageWhenNoIssues() {
        let summary = SkillDetailPresentation.validationSummary(issuesCount: 0)
        XCTAssertEqual(summary.text, "No validation warnings")
        XCTAssertEqual(summary.symbol, "checkmark.circle.fill")
    }

    func testValidationSummaryUsesWarningMessageWhenIssuesExist() {
        let summary = SkillDetailPresentation.validationSummary(issuesCount: 3)
        XCTAssertEqual(summary.text, "3 validation issue(s)")
        XCTAssertEqual(summary.symbol, "exclamationmark.triangle.fill")
    }

    func testSkillDetailsModuleContainsRiskPanelAndRecoveryCopy() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let detailFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillDetailView.swift")
        let riskFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillRiskPanel.swift")
        let detailSource = try String(contentsOf: detailFile, encoding: .utf8)
        let riskSource = try String(contentsOf: riskFile, encoding: .utf8)

        XCTAssertTrue(detailSource.contains("SkillRiskPanel"))
        XCTAssertTrue(riskSource.contains("What happens next"))
        XCTAssertTrue(riskSource.contains("Archive keeps a recoverable copy"))
        XCTAssertTrue(riskSource.contains("Permanently delete archived skill source to system Trash"))
    }

    func testSkillOverviewUsesPromptFieldWithoutDuplicateTitleLabel() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let overviewFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillOverviewCard.swift")
        let source = try String(contentsOf: overviewFile, encoding: .utf8)

        XCTAssertTrue(source.contains("TextField(\"\", text: $editableTitle, prompt: Text(\"Title\"))"))
        XCTAssertFalse(source.contains("TextField(\"Title\", text: $editableTitle)"))
    }

    func testSkillOverviewDoesNotRenderActiveAsDefaultStatusChip() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let overviewFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillOverviewCard.swift")
        let source = try String(contentsOf: overviewFile, encoding: .utf8)

        XCTAssertFalse(source.contains("label: skill.status == .archived ? \"Archived\" : \"Active\""))
        XCTAssertFalse(source.contains("Active is gray because it is informational: this skill is healthy and does not require action."))
        XCTAssertFalse(source.contains("Gray means informational status. Active is normal and requires no action."))
    }

    func testDeepDiveUsesFullRowButtonToggle() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let detailFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillDetailView.swift")
        let source = try String(contentsOf: detailFile, encoding: .utf8)

        XCTAssertTrue(source.contains("Button {"))
        XCTAssertTrue(source.contains("contentShape(Rectangle())"))
        XCTAssertTrue(source.contains(".buttonStyle(.plain)"))
        XCTAssertFalse(source.contains("DisclosureGroup(isExpanded: $isDeepDiveExpanded)"))
    }

    func testValidationPanelShowsCodexVisibilityLabelForCodexIssues() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let panelFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillValidationPanel.swift")
        let source = try String(contentsOf: panelFile, encoding: .utf8)

        XCTAssertTrue(source.contains("Codex visibility"))
        XCTAssertTrue(source.contains("hasPrefix(\"codex_\")"))
        XCTAssertTrue(source.contains("Button(\"Fix\")"))
        XCTAssertFalse(source.contains("Repair for Codex"))
    }

    func testOverviewCardContainsCodexVisibilityChips() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let overviewFile = repoRoot.appendingPathComponent("Sources/App/SkillDetails/SkillOverviewCard.swift")
        let source = try String(contentsOf: overviewFile, encoding: .utf8)

        XCTAssertTrue(source.contains("Codex visible"))
        XCTAssertTrue(source.contains("Codex hidden"))
    }
}
