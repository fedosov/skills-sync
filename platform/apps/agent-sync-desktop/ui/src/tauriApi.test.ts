import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getAppContext,
  getRuntimeStatus,
  getSkillsWorkspaceContext,
  listMcpServers,
  listSkills,
  listSkillsCli,
  openAgentsDir,
  openAgentsToml,
  openUserHome,
  runDotagentsCommand,
  runSkillsCliCommand,
  setProjectRoot,
  setScope,
  setSkillsActiveAgents,
  setSkillsScope,
  setSkillsVersionOverride,
} from "./tauriApi";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauriApi payloads", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("loads runtime status and app context without args", async () => {
    await getRuntimeStatus();
    await getAppContext();

    expect(invoke).toHaveBeenCalledWith("get_runtime_status");
    expect(invoke).toHaveBeenCalledWith("get_app_context");
  });

  it("sends scope and project root payloads", async () => {
    await setScope("project");
    await setProjectRoot("/tmp/demo");

    expect(invoke).toHaveBeenCalledWith("set_scope", {
      scope: "project",
    });
    expect(invoke).toHaveBeenCalledWith("set_project_root", {
      projectRoot: "/tmp/demo",
    });
  });

  it("lists vendor reads without payloads", async () => {
    await listSkills();
    await listMcpServers();

    expect(invoke).toHaveBeenCalledWith("list_skills");
    expect(invoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("sends structured command requests", async () => {
    await runDotagentsCommand({
      kind: "skillAdd",
      source: "owner/repo",
      name: "lint",
      all: false,
    });

    expect(invoke).toHaveBeenCalledWith("run_dotagents_command", {
      request: {
        kind: "skillAdd",
        source: "owner/repo",
        name: "lint",
        all: false,
      },
    });
  });

  it("invokes skills workspace context and setters", async () => {
    await getSkillsWorkspaceContext();
    await setSkillsScope("project");
    await setSkillsActiveAgents(["Claude Code", "Cursor"]);
    await setSkillsVersionOverride("0.4.0");
    await setSkillsVersionOverride(null);
    await listSkillsCli();

    expect(invoke).toHaveBeenCalledWith("get_skills_workspace_context");
    expect(invoke).toHaveBeenCalledWith("set_skills_scope", {
      scope: "project",
    });
    expect(invoke).toHaveBeenCalledWith("set_skills_active_agents", {
      agents: ["Claude Code", "Cursor"],
    });
    expect(invoke).toHaveBeenCalledWith("set_skills_version_override", {
      versionOverride: "0.4.0",
    });
    expect(invoke).toHaveBeenCalledWith("set_skills_version_override", {
      versionOverride: null,
    });
    expect(invoke).toHaveBeenCalledWith("list_skills_cli");
  });

  it("sends skills CLI command requests", async () => {
    await runSkillsCliCommand({
      kind: "add",
      source: "vercel-labs/agent-skills",
      agents: ["Claude Code"],
      scope: "global",
    });

    expect(invoke).toHaveBeenCalledWith("run_skills_cli_command", {
      request: {
        kind: "add",
        source: "vercel-labs/agent-skills",
        agents: ["Claude Code"],
        scope: "global",
      },
    });
  });

  it("opens agent paths without extra args", async () => {
    await openAgentsToml();
    await openAgentsDir();
    await openUserHome();

    expect(invoke).toHaveBeenCalledWith("open_agents_toml");
    expect(invoke).toHaveBeenCalledWith("open_agents_dir");
    expect(invoke).toHaveBeenCalledWith("open_user_home");
  });
});
