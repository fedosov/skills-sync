import Foundation
import XCTest
@testable import SkillsSyncApp

final class SkillsSyncSharedTests: XCTestCase {
    private var tempDir: URL!
    private var store: SyncStateStore!

    override func setUpWithError() throws {
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        setenv("SKILLS_SYNC_GROUP_DIR", tempDir.path, 1)
        store = SyncStateStore()
    }

    override func tearDownWithError() throws {
        unsetenv("SKILLS_SYNC_GROUP_DIR")
        try? FileManager.default.removeItem(at: tempDir)
        store = nil
    }

    func testLoadStateDecoding() throws {
        let payload = """
        {
          "version": 1,
          "generated_at": "2026-01-01T00:00:00Z",
          "sync": {
            "status": "ok",
            "last_started_at": "2026-01-01T00:00:00Z",
            "last_finished_at": "2026-01-01T00:00:05Z",
            "duration_ms": 5000,
            "error": null
          },
          "summary": {
            "global_count": 2,
            "project_count": 3,
            "conflict_count": 0
          },
          "skills": [
            {
              "id": "skill-1",
              "name": "alpha",
              "scope": "global",
              "workspace": null,
              "canonical_source_path": "/tmp/alpha",
              "target_paths": ["/tmp/t1"],
              "exists": true,
              "is_symlink_canonical": false,
              "package_type": "dir",
              "skill_key": "alpha",
              "symlink_target": "/tmp/alpha"
            }
          ],
          "top_skills": ["skill-1"]
        }
        """

        let data = try XCTUnwrap(payload.data(using: .utf8))
        try data.write(to: SyncPaths.stateURL)
        let state = store.loadState()

        XCTAssertEqual(state.version, 1)
        XCTAssertEqual(state.summary.globalCount, 2)
        XCTAssertEqual(state.skills.count, 1)
        XCTAssertEqual(state.topSkills.first, "skill-1")
    }

    func testDeepLinkRoutingParsesSkillDetailsURL() {
        let route = DeepLinkParser.parse(URL(string: "skillssync://skill?id=abc-123")!)
        XCTAssertEqual(route, .skill(id: "abc-123"))
    }

    func testDeepLinkRoutingParsesOpenURL() {
        let route = DeepLinkParser.parse(URL(string: "skillssync://open")!)
        XCTAssertEqual(route, .open)
    }

    func testDeepLinkRoutingRejectsUnknownScheme() {
        let route = DeepLinkParser.parse(URL(string: "https://example.com")!)
        XCTAssertNil(route)
    }

    func testTopSkillsUsesPreferredAndFallbackUpToSix() {
        let skills = [
            makeSkill(id: "g1", name: "Alpha", scope: "global"),
            makeSkill(id: "g2", name: "Beta", scope: "global"),
            makeSkill(id: "g3", name: "Gamma", scope: "global"),
            makeSkill(id: "p1", name: "Delta", scope: "project"),
            makeSkill(id: "p2", name: "Epsilon", scope: "project"),
            makeSkill(id: "p3", name: "Zeta", scope: "project"),
            makeSkill(id: "p4", name: "Eta", scope: "project"),
        ]

        let state = SyncState(
            version: 1,
            generatedAt: "2026-01-01T00:00:00Z",
            sync: .empty,
            summary: .empty,
            skills: skills,
            topSkills: ["p2", "missing", "g3"]
        )

        let top = store.topSkills(from: state)

        XCTAssertEqual(top.count, 6)
        XCTAssertEqual(top.first?.id, "p2")
        XCTAssertEqual(top[1].id, "g3")
        XCTAssertTrue(top.contains(where: { $0.id == "g1" }))
        XCTAssertTrue(top.contains(where: { $0.id == "g2" }))
    }

    func testSyncPathsFallbackUsesApplicationSupportDirectory() {
        let fallback = SyncPaths.storageDirectoryURL.path
        XCTAssertTrue(fallback.contains("/Library/Application Support/SkillsSync"))
        XCTAssertFalse(fallback.contains("/.config/ai-agents/skillssync"))
    }

