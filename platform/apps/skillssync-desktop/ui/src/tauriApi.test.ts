import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getRuntimeControls,
  loadDashboardSnapshot,
  getStarredSkillIds,
  getState,
  getSkillDetails,
  getSubagentDetails,
  listSubagents,
  getPlatformContext,
  getMcpServers,
  mutateSkill,
  migrateDotagents,
  dotagentsMcpAdd,
  dotagentsMcpRemove,
  dotagentsSkillsAdd,
  dotagentsSkillsInstall,
  dotagentsSkillsRemove,
  dotagentsSkillsUpdate,
  listDotagentsMcp,
  listDotagentsSkills,
  openSkillPath,
  openSubagentPath,
  renameSkill,
  runDotagentsSync,
  runSync,
  setAllowFilesystemChanges,
  setSkillStarred,
  setMcpServerEnabled,
  listAuditEvents,
} from "./tauriApi";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauriApi command payloads", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("sends camelCase payload for get_skill_details", async () => {
    await getSkillDetails("alpha");
    expect(invoke).toHaveBeenCalledWith("get_skill_details", {
      skillKey: "alpha",
    });
  });

  it("sends camelCase payload for get_subagent_details", async () => {
    await getSubagentDetails("reviewer");
    expect(invoke).toHaveBeenCalledWith("get_subagent_details", {
      subagentId: "reviewer",
    });
  });

  it("sends camelCase payload for rename_skill", async () => {
    await renameSkill("alpha", "New Title");
    expect(invoke).toHaveBeenCalledWith("rename_skill", {
      skillKey: "alpha",
      newTitle: "New Title",
    });
  });

  it("sends camelCase payload for mutation commands", async () => {
    await mutateSkill("archive_skill", "alpha");
    expect(invoke).toHaveBeenCalledWith("archive_skill", {
      skillKey: "alpha",
      confirmed: true,
    });
  });

  it("sends target payload for open_skill_path", async () => {
    await openSkillPath("alpha", "file");
    expect(invoke).toHaveBeenCalledWith("open_skill_path", {
      skillKey: "alpha",
      target: "file",
    });
  });

  it("sends target payload for open_subagent_path", async () => {
    await openSubagentPath("reviewer", "folder");
    expect(invoke).toHaveBeenCalledWith("open_subagent_path", {
      subagentId: "reviewer",
      target: "folder",
    });
  });

  it("runs sync with manual trigger", async () => {
    await runSync();
    expect(invoke).toHaveBeenCalledWith("run_sync", { trigger: "manual" });
  });

  it("runs dotagents sync with explicit scope", async () => {
    await runDotagentsSync("project");
    expect(invoke).toHaveBeenCalledWith("run_dotagents_sync", {
      scope: "project",
    });
  });

  it("loads runtime controls without args", async () => {
    await getRuntimeControls();
    expect(invoke).toHaveBeenCalledWith("get_runtime_controls");
  });

  it("sets allow filesystem changes payload", async () => {
    await setAllowFilesystemChanges(true);
    expect(invoke).toHaveBeenCalledWith("set_allow_filesystem_changes", {
      allow: true,
    });
  });

  it("lists audit events with filters", async () => {
    await listAuditEvents({ limit: 25, status: "blocked", action: "run_sync" });
    expect(invoke).toHaveBeenCalledWith("list_audit_events", {
      limit: 25,
      status: "blocked",
      action: "run_sync",
    });
  });

  it("loads state without args", async () => {
    await getState();
    expect(invoke).toHaveBeenCalledWith("get_state");
  });

  it("lists subagents with scope", async () => {
    await listSubagents("all");
    expect(invoke).toHaveBeenCalledWith("list_subagents", { scope: "all" });
  });

  it("loads platform context without args", async () => {
    await getPlatformContext();
    expect(invoke).toHaveBeenCalledWith("get_platform_context");
  });

  it("loads mcp servers without args", async () => {
    await getMcpServers();
    expect(invoke).toHaveBeenCalledWith("get_mcp_servers");
  });

  it("lists dotagents skills", async () => {
    await listDotagentsSkills("all");
    expect(invoke).toHaveBeenCalledWith("list_dotagents_skills", {
      scope: "all",
    });
  });

  it("lists dotagents mcp", async () => {
    await listDotagentsMcp("user");
    expect(invoke).toHaveBeenCalledWith("list_dotagents_mcp", {
      scope: "user",
    });
  });

  it("runs dotagents skills install", async () => {
    await dotagentsSkillsInstall("project");
    expect(invoke).toHaveBeenCalledWith("dotagents_skills_install", {
      scope: "project",
    });
  });

  it("runs dotagents skills add", async () => {
    await dotagentsSkillsAdd("owner/repo", "all");
    expect(invoke).toHaveBeenCalledWith("dotagents_skills_add", {
      package: "owner/repo",
      scope: "all",
    });
  });

  it("runs dotagents skills remove", async () => {
    await dotagentsSkillsRemove("owner/repo", "all");
    expect(invoke).toHaveBeenCalledWith("dotagents_skills_remove", {
      package: "owner/repo",
      scope: "all",
    });
  });

  it("runs dotagents skills update with package", async () => {
    await dotagentsSkillsUpdate("owner/repo", "all");
    expect(invoke).toHaveBeenCalledWith("dotagents_skills_update", {
      package: "owner/repo",
      scope: "all",
    });
  });

  it("runs dotagents mcp add/remove and migration", async () => {
    await dotagentsMcpAdd(["exa"], "project");
    await dotagentsMcpRemove(["exa"], "project");
    await migrateDotagents("all");
    expect(invoke).toHaveBeenCalledWith("dotagents_mcp_add", {
      args: ["exa"],
      scope: "project",
    });
    expect(invoke).toHaveBeenCalledWith("dotagents_mcp_remove", {
      args: ["exa"],
      scope: "project",
    });
    expect(invoke).toHaveBeenCalledWith("migrate_dotagents", { scope: "all" });
  });

  it("loads starred skill ids without args", async () => {
    await getStarredSkillIds();
    expect(invoke).toHaveBeenCalledWith("get_starred_skill_ids");
  });

  it("sends snake_case payload for set_skill_starred", async () => {
    await setSkillStarred("skill-1", true);
    expect(invoke).toHaveBeenCalledWith("set_skill_starred", {
      skillId: "skill-1",
      starred: true,
    });
  });

  it("sends payload for set_mcp_server_enabled", async () => {
    await setMcpServerEnabled("exa", "claude", false);
    expect(invoke).toHaveBeenCalledWith("set_mcp_server_enabled", {
      serverKey: "exa",
      agent: "claude",
      enabled: false,
    });
  });

  it("sends scope and workspace for set_mcp_server_enabled when provided", async () => {
    await setMcpServerEnabled(
      "exa",
      "codex",
      true,
      "project",
      "/tmp/workspace-a",
    );
    expect(invoke).toHaveBeenCalledWith("set_mcp_server_enabled", {
      serverKey: "exa",
      agent: "codex",
      enabled: true,
      scope: "project",
      workspace: "/tmp/workspace-a",
    });
  });

  it("loads dashboard snapshot via existing commands", async () => {
    await loadDashboardSnapshot();

    expect(invoke).toHaveBeenCalledWith("get_state");
    expect(invoke).toHaveBeenCalledWith("get_starred_skill_ids");
    expect(invoke).toHaveBeenCalledWith("list_subagents", { scope: "all" });
  });
});
