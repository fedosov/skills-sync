import XCTest
@testable import SkillsSyncApp

final class AppTypographyTests: XCTestCase {
    func testAppTextRoleMapsToExpectedSwiftUITextStyles() {
        XCTAssertEqual(AppTextRole.title.spec, AppTextSpec(textStyle: .title3, weight: .semibold, monospaced: false))
        XCTAssertEqual(AppTextRole.sectionHeader.spec, AppTextSpec(textStyle: .headline, weight: nil, monospaced: false))
        XCTAssertEqual(AppTextRole.body.spec, AppTextSpec(textStyle: .body, weight: nil, monospaced: false))
        XCTAssertEqual(AppTextRole.secondary.spec, AppTextSpec(textStyle: .subheadline, weight: nil, monospaced: false))
        XCTAssertEqual(AppTextRole.meta.spec, AppTextSpec(textStyle: .caption, weight: nil, monospaced: false))
        XCTAssertEqual(AppTextRole.pathMono.spec, AppTextSpec(textStyle: .footnote, weight: nil, monospaced: true))
    }

    func testSpacingScaleMatchesDesignSystemValues() {
        XCTAssertEqual(AppSpacing.xs, 4)
        XCTAssertEqual(AppSpacing.sm, 8)
        XCTAssertEqual(AppSpacing.md, 12)
        XCTAssertEqual(AppSpacing.lg, 16)
        XCTAssertEqual(AppSpacing.xl, 24)
    }

    func testPathRoleUsesMonospacedStyleByContract() {
        XCTAssertEqual(AppTextRole.pathMono.spec.textStyle, .footnote)
        XCTAssertNil(AppTextRole.pathMono.spec.weight)
        XCTAssertTrue(AppTextRole.pathMono.spec.monospaced)
    }

    func testNoCaption2ForPrimaryPathRendering() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let appFile = repoRoot.appendingPathComponent("Sources/App/SkillsSyncApp.swift")
        let source = try String(contentsOf: appFile, encoding: .utf8)

        XCTAssertFalse(source.contains("caption2.monospaced"))
    }
}
