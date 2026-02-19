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

    private func makeSkill(path: String) -> SkillRecord {
        SkillRecord(
            id: "id-\(UUID().uuidString)",
            name: "sample",
            scope: "global",
            workspace: nil,
            canonicalSourcePath: path,
            targetPaths: [],
            exists: true,
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: "sample",
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
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            try await expression()
            XCTFail("Expected error to be thrown", file: file, line: line)
        } catch { }
    }
}