    func testSkillTitlePriorityTitleThenNameThenH1ThenRecordName() throws {
        let parser = SkillPreviewParser()

        let titleDir = tempDir.appendingPathComponent("skill-title", isDirectory: true)
        try writeFile(titleDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        title: Fancy Title
        name: from-name
        ---

        # Heading Title
        """)
        let titlePreview = parser.parse(skill: makeSkill(id: "s1", name: "record-name", scope: "global", sourcePath: titleDir.path))
        XCTAssertEqual(titlePreview.displayTitle, "Fancy Title")

        let nameDir = tempDir.appendingPathComponent("skill-name", isDirectory: true)
        try writeFile(nameDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        name: Name Only
        ---

        # Heading Title
        """)
        let namePreview = parser.parse(skill: makeSkill(id: "s2", name: "record-name", scope: "global", sourcePath: nameDir.path))
        XCTAssertEqual(namePreview.displayTitle, "Name Only")

        let h1Dir = tempDir.appendingPathComponent("skill-h1", isDirectory: true)
        try writeFile(h1Dir.appendingPathComponent("SKILL.md"), contents: """
        # Heading Only

        body
        """)
        let h1Preview = parser.parse(skill: makeSkill(id: "s3", name: "record-name", scope: "global", sourcePath: h1Dir.path))
        XCTAssertEqual(h1Preview.displayTitle, "Heading Only")

        let fallbackDir = tempDir.appendingPathComponent("skill-fallback", isDirectory: true)
        try writeFile(fallbackDir.appendingPathComponent("SKILL.md"), contents: "body without heading")
        let fallbackPreview = parser.parse(skill: makeSkill(id: "s4", name: "record-name", scope: "global", sourcePath: fallbackDir.path))
        XCTAssertEqual(fallbackPreview.displayTitle, "record-name")
    }

    func testParseFrontmatterHeaderExtractsKnownKeysAndIntro() throws {
        let parser = SkillPreviewParser()
        let skillDir = tempDir.appendingPathComponent("skill-header", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        title: Header Title
        name: fallback-name
        description: One-line description.
        source: https://example.com/skill
        risk: safe
        ---

        # Main Header

        First intro paragraph for preview.

        ## Next Section
        """)

        let preview = parser.parse(skill: makeSkill(id: "s5", name: "record-name", scope: "global", sourcePath: skillDir.path))

        XCTAssertEqual(preview.header?.title, "Header Title")
        XCTAssertEqual(preview.header?.description, "One-line description.")
        XCTAssertEqual(preview.header?.intro, "First intro paragraph for preview.")
        XCTAssertEqual(preview.header?.metadata.first(where: { $0.key == "risk" })?.value, "safe")
        XCTAssertEqual(preview.header?.metadata.first(where: { $0.key == "source" })?.value, "https://example.com/skill")
    }

    func testTreeBuildLimitsToThreeLevelsAndAddsMoreNode() throws {
        let parser = SkillPreviewParser()
        let skillDir = tempDir.appendingPathComponent("skill-tree", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: "# Root")
        try writeFile(skillDir.appendingPathComponent("a/b/c/d/deep.txt"), contents: "deep")
        try writeFile(skillDir.appendingPathComponent("a/b/c/peer.txt"), contents: "peer")

        let preview = parser.parse(skill: makeSkill(id: "s6", name: "record-name", scope: "global", sourcePath: skillDir.path))
        let thirdLevel = try XCTUnwrap(preview.tree?.children.first(where: { $0.name == "a" })?.children.first(where: { $0.name == "b" })?.children.first(where: { $0.name == "c" }))
        XCTAssertTrue(thirdLevel.children.contains(where: { $0.name.contains("more") }))
    }

    func testExtractContentRelationsFindsBacktickPathsAndMarkdownLinksAndOpenPattern() throws {
        let parser = SkillPreviewParser()
        let skillDir = tempDir.appendingPathComponent("skill-rel", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("resources/implementation-playbook.md"), contents: "res")
        try writeFile(skillDir.appendingPathComponent("references/guide.md"), contents: "ref")
        try writeFile(skillDir.appendingPathComponent("scripts/run.sh"), contents: "echo run")
        try writeFile(skillDir.appendingPathComponent("assets/logo.svg"), contents: "<svg/>")
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        name: rel-skill
        ---

        Use `resources/implementation-playbook.md`.
        Read [guide](references/guide.md).
        Then open scripts/run.sh.
        Asset: `assets/logo.svg`.
        Missing: `resources/missing.md`.
        """)

        let preview = parser.parse(skill: makeSkill(id: "s7", name: "record-name", scope: "global", sourcePath: skillDir.path))
        let contentTargets = Set(preview.relations.filter { $0.kind == .content }.map(\.to))

        XCTAssertTrue(contentTargets.contains("resources/implementation-playbook.md"))
        XCTAssertTrue(contentTargets.contains("references/guide.md"))
        XCTAssertTrue(contentTargets.contains("scripts/run.sh"))
        XCTAssertTrue(contentTargets.contains("assets/logo.svg"))
        XCTAssertFalse(contentTargets.contains("resources/missing.md"))
    }

