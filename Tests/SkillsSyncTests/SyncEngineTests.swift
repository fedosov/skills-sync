import Foundation
import XCTest
@testable import SkillsSyncApp

final class SyncEngineTests: XCTestCase {
    private var tempDir: URL!
    private var homeDir: URL!
    private var runtimeDir: URL!
    private var store: SyncStateStore!

    override func setUpWithError() throws {
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        homeDir = tempDir.appendingPathComponent("home", isDirectory: true)
        runtimeDir = tempDir.appendingPathComponent("runtime", isDirectory: true)
        try FileManager.default.createDirectory(at: homeDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: runtimeDir, withIntermediateDirectories: true)

        setenv("SKILLS_SYNC_GROUP_DIR", runtimeDir.path, 1)
        store = SyncStateStore()
    }

    override func tearDownWithError() throws {
        unsetenv("SKILLS_SYNC_GROUP_DIR")
        SyncEngineEnvironment.testOverride = nil
        try? FileManager.default.removeItem(at: tempDir)
        store = nil
    }

    func testRunSyncBuildsStateAndPersistsIt() async throws {
        try writeSkill(root: path(".claude/skills"), key: "alpha", body: "A")
        try writeSkill(root: path(".agents/skills"), key: "beta", body: "B")
        try writeSkill(root: path("Dev/workspace-a/.claude/skills"), key: "project-1", body: "P")

        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(state.summary.globalCount, 2)
        XCTAssertEqual(state.summary.projectCount, 1)
        XCTAssertEqual(state.summary.conflictCount, 0)
        XCTAssertEqual(state.skills.count, 3)
        XCTAssertEqual(store.loadState().skills.count, 3)
    }

    func testRunSyncDiscoversProjectSkillInCodexSkillsDirectory() async throws {
        try writeSkill(root: path("Dev/workspace-a/.codex/skills"), key: "project-codex", body: "PC")
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(state.summary.projectCount, 1)
        let project = try XCTUnwrap(state.skills.first(where: { $0.skillKey == "project-codex" }))
        XCTAssertEqual(project.scope, "project")
        XCTAssertEqual(
            URL(fileURLWithPath: project.workspace ?? "").standardizedFileURL.path,
            path("Dev/workspace-a").standardizedFileURL.path
        )
    }

    func testRunSyncCreatesProjectCodexTargetSymlinkForCanonicalProjectSkill() async throws {
        try writeSkill(root: path("Dev/workspace-a/.claude/skills"), key: "project-shared", body: "same")
        try writeSkill(root: path("Dev/workspace-a/.codex/skills"), key: "project-shared", body: "same")
        configureEngine()

        let engine = SyncEngine()
        _ = try await engine.runSync(trigger: .manual)

        let codexPath = path("Dev/workspace-a/.codex/skills/project-shared")
        let claudePath = path("Dev/workspace-a/.claude/skills/project-shared")
        XCTAssertTrue(codexPath.isTestSymlink)
        let destination = URL(fileURLWithPath: try FileManager.default.destinationOfSymbolicLink(atPath: codexPath.path)).standardizedFileURL.path
        XCTAssertEqual(destination, claudePath.standardizedFileURL.path)
    }

    func testRunSyncDiscoversWorkspaceFromConfiguredCustomRoot() async throws {
        let externalWorkspace = path("external/custom-root/workspace-z")
        try writeSkill(root: externalWorkspace.appendingPathComponent(".claude/skills", isDirectory: true), key: "custom-root-skill", body: "CR")
        try writeSettings(autoMigrate: false, workspaceDiscoveryRoots: [path("external/custom-root").path])
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertTrue(state.skills.contains(where: { $0.skillKey == "custom-root-skill" && $0.scope == "project" }))
    }

    func testRunSyncScansConfiguredCustomRootRecursivelyToDepthThree() async throws {
        let level3Workspace = path("external/depth-root/one/two/workspace-depth3")
        try writeSkill(root: level3Workspace.appendingPathComponent(".claude/skills", isDirectory: true), key: "depth-3", body: "D3")
        try writeSettings(autoMigrate: false, workspaceDiscoveryRoots: [path("external/depth-root").path])
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertTrue(state.skills.contains(where: { $0.skillKey == "depth-3" }))
    }

