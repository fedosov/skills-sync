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
          "top_skills": ["skill-1"],
          "last_command_result": null
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

    func testAppendCommandWritesJsonLine() throws {
        let command = store.makeCommand(type: .syncNow, requestedBy: "unit-test")
        try store.appendCommand(command)

        let lines = try String(contentsOf: SyncPaths.commandQueueURL)
            .split(separator: "\n")
        XCTAssertEqual(lines.count, 1)

        let data = Data(lines[0].utf8)
        let decoded = try JSONDecoder().decode(SyncCommand.self, from: data)
        XCTAssertEqual(decoded.type, .syncNow)
        XCTAssertEqual(decoded.requestedBy, "unit-test")
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
            topSkills: ["p2", "missing", "g3"],
            lastCommandResult: nil
        )

        let top = store.topSkills(from: state)

        XCTAssertEqual(top.count, 6)
        XCTAssertEqual(top.first?.id, "p2")
        XCTAssertEqual(top[1].id, "g3")
        XCTAssertTrue(top.contains(where: { $0.id == "g1" }))
        XCTAssertTrue(top.contains(where: { $0.id == "g2" }))
    }

    func testSyncPathsFallbackUsesApplicationSupportDirectory() {
        let fallback = SyncPaths.fallbackContainerURL.path
        XCTAssertTrue(fallback.contains("/Library/Application Support/SkillsSync"))
        XCTAssertFalse(fallback.contains("/.config/ai-agents/skillssync"))
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
            isSymlinkCanonical: false,
            packageType: "dir",
            skillKey: name.lowercased(),
            symlinkTarget: "/tmp/\(id)"
        )
    }
}