    func testPreviewFallsBackWhenSkillFileMissing() {
        let parser = SkillPreviewParser()
        let skillDir = tempDir.appendingPathComponent("skill-missing", isDirectory: true)
        try? FileManager.default.createDirectory(at: skillDir, withIntermediateDirectories: true)
        let skill = makeSkill(
            id: "s8",
            name: "record-name",
            scope: "global",
            sourcePath: skillDir.path
        )

        let preview = parser.parse(skill: skill)

        XCTAssertEqual(preview.displayTitle, "record-name")
        XCTAssertNil(preview.header)
        XCTAssertNotNil(preview.previewUnavailableReason)
        XCTAssertTrue(preview.relations.contains(where: { $0.kind == .symlink }))
    }

    func testSkillValidatorReturnsNoIssuesForValidSkill() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-valid", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("resources/guide.md"), contents: "guide")
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        title: Valid Skill
        ---

        # Valid Skill

        Read `resources/guide.md`.
        """)

        let result = validator.validate(skill: makeSkill(id: "sv1", name: "valid", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.isEmpty)
        XCTAssertFalse(result.hasWarnings)
    }

    func testSkillValidatorReportsMissingSkillFileForDirectoryPackage() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-missing", isDirectory: true)
        try FileManager.default.createDirectory(at: skillDir, withIntermediateDirectories: true)

        let result = validator.validate(skill: makeSkill(id: "sv2", name: "missing", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "missing_skill_md" }))
        XCTAssertTrue(result.hasWarnings)
    }

    func testSkillValidatorReportsEmptyMainFile() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-empty", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: "   \n \n")

        let result = validator.validate(skill: makeSkill(id: "sv3", name: "empty", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "empty_main_file" }))
    }

    func testSkillValidatorReportsMissingTitleWhenNoFrontmatterNameOrHeading() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-no-title", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: """
        Just body text without heading.
        """)

        let result = validator.validate(skill: makeSkill(id: "sv4", name: "no-title", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "missing_title" }))
    }

    func testSkillValidatorReportsBrokenLocalReferences() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-broken-ref", isDirectory: true)
        try writeFile(skillDir.appendingPathComponent("SKILL.md"), contents: """
        ---
        title: Broken Ref Skill
        ---

        # Broken Ref Skill

        Read `resources/missing.md`.
        """)

        let result = validator.validate(skill: makeSkill(id: "sv5", name: "broken", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "broken_reference" }))
        let issue = try XCTUnwrap(result.issues.first(where: { $0.code == "broken_reference" }))
        XCTAssertTrue(issue.message.contains("resources/missing.md"))
        XCTAssertEqual(issue.line, 7)
        XCTAssertEqual(issue.source, skillDir.appendingPathComponent("SKILL.md").path)
        XCTAssertFalse(issue.details.isEmpty)
    }

    func testSkillValidatorReportsBrokenSkillMDSymlinkWithTargetPath() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-broken-symlink", isDirectory: true)
        try FileManager.default.createDirectory(at: skillDir, withIntermediateDirectories: true)
        let missingTarget = tempDir.appendingPathComponent("legacy/missing-skill.md")
        try FileManager.default.createDirectory(at: missingTarget.deletingLastPathComponent(), withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            at: skillDir.appendingPathComponent("SKILL.md"),
            withDestinationURL: missingTarget
        )

        let result = validator.validate(skill: makeSkill(id: "sv6", name: "broken-symlink", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "broken_skill_md_symlink" }))
        XCTAssertTrue(result.issues.contains(where: { $0.details.contains(missingTarget.path) }))
    }

    func testSkillValidatorReportsSkillMDIsSymlinkWhenTargetExists() throws {
        let validator = SkillValidator()
        let skillDir = tempDir.appendingPathComponent("validator-live-symlink", isDirectory: true)
        try FileManager.default.createDirectory(at: skillDir, withIntermediateDirectories: true)
        let liveTarget = tempDir.appendingPathComponent("legacy/live-skill.md")
        try writeFile(liveTarget, contents: """
        ---
        title: Via Symlink
        ---

