import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getStarredSkillIds,
  getState,
  getSkillDetails,
  getSubagentDetails,
  listSubagents,
  getPlatformContext,
  getMcpServers,
  mutateSkill,
  openSkillPath,
  openSubagentPath,
  renameSkill,
  runSync,
  setSkillStarred,
  setMcpServerEnabled,
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
});