    func testRunSyncDoesNotScanConfiguredCustomRootDeeperThanDepthThree() async throws {
        let tooDeepWorkspace = path("external/depth-root/one/two/three/workspace-depth4")
        try writeSkill(root: tooDeepWorkspace.appendingPathComponent(".claude/skills", isDirectory: true), key: "depth-4", body: "D4")
        try writeSettings(autoMigrate: false, workspaceDiscoveryRoots: [path("external/depth-root").path])
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertFalse(state.skills.contains(where: { $0.skillKey == "depth-4" }))
    }

    func testRunSyncMarksFailedOnConflictAndWritesConflictCount() async throws {
        try writeSkill(root: path(".claude/skills"), key: "duplicate", body: "first")
        try writeSkill(root: path(".agents/skills"), key: "duplicate", body: "second")
        configureEngine()

        let engine = SyncEngine()

        do {
            _ = try await engine.runSync(trigger: .manual)
            XCTFail("Expected conflict")
        } catch {
            let state = store.loadState()
            XCTAssertEqual(state.sync.status, .failed)
            XCTAssertEqual(state.summary.conflictCount, 1)
        }
    }

    func testRunSyncIgnoresLegacyGlobalDirectory() async throws {
        try writeLegacySkill(name: "legacy-only", body: "legacy")
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(state.summary.globalCount, 0)
        XCTAssertTrue(state.skills.isEmpty)
    }

    func testRunSyncDoesNotUseLegacyForConflictOrPriority() async throws {
        try writeSkill(root: path(".claude/skills"), key: "shared", body: "canonical")
        try writeLegacySkill(name: "shared", body: "legacy-different")
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(state.summary.conflictCount, 0)
        XCTAssertEqual(state.summary.globalCount, 1)
        XCTAssertEqual(
            URL(fileURLWithPath: state.skills.first?.canonicalSourcePath ?? "").standardizedFileURL.path,
            path(".claude/skills/shared").standardizedFileURL.path
        )
    }

    func testRunSyncWhenAutoMigrationOffKeepsCurrentCanonicalSelection() async throws {
        try writeSkill(root: path(".claude/skills"), key: "shared", body: "same")
        try writeSkill(root: path(".codex/skills"), key: "shared", body: "same")
        try writeAutoMigrationPreference(enabled: false)
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        let canonical = try XCTUnwrap(state.skills.first(where: { $0.skillKey == "shared" }))
        XCTAssertEqual(
            URL(fileURLWithPath: canonical.canonicalSourcePath).standardizedFileURL.path,
            path(".claude/skills/shared").standardizedFileURL.path
        )
    }

