import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as tauriApi from "./tauriApi";
import type {
  McpServerRecord,
  SkillDetails,
  SkillRecord,
  SyncState,
} from "./types";

vi.mock("./tauriApi", () => ({
  getState: vi.fn(),
  runSync: vi.fn(),
  getSkillDetails: vi.fn(),
  getSubagentDetails: vi.fn(),
  listSubagents: vi.fn(),
  mutateSkill: vi.fn(),
  renameSkill: vi.fn(),
  openSkillPath: vi.fn(),
  openSubagentPath: vi.fn(),
  getStarredSkillIds: vi.fn(),
  setSkillStarred: vi.fn(),
  setMcpServerEnabled: vi.fn(),
}));

let clipboardWriteSpy: ReturnType<typeof vi.fn>;

const projectSkill: SkillRecord = {
  id: "project-1",
  name: "Project Skill",
  scope: "project",
  workspace: "/tmp/workspace",
  canonical_source_path: "/tmp/workspace/.claude/skills/project-skill",
  target_paths: ["/tmp/workspace/.claude/skills/project-skill"],
  status: "active",
  package_type: "dir",
  skill_key: "project-skill",
};

const archivedSkill: SkillRecord = {
  id: "archived-1",
  name: "Archived Skill",
  scope: "global",
  workspace: null,
  canonical_source_path: "/tmp/runtime/archives/abc/source",
  target_paths: ["/tmp/home/.agents/skills/archived-skill"],
  status: "archived",
  package_type: "dir",
  skill_key: "archived-skill",
};

function buildState(
  skills: SkillRecord[],
  mcpServers: McpServerRecord[] = [],
): SyncState {
  return {
    version: 2,
    generated_at: "2026-02-20T17:00:00Z",
    sync: { status: "ok", error: null },
    summary: {
      global_count: skills.filter(
        (skill) => skill.scope === "global" && skill.status === "active",
      ).length,
      project_count: skills.filter(
        (skill) => skill.scope === "project" && skill.status === "active",
      ).length,
      conflict_count: 0,
      mcp_count: mcpServers.length,
      mcp_warning_count: mcpServers.reduce(
        (total, item) => total + item.warnings.length,
        0,
      ),
    },
    subagent_summary: {
      global_count: 0,
      project_count: 0,
      conflict_count: 0,
      mcp_count: 0,
      mcp_warning_count: 0,
    },
    skills,
    subagents: [],
    mcp_servers: mcpServers,
  };
}

function buildDetails(
  skill: SkillRecord,
  overrides?: Partial<SkillDetails>,
): SkillDetails {
  return {
    skill,
    main_file_path: `${skill.canonical_source_path}/SKILL.md`,
    main_file_exists: true,
    main_file_body_preview: "# Preview",
    main_file_body_preview_truncated: false,
    skill_dir_tree_preview: `${skill.skill_key}/\n\`-- SKILL.md`,
    skill_dir_tree_preview_truncated: false,
    last_modified_unix_seconds: 1_700_000_000,
    ...overrides,
  };
}