        # Via Symlink
        """)
        try FileManager.default.createSymbolicLink(
            at: skillDir.appendingPathComponent("SKILL.md"),
            withDestinationURL: liveTarget
        )

        let result = validator.validate(skill: makeSkill(id: "sv7", name: "live-symlink", scope: "global", sourcePath: skillDir.path))

        XCTAssertTrue(result.issues.contains(where: { $0.code == "skill_md_is_symlink" }))
    }

    func testRepairPromptBuilderIncludesSkillIdentityAndIssue() {
        let skill = makeSkill(
            id: "sv8",
            name: "agent-helper",
            scope: "global",
            sourcePath: "/tmp/agent-helper"
        )
        let issue = SkillValidationIssue(
            code: "broken_reference",
            message: "Broken reference: agents/flow.md",
            source: "/tmp/agent-helper/SKILL.md",
            line: 12,
            details: "Referenced path does not exist in this skill package."
        )

        let prompt = SkillRepairPromptBuilder.prompt(for: skill, issue: issue)

        XCTAssertTrue(prompt.contains("Skill: agent-helper"))
        XCTAssertTrue(prompt.contains("Skill key: agent-helper"))
        XCTAssertTrue(prompt.contains("Issue (broken_reference): Broken reference: agents/flow.md"))
        XCTAssertTrue(prompt.contains("Issue source: /tmp/agent-helper/SKILL.md:12"))
        XCTAssertTrue(prompt.contains("Issue details: Referenced path does not exist in this skill package."))
        XCTAssertTrue(prompt.contains("Please investigate and repair this skill package."))
    }

    func testRepairPromptBuilderIncludesCanonicalPathAndScope() {
        let skill = SkillRecord(
            id: "sv9",
            name: "proj-skill",
            scope: "project",
            workspace: "/tmp/workspace-a",
            canonicalSourcePath: "/tmp/workspace-a/.claude/skills/proj-skill",
            targetPaths: ["/tmp/workspace-a/.agents/skills/proj-skill"],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: "proj-skill",
            symlinkTarget: "/tmp/workspace-a/.claude/skills/proj-skill"
        )
        let issue = SkillValidationIssue(
            code: "missing_title",
            message: "No title found."
        )

        let prompt = SkillRepairPromptBuilder.prompt(for: skill, issue: issue)

        XCTAssertTrue(prompt.contains("Scope: project"))
        XCTAssertTrue(prompt.contains("Canonical path: /tmp/workspace-a/.claude/skills/proj-skill"))
        XCTAssertTrue(prompt.contains("Workspace: /tmp/workspace-a"))
    }

    func testPreferencesStoreDecodesLegacyV1Settings() throws {
        let payload = """
        {
          "version": 1,
          "auto_migrate_to_canonical_source": true
        }
        """
        try XCTUnwrap(payload.data(using: .utf8)).write(to: SyncPaths.appSettingsURL)

        let settings = SyncPreferencesStore().loadSettings()

        XCTAssertEqual(settings.version, 1)
        XCTAssertTrue(settings.autoMigrateToCanonicalSource)
        XCTAssertNil(settings.windowState)
        XCTAssertNil(settings.uiState)
    }

    func testPreferencesStorePersistsV2WindowAndUIState() {
        let store = SyncPreferencesStore()
        let settings = SyncAppSettings(
            version: 2,
            autoMigrateToCanonicalSource: true,
            windowState: AppWindowState(x: 11, y: 22, width: 1200, height: 800, isMaximized: false),
            uiState: AppUIState(
                sidebarWidth: 401,
                scopeFilter: ScopeFilter.project.rawValue,
                searchText: "alpha",
                selectedSkillIDs: ["a", "b"]
            )
        )

        store.saveSettings(settings)
        let loaded = store.loadSettings()

        XCTAssertEqual(loaded.version, 2)
        XCTAssertEqual(loaded, settings)
    }

    func testWindowStateGeometryHelpersClampAndRejectInvalidFrames() {
        XCTAssertEqual(WindowStateGeometry.clampSidebarWidth(100), 300)
        XCTAssertEqual(WindowStateGeometry.clampSidebarWidth(390), 390)
        XCTAssertEqual(WindowStateGeometry.clampSidebarWidth(800), 420)
        XCTAssertEqual(WindowStateGeometry.clampSidebarWidth(nil), nil)

        let invalid = AppWindowState(x: 0, y: 0, width: 100, height: 100, isMaximized: false)
        XCTAssertNil(WindowStateGeometry.validFrameRect(from: invalid, screensVisibleFrames: []))
    }

    private func makeSkill(id: String, name: String, scope: String, sourcePath: String? = nil) -> SkillRecord {
        SkillRecord(
            id: id,
            name: name,
            scope: scope,
            workspace: scope == "project" ? "/tmp/project" : nil,
            canonicalSourcePath: sourcePath ?? "/tmp/\(id)",
            targetPaths: ["/tmp/target/\(id)"],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: name.lowercased(),
            symlinkTarget: "/tmp/\(id)"
        )
    }

    private func writeFile(_ path: URL, contents: String) throws {
        try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try XCTUnwrap(contents.data(using: .utf8)).write(to: path)
    }
}