    func testRunSyncWhenAutoMigrationOnMovesCodexOnlySkillToClaudeAndCreatesSymlinkBack() async throws {
        try writeSkill(root: path(".codex/skills"), key: "alpha", body: "A")
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        let canonical = try XCTUnwrap(state.skills.first(where: { $0.skillKey == "alpha" }))
        let claudePath = path(".claude/skills/alpha")
        let codexPath = path(".codex/skills/alpha")

        XCTAssertEqual(
            URL(fileURLWithPath: canonical.canonicalSourcePath).standardizedFileURL.path,
            claudePath.standardizedFileURL.path
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: claudePath.path))
        XCTAssertTrue(codexPath.isTestSymlink)
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: codexPath.path), claudePath.path)
    }

    func testRunSyncWhenAutoMigrationOnConvertsAgentsSourceToSymlinkWhenClaudeExists() async throws {
        try writeSkill(root: path(".claude/skills"), key: "beta", body: "B")
        try writeSkill(root: path(".agents/skills"), key: "beta", body: "B")
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        _ = try await engine.runSync(trigger: .manual)

        let agentsPath = path(".agents/skills/beta")
        let claudePath = path(".claude/skills/beta")
        XCTAssertTrue(agentsPath.isTestSymlink)
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: agentsPath.path), claudePath.path)
    }

    func testRunSyncWhenAutoMigrationOnFailsWholeSyncOnMigrationError() async throws {
        try writeSkill(root: path(".codex/skills"), key: "gamma", body: "G")
        let blockingFile = path(".claude/skills/gamma")
        try FileManager.default.createDirectory(at: blockingFile.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("blocked".utf8).write(to: blockingFile)
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.runSync(trigger: .manual)
        } assertion: { error in
            XCTAssertTrue(error.localizedDescription.localizedCaseInsensitiveContains("migration"))
        }

        XCTAssertEqual(store.loadState().sync.status, .failed)
    }

    func testRunSyncWhenAutoMigrationOnReplacesBrokenCanonicalSymlink() async throws {
        try writeSkill(root: path(".agents/skills"), key: "accessibility-compliance-accessibility-audit", body: "A11Y")
        let staleTarget = path(".agents/skills/missing-accessibility-compliance-accessibility-audit")
        let claudePath = path(".claude/skills/accessibility-compliance-accessibility-audit")
        try FileManager.default.createDirectory(at: claudePath.deletingLastPathComponent(), withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: claudePath, withDestinationURL: staleTarget)
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)
        let canonical = try XCTUnwrap(state.skills.first(where: { $0.skillKey == "accessibility-compliance-accessibility-audit" }))

        XCTAssertEqual(
            URL(fileURLWithPath: canonical.canonicalSourcePath).standardizedFileURL.path,
            claudePath.standardizedFileURL.path
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: claudePath.path))
        XCTAssertTrue(path(".agents/skills/accessibility-compliance-accessibility-audit").isTestSymlink)
        XCTAssertEqual(
            try FileManager.default.destinationOfSymbolicLink(atPath: path(".agents/skills/accessibility-compliance-accessibility-audit").path),
            claudePath.path
        )
    }

    func testRunSyncWhenAutoMigrationOnSwapsCanonicalWhenCanonicalSkillFileIsSymlinkAndAlternativeIsRegular() async throws {
        let key = "swap-canonical-skill"
        let claudeSkill = path(".claude/skills/\(key)")
        let agentsSkill = path(".agents/skills/\(key)")
        let legacyTarget = path(".config/ai-agents/skills/\(key).md")

        try FileManager.default.createDirectory(at: legacyTarget.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("same-body".utf8).write(to: legacyTarget)
        try FileManager.default.createDirectory(at: claudeSkill, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            at: claudeSkill.appendingPathComponent("SKILL.md"),
            withDestinationURL: legacyTarget
        )
        try writeSkill(root: path(".agents/skills"), key: key, body: "same-body")
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        let canonical = try XCTUnwrap(state.skills.first(where: { $0.skillKey == key }))
        let canonicalSkillFile = URL(fileURLWithPath: canonical.canonicalSourcePath, isDirectory: true).appendingPathComponent("SKILL.md")
        XCTAssertEqual(URL(fileURLWithPath: canonical.canonicalSourcePath).standardizedFileURL.path, claudeSkill.standardizedFileURL.path)
        XCTAssertFalse(canonicalSkillFile.isTestSymlink)
        XCTAssertTrue(agentsSkill.isTestSymlink)
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: agentsSkill.path), claudeSkill.path)
    }

    func testRunSyncWhenAutoMigrationOffDoesNotRepairCanonicalSkillFileSymlink() async throws {
        let key = "no-repair-skill"
        let claudeSkill = path(".claude/skills/\(key)")
        let legacyTarget = path(".config/ai-agents/skills/\(key).md")

        try FileManager.default.createDirectory(at: legacyTarget.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("same-body".utf8).write(to: legacyTarget)
        try FileManager.default.createDirectory(at: claudeSkill, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            at: claudeSkill.appendingPathComponent("SKILL.md"),
            withDestinationURL: legacyTarget
        )
        try writeAutoMigrationPreference(enabled: false)
        configureEngine()

        let engine = SyncEngine()
        _ = try await engine.runSync(trigger: .manual)

        XCTAssertTrue(claudeSkill.appendingPathComponent("SKILL.md").isTestSymlink)
    }

    func testRunSyncWhenAutoMigrationOnKeepsBrokenCanonicalIfNoHealthyAlternativeExists() async throws {
        let key = "broken-no-alt"
        let claudeSkill = path(".claude/skills/\(key)")
        let missingTarget = path(".config/ai-agents/skills/\(key).md")
        try FileManager.default.createDirectory(at: claudeSkill, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            at: claudeSkill.appendingPathComponent("SKILL.md"),
            withDestinationURL: missingTarget
        )
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        let state = try await engine.runSync(trigger: .manual)

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertTrue(claudeSkill.appendingPathComponent("SKILL.md").isTestSymlink)
    }

    func testDeleteCanonicalSourceRequiresConfirmedTrue() async throws {
        let skillPath = path(".claude/skills/delete-me")
        try FileManager.default.createDirectory(at: skillPath, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: skillPath.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: skillPath.path)

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.deleteCanonicalSource(skill: skill, confirmed: false)
        }
    }

    func testDeleteCanonicalSourceRejectsOutsideAllowedRoots() async throws {
        let outside = path("outside/delete-me")
        try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: outside.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: outside.path)

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
        }
    }

    func testDeleteCanonicalSourceRejectsLegacyPath() async throws {
        let legacyFile = path(".config/ai-agents/skills/legacy-delete.md")
        try FileManager.default.createDirectory(at: legacyFile.deletingLastPathComponent(), withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: legacyFile)

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: legacyFile.path)

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
        }
    }

    func testDeleteCanonicalSourceMovesToTrashAndResyncs() async throws {
        let deletable = path(".claude/skills/delete-ok")
        try FileManager.default.createDirectory(at: deletable, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: deletable.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        _ = try await engine.runSync(trigger: .manual)
        let before = try XCTUnwrap(store.loadState().skills.first(where: { $0.name == "delete-ok" }))

        let state = try await engine.deleteCanonicalSource(skill: before, confirmed: true)

        XCTAssertFalse(FileManager.default.fileExists(atPath: deletable.path))
        XCTAssertEqual(state.summary.globalCount, 0)
        let trashDir = path(".Trash")
        let trashed = try FileManager.default.contentsOfDirectory(atPath: trashDir.path)
        XCTAssertTrue(trashed.contains(where: { $0.hasPrefix("delete-ok") }))
    }

    func testMakeGlobalRequiresConfirmedTrue() async throws {
        let projectSkill = path("Dev/workspace-a/.claude/skills/project-skill")
        try FileManager.default.createDirectory(at: projectSkill, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: projectSkill.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: projectSkill.path, scope: "project", workspace: path("Dev/workspace-a").path, key: "project-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: false)
        }
    }

    func testMakeGlobalRejectsNonProjectSkill() async throws {
        let globalSkillPath = path(".claude/skills/global-skill")
        try FileManager.default.createDirectory(at: globalSkillPath, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: globalSkillPath.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: globalSkillPath.path, scope: "global", workspace: nil, key: "global-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: true)
        }
    }

    func testMakeGlobalRejectsOutsideProjectRoots() async throws {
        let outside = path("outside/project-skill")
        try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: outside.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: outside.path, scope: "project", workspace: path("Dev/workspace-a").path, key: "project-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: true)
        }
    }

    func testMakeGlobalRejectsProtectedPath() async throws {
        let protected = path("Dev/workspace-a/.claude/skills/.system/protected-skill")
        try FileManager.default.createDirectory(at: protected, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: protected.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: protected.path, scope: "project", workspace: path("Dev/workspace-a").path, key: ".system/protected-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: true)
        }
    }

    func testMakeGlobalRejectsWhenSourceMissing() async throws {
        configureEngine()
        let engine = SyncEngine()
        let missing = path("Dev/workspace-a/.claude/skills/missing-skill")
        let skill = makeSkill(path: missing.path, scope: "project", workspace: path("Dev/workspace-a").path, key: "missing-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: true)
        }
    }

    func testMakeGlobalRejectsWhenGlobalTargetExists() async throws {
        let source = path("Dev/workspace-a/.claude/skills/project-skill")
        let target = path(".claude/skills/project-skill")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        try "source".data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        try "target".data(using: .utf8)?.write(to: target.appendingPathComponent("SKILL.md"))

        configureEngine()
        let engine = SyncEngine()
        let skill = makeSkill(path: source.path, scope: "project", workspace: path("Dev/workspace-a").path, key: "project-skill")

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.makeGlobal(skill: skill, confirmed: true)
        }
    }

    func testMakeGlobalMovesProjectSkillToGlobalAndResyncs() async throws {
        let source = path("Dev/workspace-a/.claude/skills/project-skill")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try "project".data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        _ = try await engine.runSync(trigger: .manual)
        let before = try XCTUnwrap(store.loadState().skills.first(where: { $0.skillKey == "project-skill" }))

        let state = try await engine.makeGlobal(skill: before, confirmed: true)

        let destination = path(".claude/skills/project-skill")
        XCTAssertFalse(FileManager.default.fileExists(atPath: source.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: destination.path))
        XCTAssertTrue(state.skills.contains(where: { $0.skillKey == "project-skill" && $0.scope == "global" }))
    }

    func testRenameSkillRenamesCanonicalDirectoryAndResyncsTargets() async throws {
        let oldKey = "old-skill"
        let source = path(".claude/skills/\(oldKey)")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try """
        ---
        title: Old Title
        ---
        """.data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == oldKey }))

        let renamed = try await engine.renameSkill(skill: skill, newTitle: "New Skill Name")

        let newKey = "new-skill-name"
        let newSource = path(".claude/skills/\(newKey)")
        XCTAssertFalse(FileManager.default.fileExists(atPath: source.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: newSource.path))
        XCTAssertTrue(renamed.skills.contains(where: { $0.skillKey == newKey }))
        XCTAssertFalse(FileManager.default.fileExists(atPath: path(".agents/skills/\(oldKey)").path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: path(".codex/skills/\(oldKey)").path))
        XCTAssertTrue(path(".agents/skills/\(newKey)").isTestSymlink)
        XCTAssertTrue(path(".codex/skills/\(newKey)").isTestSymlink)
    }

    func testRenameSkillUpdatesFrontmatterTitle() async throws {
        let source = path(".claude/skills/rename-title")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try """
        ---
        title: Before
        description: Keep me
        ---

        # Heading
        """.data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "rename-title" }))

        _ = try await engine.renameSkill(skill: skill, newTitle: "After Name")

        let file = path(".claude/skills/after-name/SKILL.md")
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertTrue(text.contains("title: After Name"))
        XCTAssertTrue(text.contains("description: Keep me"))
    }

    func testRenameSkillCreatesFrontmatterWhenMissing() async throws {
        let source = path(".claude/skills/no-frontmatter")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try """
        # Body Heading

        Body text.
        """.data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "no-frontmatter" }))

        _ = try await engine.renameSkill(skill: skill, newTitle: "Created Title")

        let file = path(".claude/skills/created-title/SKILL.md")
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertTrue(text.hasPrefix("---\ntitle: Created Title\n---\n"))
        XCTAssertTrue(text.contains("# Body Heading"))
    }

    func testRenameSkillRejectsEmptySlugAfterNormalization() async throws {
        let source = path(".claude/skills/empty-title")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try "body".data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "empty-title" }))

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.renameSkill(skill: skill, newTitle: " !!! ")
        }
    }

    func testRenameSkillRejectsConflictWhenTargetExists() async throws {
        let source = path(".claude/skills/source-skill")
        let target = path(".claude/skills/existing-skill")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        try "source".data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        try "target".data(using: .utf8)?.write(to: target.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "source-skill" }))

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.renameSkill(skill: skill, newTitle: "Existing Skill")
        }
    }

    func testRenameSkillRejectsProtectedOrOutsideAllowedRoots() async throws {
        let outside = path("outside/some-skill")
        try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: outside.appendingPathComponent("SKILL.md"))

        let protected = path(".claude/skills/.system/protected-skill")
        try FileManager.default.createDirectory(at: protected, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: protected.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.renameSkill(skill: self.makeSkill(path: outside.path, key: "some-skill"), newTitle: "Renamed")
        }
        await XCTAssertThrowsErrorAsync {
            _ = try await engine.renameSkill(
                skill: self.makeSkill(path: protected.path, key: ".system/protected-skill"),
                newTitle: "Renamed"
            )
        }
    }

    func testRenameSkillNoOpWhenSlugUnchanged() async throws {
        let source = path(".claude/skills/same-name")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        try "x".data(using: .utf8)?.write(to: source.appendingPathComponent("SKILL.md"))
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "same-name" }))

        await XCTAssertThrowsErrorAsync {
            _ = try await engine.renameSkill(skill: skill, newTitle: " Same Name ")
        }
    }

    func testOpenAndRevealUseShellRunnerCommands() throws {
        let recorder = CommandRecorder()
        configureEngine(shellRunner: recorder)
        let engine = SyncEngine()
        let skill = makeSkill(path: path(".claude/skills/alpha").path)

        try engine.openInZed(skill: skill)
        try engine.revealInFinder(skill: skill)

        XCTAssertEqual(recorder.commands.count, 2)
        XCTAssertEqual(recorder.commands[0], ["open", "-a", "Zed", skill.canonicalSourcePath])
        XCTAssertEqual(recorder.commands[1], ["open", "-R", skill.canonicalSourcePath])
    }

    func testSyncNowIntentRunsRealSyncAndWritesState() async throws {
        try writeSkill(root: path(".claude/skills"), key: "from-intent", body: "intent")
        configureEngine()

        _ = try await SyncNowIntent().perform()
        let state = store.loadState()

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(state.summary.globalCount, 1)
    }

    func testSyncNowIntentHonorsPersistedAutoMigrationSetting() async throws {
        try writeSkill(root: path(".codex/skills"), key: "intent-skill", body: "intent")
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        _ = try await SyncNowIntent().perform()
        let state = store.loadState()

        XCTAssertEqual(state.sync.status, .ok)
        XCTAssertEqual(
            URL(fileURLWithPath: state.skills.first?.canonicalSourcePath ?? "").standardizedFileURL.path,
            path(".claude/skills/intent-skill").standardizedFileURL.path
        )
        XCTAssertTrue(path(".codex/skills/intent-skill").isTestSymlink)
    }

    func testDeleteCanonicalSourceResyncHonorsPersistedAutoMigrationSetting() async throws {
        try writeSkill(root: path(".claude/skills"), key: "delete-resync", body: "same")
        try writeSkill(root: path(".agents/skills"), key: "delete-resync", body: "same")
        try writeAutoMigrationPreference(enabled: true)
        configureEngine()

        let engine = SyncEngine()
        let initial = try await engine.runSync(trigger: .manual)
        let skill = try XCTUnwrap(initial.skills.first(where: { $0.skillKey == "delete-resync" }))

        try FileManager.default.removeItem(at: path(".agents/skills/delete-resync"))
        try writeSkill(root: path(".agents/skills"), key: "delete-resync", body: "same")

        let finalState = try await engine.deleteCanonicalSource(skill: skill, confirmed: true)
        let canonical = try XCTUnwrap(finalState.skills.first(where: { $0.skillKey == "delete-resync" }))

        XCTAssertEqual(
            URL(fileURLWithPath: canonical.canonicalSourcePath).standardizedFileURL.path,
            path(".claude/skills/delete-resync").standardizedFileURL.path
        )
        XCTAssertTrue(path(".agents/skills/delete-resync").isTestSymlink)
    }

    private func configureEngine(shellRunner: SyncShellRunning = DefaultSyncShellRunner()) {
        SyncEngineEnvironment.testOverride = SyncEngineEnvironment(
            homeDirectory: homeDir,
            devRoot: path("Dev"),
            worktreesRoot: path(".codex/worktrees"),
            runtimeDirectory: runtimeDir,
            shellRunner: shellRunner
        )
    }

    private func path(_ relative: String) -> URL {
        homeDir.appendingPathComponent(relative, isDirectory: true)
    }

    private func writeSkill(root: URL, key: String, body: String) throws {
        let dir = root.appendingPathComponent(key, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try body.data(using: .utf8)?.write(to: dir.appendingPathComponent("SKILL.md"))
    }

    private func writeLegacySkill(name: String, body: String) throws {
        let legacyRoot = path(".config/ai-agents/skills")
        try FileManager.default.createDirectory(at: legacyRoot, withIntermediateDirectories: true)
        try body.data(using: .utf8)?.write(to: legacyRoot.appendingPathComponent("\(name).md"))
    }

    private func writeAutoMigrationPreference(enabled: Bool) throws {
        try writeSettings(autoMigrate: enabled, workspaceDiscoveryRoots: nil)
    }

    private func writeSettings(autoMigrate: Bool, workspaceDiscoveryRoots: [String]?) throws {
        let payload: [String: Any] = [
            "version": 1,
            "auto_migrate_to_canonical_source": autoMigrate,
            "workspace_discovery_roots": workspaceDiscoveryRoots ?? []
        ]
        let url = runtimeDir.appendingPathComponent("app-settings.json")
        let data = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
        try data.write(to: url)
    }

    private func makeSkill(path: String, scope: String = "global", workspace: String? = nil, key: String = "sample") -> SkillRecord {
        SkillRecord(
            id: "id-\(UUID().uuidString)",
            name: "sample",
            scope: scope,
            workspace: workspace,
            canonicalSourcePath: path,
            targetPaths: [],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: key,
            symlinkTarget: path
        )
    }
}

private final class CommandRecorder: SyncShellRunning {
    var commands: [[String]] = []

    func run(_ command: [String]) throws {
        commands.append(command)
    }
}

private extension XCTestCase {
    func XCTAssertThrowsErrorAsync(
        _ expression: @escaping () async throws -> Void,
        assertion: ((Error) -> Void)? = nil,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            try await expression()
            XCTFail("Expected error to be thrown", file: file, line: line)
        } catch {
            assertion?(error)
        }
    }
}

private extension URL {
    var isTestSymlink: Bool {
        (try? resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
    }
}