function setApiDefaults(
  state: SyncState,
  detailsBySkillKey: Record<string, SkillDetails>,
) {
  vi.mocked(tauriApi.getState).mockResolvedValue(state);
  vi.mocked(tauriApi.getStarredSkillIds).mockResolvedValue([]);
  vi.mocked(tauriApi.listSubagents).mockResolvedValue([
    {
      id: "sub-1",
      name: "Subagent",
      description: "Desc",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/subagent.md",
      target_paths: ["/tmp/home/.claude/agents/subagent.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "subagent",
      symlink_target: "/tmp/home/.claude/agents/subagent.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
    },
  ]);
  vi.mocked(tauriApi.getSubagentDetails).mockResolvedValue({
    subagent: {
      id: "sub-1",
      name: "Subagent",
      description: "Desc",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/subagent.md",
      target_paths: ["/tmp/home/.claude/agents/subagent.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "subagent",
      symlink_target: "/tmp/home/.claude/agents/subagent.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
    },
    main_file_path: "/tmp/home/.claude/agents/subagent.md",
    main_file_exists: true,
    main_file_body_preview: "# Subagent",
    main_file_body_preview_truncated: false,
    subagent_dir_tree_preview: "agents/\n`-- subagent.md",
    subagent_dir_tree_preview_truncated: false,
    last_modified_unix_seconds: 1_700_000_000,
    target_statuses: [],
  });
  vi.mocked(tauriApi.runSync).mockResolvedValue(state);
  vi.mocked(tauriApi.mutateSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.renameSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.openSkillPath).mockResolvedValue(undefined);
  vi.mocked(tauriApi.openSubagentPath).mockResolvedValue(undefined);
  vi.mocked(tauriApi.setSkillStarred).mockResolvedValue([]);
  vi.mocked(tauriApi.setMcpServerEnabled).mockResolvedValue(state);
  vi.mocked(tauriApi.getSkillDetails).mockImplementation((skillKey) => {
    const details = detailsBySkillKey[skillKey];
    if (!details) {
      return Promise.reject(new Error(`missing details for ${skillKey}`));
    }
    return Promise.resolve(details);
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  clipboardWriteSpy = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", {
    clipboard: { writeText: clipboardWriteSpy },
  });
});

describe("App quiet redesign", () => {
  it("renders unified source list sections and no tabs", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText("Skills")).toBeInTheDocument();
    expect(screen.getByText("Subagents")).toBeInTheDocument();
    expect(screen.getByText("MCP")).toBeInTheDocument();

    expect(
      screen.queryByRole("button", { name: /^Skills$/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^Subagents$/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^MCP$/ }),
    ).not.toBeInTheDocument();
  });

  it("shows overview and details together by default", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText("SKILL dir tree")).toBeInTheDocument();
    expect(screen.getByText("Targets")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Overview" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Details" }),
    ).not.toBeInTheDocument();
  });

  it("shows small scope labels near status dots", async () => {
    const state = buildState([projectSkill, archivedSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getAllByText("Project").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Global").length).toBeGreaterThan(0);
  });

  it("opens skill folder/file through Open menu", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Open…" }));
    await user.click(screen.getByRole("menuitem", { name: "Open folder" }));

    await user.click(screen.getByRole("button", { name: "Open…" }));
    await user.click(screen.getByRole("menuitem", { name: "Open file" }));

    expect(tauriApi.openSkillPath).toHaveBeenNthCalledWith(
      1,
      projectSkill.skill_key,
      "folder",
    );
    expect(tauriApi.openSkillPath).toHaveBeenNthCalledWith(
      2,
      projectSkill.skill_key,
      "file",
    );
  });

  it("renders overflow actions and confirms delete with DELETE text", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "More actions" }));

    expect(
      screen.getByRole("menuitem", { name: "Archive" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("menuitem", { name: "Delete" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("menuitem", { name: "Delete" }));

    const dialog = screen.getByRole("dialog", { name: "Confirm delete" });
    const confirmButton = within(dialog).getByRole("button", {
      name: "Delete",
    });
    expect(confirmButton).toBeDisabled();

    await user.type(
      within(dialog).getByLabelText("Type DELETE to confirm"),
      "DELETE",
    );
    expect(confirmButton).toBeEnabled();

    await user.click(confirmButton);

    expect(tauriApi.mutateSkill).toHaveBeenCalledWith(
      "delete_skill",
      projectSkill.skill_key,
    );
  });

  it("prevents repeated skill mutations while one is in flight", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    let resolveMutation: ((value: SyncState) => void) | undefined;
    const pendingMutation = new Promise<SyncState>((resolve) => {
      resolveMutation = resolve;
    });
    vi.mocked(tauriApi.mutateSkill).mockReturnValue(pendingMutation);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Archive" }));
    expect(tauriApi.mutateSkill).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "More actions" }));
    const archiveAgain = screen.queryByRole("menuitem", { name: "Archive" });
    if (archiveAgain) {
      await user.click(archiveAgain);
    }
    expect(tauriApi.mutateSkill).toHaveBeenCalledTimes(1);

    resolveMutation?.(state);
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "More actions" }),
      ).toBeEnabled();
    });
  });

  it("shows compact path and handles copy fallback", async () => {
    const details = buildDetails(projectSkill, {
      main_file_path: "/tmp/workspace/.claude/skills/project-skill/SKILL.md",
    });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText("/tmp/.../SKILL.md")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Copy main path" }));

    expect(
      screen.queryByText("Copy main path failed."),
    ).not.toBeInTheDocument();
  });

  it("closes open-target menu when selecting another catalog item", async () => {
    const state = buildState([projectSkill, archivedSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Open…" }));
    expect(
      screen.getByRole("menuitem", { name: "Open folder" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Archived Skill/i }));

    await waitFor(() => {
      expect(
        screen.queryByRole("menuitem", { name: "Open folder" }),
      ).not.toBeInTheDocument();
    });
  });

  it("sends scope and workspace when toggling project mcp server", async () => {
    const workspace = "/tmp/workspace-a";
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace,
          transport: "http",
          command: null,
          args: [],
          url: "https://mcp.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: true,
            project: true,
          },
          targets: [`${workspace}/.mcp.json`],
          warnings: [],
        },
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: /exa/i }));
    await user.click(screen.getByRole("switch", { name: "project toggle" }));

    await waitFor(() => {
      expect(tauriApi.setMcpServerEnabled).toHaveBeenCalledWith(
        "exa",
        "project",
        false,
        "project",
        workspace,
      );
    });
  });

  it("closes menu and dialog on Escape", async () => {
    const state = buildState([projectSkill, archivedSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Archived Skill/i }));
    await user.click(screen.getByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(
      screen.getByRole("dialog", { name: "Confirm delete" }),
    ).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("dialog", { name: "Confirm delete" }),
    ).not.toBeInTheDocument();
  });
});
