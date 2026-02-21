import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
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

const globalSkill: SkillRecord = {
  id: "global-1",
  name: "Global Skill",
  scope: "global",
  workspace: null,
  canonical_source_path: "/tmp/home/.claude/skills/global-skill",
  target_paths: ["/tmp/home/.claude/skills/global-skill"],
  status: "active",
  package_type: "dir",
  skill_key: "global-skill",
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
  starredSkillIds: string[] = [],
) {
  vi.mocked(tauriApi.getState).mockResolvedValue(state);
  vi.mocked(tauriApi.getStarredSkillIds).mockResolvedValue(starredSkillIds);
  vi.mocked(tauriApi.listSubagents).mockResolvedValue([]);
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
    target_statuses: [
      {
        path: "/tmp/home/.claude/agents/subagent.md",
        exists: true,
        is_symlink: true,
        symlink_target: "/tmp/home/.agents/subagents/subagent.md",
        points_to_canonical: true,
        kind: "symlink",
      },
    ],
  });
  vi.mocked(tauriApi.setSkillStarred).mockImplementation((_skillId, starred) =>
    Promise.resolve(starred ? ["project-1"] : []),
  );
  vi.mocked(tauriApi.runSync).mockResolvedValue(state);
  vi.mocked(tauriApi.mutateSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.renameSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.openSkillPath).mockResolvedValue(undefined);
  vi.mocked(tauriApi.openSubagentPath).mockResolvedValue(undefined);
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
  vi.spyOn(window, "confirm").mockReturnValue(true);
});

describe("App critical actions", () => {
  it("uses independent desktop scroll containers for left and right columns", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const main = screen.getByRole("main");
    expect(main).toHaveClass("lg:min-h-0");
    expect(main).toHaveClass("lg:flex-1");

    const skillsCard = screen
      .getByRole("heading", { name: "Skills" })
      .closest(".rounded-lg");
    expect(skillsCard).not.toBeNull();
    expect(skillsCard).toHaveClass("lg:flex");
    expect(skillsCard).toHaveClass("lg:flex-col");
    expect(skillsCard).toHaveClass("lg:min-h-0");
    expect(skillsCard).toHaveClass("lg:h-full");
    if (!(skillsCard instanceof HTMLElement)) {
      throw new Error("Skills card must be an HTMLElement.");
    }

    const skillsList = within(skillsCard).getByRole("list");
    const skillsScroller = skillsList.parentElement;
    expect(skillsScroller).not.toBeNull();
    expect(skillsScroller).toHaveClass("lg:flex-1");
    expect(skillsScroller).toHaveClass("lg:min-h-0");
    expect(skillsScroller).toHaveClass("lg:overflow-y-auto");
    expect(skillsScroller).not.toHaveClass("h-[calc(100%-52px)]");

    const detailsCard = screen
      .getByRole("heading", { name: projectSkill.name })
      .closest(".rounded-lg");
    expect(detailsCard).not.toBeNull();
    expect(detailsCard).toHaveClass("lg:flex");
    expect(detailsCard).toHaveClass("lg:flex-col");
    expect(detailsCard).toHaveClass("lg:min-h-0");
    expect(detailsCard).toHaveClass("lg:h-full");
    if (!(detailsCard instanceof HTMLElement)) {
      throw new Error("Details card must be an HTMLElement.");
    }

    const workspaceLabel = within(detailsCard).getByText("Workspace");
    const detailsScroller = workspaceLabel.closest("dl")?.parentElement;
    expect(detailsScroller).not.toBeNull();
    expect(detailsScroller).toHaveClass("lg:flex-1");
    expect(detailsScroller).toHaveClass("lg:min-h-0");
    expect(detailsScroller).toHaveClass("lg:overflow-y-auto");
  });

  it("loads initial state and selected skill details", async () => {
    const state = buildState([projectSkill, archivedSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    render(<App />);

    await screen.findByRole("heading", { name: projectSkill.name });
    expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    expect(tauriApi.getSkillDetails).toHaveBeenCalledWith(
      projectSkill.skill_key,
    );
    expect(tauriApi.getStarredSkillIds).toHaveBeenCalledTimes(1);
  });

  it("shows star action in details and toggles it", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(
      state,
      {
        [projectSkill.skill_key]: buildDetails(projectSkill),
      },
      [],
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Star" }));
    expect(tauriApi.setSkillStarred).toHaveBeenCalledWith("project-1", true);
  });

  it("reorders list when starring while keeping active above archived", async () => {
    const state = buildState([projectSkill, globalSkill, archivedSkill]);
    setApiDefaults(
      state,
      {
        [projectSkill.skill_key]: buildDetails(projectSkill),
        [globalSkill.skill_key]: buildDetails(globalSkill),
        [archivedSkill.skill_key]: buildDetails(archivedSkill),
      },
      [],
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const skillsCard = screen
      .getByRole("heading", { name: "Skills" })
      .closest(".rounded-lg");
    expect(skillsCard).not.toBeNull();
    if (!(skillsCard instanceof HTMLElement)) {
      throw new Error("Skills card must be an HTMLElement.");
    }
    const list = within(skillsCard).getByRole("list");
    const namesBefore = within(list)
      .getAllByRole("button")
      .map((button) => {
        const title = button.querySelector("p");
        return title?.textContent ?? "";
      });
    expect(namesBefore[0]).toBe("Global Skill");

    await user.click(within(skillsCard).getAllByRole("button")[1]);
    await user.click(screen.getByRole("button", { name: "Star" }));

    const namesAfter = within(list)
      .getAllByRole("button")
      .map((button) => {
        const title = button.querySelector("p");
        return title?.textContent ?? "";
      });
    expect(namesAfter[0]).toBe("Project Skill");
    expect(namesAfter[2]).toBe("Archived Skill");
  });

  it("refreshes from toolbar via run_sync", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(
      screen.queryByRole("button", { name: "Sync" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    expect(tauriApi.runSync).toHaveBeenCalledTimes(2);
  });

  it("falls back to cached state when run_sync fails", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.runSync).mockRejectedValueOnce(new Error("sync failed"));

    render(<App />);

    await screen.findByRole("heading", { name: projectSkill.name });
    expect(tauriApi.getState).toHaveBeenCalledTimes(1);
  });

  it("loads subagents only after run_sync resolves", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    let resolveSync!: (value: SyncState) => void;
    vi.mocked(tauriApi.runSync).mockImplementation(
      () =>
        new Promise<SyncState>((resolve) => {
          resolveSync = resolve;
        }),
    );

    render(<App />);

    await waitFor(() => {
      expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    });
    expect(tauriApi.listSubagents).not.toHaveBeenCalled();

    resolveSync(state);

    await waitFor(() => {
      expect(tauriApi.listSubagents).toHaveBeenCalledTimes(1);
    });
  });

  it("archives skill via in-app confirmation flow", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    const confirmSpy = vi.spyOn(window, "confirm");

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Archive" }));
    expect(tauriApi.mutateSkill).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        `Review action: archive_skill on ${projectSkill.skill_key}.`,
      ),
    ).toBeInTheDocument();
    expect(confirmSpy).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Apply change" }));

    expect(tauriApi.mutateSkill).toHaveBeenCalledWith(
      "archive_skill",
      projectSkill.skill_key,
    );
  });

  it("does not mutate when confirmation is rejected", async () => {
    const state = buildState([globalSkill]);
    setApiDefaults(state, {
      [globalSkill.skill_key]: buildDetails(globalSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: globalSkill.name });

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(
      screen.getByText(
        `Review action: delete_skill on ${globalSkill.skill_key}.`,
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(tauriApi.mutateSkill).not.toHaveBeenCalled();
  });

  it("restores archived skill", async () => {
    const state = buildState([archivedSkill]);
    setApiDefaults(state, {
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: archivedSkill.name });

    await user.click(screen.getByRole("button", { name: "Restore" }));
    await user.click(screen.getByRole("button", { name: "Apply change" }));

    expect(tauriApi.mutateSkill).toHaveBeenCalledWith(
      "restore_skill",
      archivedSkill.skill_key,
    );
  });

  it("calls make_global and delete for active project skill", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Make global" }));
    await user.click(screen.getByRole("button", { name: "Apply change" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Apply change" }));

    expect(tauriApi.mutateSkill).toHaveBeenNthCalledWith(
      1,
      "make_global",
      projectSkill.skill_key,
    );
    expect(tauriApi.mutateSkill).toHaveBeenNthCalledWith(
      2,
      "delete_skill",
      projectSkill.skill_key,
    );
  });

  it("opens folder and file targets", async () => {
    const details = buildDetails(projectSkill, { main_file_exists: true });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Open folder" }));
    await user.click(screen.getByRole("button", { name: "Open file" }));

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

  it("disables opening file when there is no main file", async () => {
    const details = buildDetails(projectSkill, { main_file_exists: false });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const openFileButton = screen.getByRole("button", { name: "Open file" });
    expect(openFileButton).toBeDisabled();
  });

  it("opens full skill file from truncated preview link", async () => {
    const details = buildDetails(projectSkill, {
      main_file_body_preview: "# Preview",
      main_file_body_preview_truncated: true,
    });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText(/Preview truncated\./)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "watch full" }));

    expect(tauriApi.openSkillPath).toHaveBeenCalledWith(
      projectSkill.skill_key,
      "file",
    );
  });

  it("renders compact skill dir tree and truncation note", async () => {
    const details = buildDetails(projectSkill, {
      skill_dir_tree_preview: "project-skill/\n|-- references/\n`-- SKILL.md",
      skill_dir_tree_preview_truncated: true,
    });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText("SKILL dir tree")).toBeInTheDocument();
    expect(
      screen.getByText(/project-skill\/[\s\S]*references\/[\s\S]*SKILL\.md/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Tree preview truncated for performance."),
    ).toBeInTheDocument();
  });

  it("renames skill and trims title", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const input = screen.getByPlaceholderText("New skill title");
    await user.clear(input);
    await user.type(input, "  New Skill Name  ");
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect(tauriApi.renameSkill).toHaveBeenCalledWith(
      projectSkill.skill_key,
      "New Skill Name",
    );
  });

  it("shows error for invalid rename key normalization", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const input = screen.getByPlaceholderText("New skill title");
    await user.clear(input);
    await user.type(input, "___");

    const form = input.closest("form");
    expect(form).not.toBeNull();
    fireEvent.submit(form!);

    await waitFor(() => {
      expect(
        screen.getByText("Rename failed: title must produce non-empty key."),
      ).toBeInTheDocument();
    });
    expect(tauriApi.renameSkill).not.toHaveBeenCalled();
  });

  it("renders filtered/total counters in all catalog tabs with stable a11y names", async () => {
    const mcpServers: McpServerRecord[] = [
      {
        server_key: "exa",
        scope: "global",
        workspace: null,
        transport: "http",
        command: null,
        args: [],
        url: "https://mcp.exa.ai/mcp",
        env: {},
        enabled_by_agent: {
          codex: true,
          claude: true,
          project: false,
        },
        targets: ["/tmp/home/.codex/config.toml"],
        warnings: [],
      },
      {
        server_key: "docs",
        scope: "project",
        workspace: "/tmp/workspace-a",
        transport: "stdio",
        command: "npx",
        args: ["-y", "@docs/mcp"],
        url: null,
        env: {},
        enabled_by_agent: {
          codex: true,
          claude: false,
          project: true,
        },
        targets: ["/tmp/workspace-a/.mcp.json"],
        warnings: [],
      },
    ];
    const state = buildState([projectSkill, globalSkill], mcpServers);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [globalSkill.skill_key]: buildDetails(globalSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([
      {
        id: "sub-1",
        name: "Helper",
        description: "General helper",
        scope: "global",
        workspace: null,
        canonical_source_path: "/tmp/home/.agents/subagents/helper.md",
        target_paths: ["/tmp/home/.claude/agents/helper.md"],
        exists: true,
        is_symlink_canonical: false,
        package_type: "file",
        subagent_key: "helper",
        symlink_target: "/tmp/home/.agents/subagents/helper.md",
        model: null,
        tools: [],
        codex_tools_ignored: false,
      },
    ]);

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const skillsTab = screen.getByRole("button", { name: "Skills" });
    const subagentsTab = screen.getByRole("button", { name: "Subagents" });
    const mcpTab = screen.getByRole("button", { name: "MCP" });

    expect(within(skillsTab).getByText("2/2")).toBeInTheDocument();
    expect(within(subagentsTab).getByText("1/1")).toBeInTheDocument();
    expect(within(mcpTab).getByText("2/2")).toBeInTheDocument();
  });

  it("updates tab filtered counters from search while keeping total unchanged", async () => {
    const mcpServers: McpServerRecord[] = [
      {
        server_key: "exa",
        scope: "global",
        workspace: null,
        transport: "http",
        command: null,
        args: [],
        url: "https://mcp.exa.ai/mcp",
        env: {},
        enabled_by_agent: {
          codex: true,
          claude: true,
          project: false,
        },
        targets: ["/tmp/home/.codex/config.toml"],
        warnings: [],
      },
      {
        server_key: "docs",
        scope: "project",
        workspace: "/tmp/workspace-a",
        transport: "stdio",
        command: "npx",
        args: ["-y", "@docs/mcp"],
        url: null,
        env: {},
        enabled_by_agent: {
          codex: true,
          claude: false,
          project: true,
        },
        targets: ["/tmp/workspace-a/.mcp.json"],
        warnings: [],
      },
    ];
    const state = buildState([projectSkill, globalSkill], mcpServers);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [globalSkill.skill_key]: buildDetails(globalSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([
      {
        id: "sub-1",
        name: "Helper",
        description: "General helper",
        scope: "global",
        workspace: null,
        canonical_source_path: "/tmp/home/.agents/subagents/helper.md",
        target_paths: ["/tmp/home/.claude/agents/helper.md"],
        exists: true,
        is_symlink_canonical: false,
        package_type: "file",
        subagent_key: "helper",
        symlink_target: "/tmp/home/.agents/subagents/helper.md",
        model: null,
        tools: [],
        codex_tools_ignored: false,
      },
    ]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.type(
      screen.getByPlaceholderText("Search by name, key, scope or workspace"),
      "project",
    );

    const skillsTab = screen.getByRole("button", { name: "Skills" });
    const subagentsTab = screen.getByRole("button", { name: "Subagents" });
    const mcpTab = screen.getByRole("button", { name: "MCP" });

    expect(within(skillsTab).getByText("1/2")).toBeInTheDocument();
    expect(within(subagentsTab).getByText("0/1")).toBeInTheDocument();
    expect(within(mcpTab).getByText("1/2")).toBeInTheDocument();
  });

  it("renders subagent source and link transparency sections", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([
      {
        id: "sub-1",
        name: "Subagent",
        description: "Desc",
        scope: "global",
        workspace: null,
        canonical_source_path: "/tmp/home/.agents/subagents/subagent.md",
        target_paths: [
          "/tmp/home/.claude/agents/subagent.md",
          "/tmp/home/.cursor/agents/subagent.md",
        ],
        exists: true,
        is_symlink_canonical: false,
        package_type: "file",
        subagent_key: "subagent",
        symlink_target: "/tmp/home/.agents/subagents/subagent.md",
        model: null,
        tools: [],
        codex_tools_ignored: false,
      },
    ]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Subagents" }));

    expect(screen.getByText("Canonical path")).toBeInTheDocument();
    expect(screen.getByText("Targets")).toBeInTheDocument();
    expect(screen.getByText("Target link status")).toBeInTheDocument();
    expect(
      screen.getAllByText("/tmp/home/.claude/agents/subagent.md").length,
    ).toBeGreaterThan(0);
  });

  it("loads selected subagent details by unique subagent id", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([
      {
        id: "sub-global",
        name: "Reviewer (Global)",
        description: "Global reviewer",
        scope: "global",
        workspace: null,
        canonical_source_path: "/tmp/home/.agents/subagents/reviewer.md",
        target_paths: ["/tmp/home/.claude/agents/reviewer.md"],
        exists: true,
        is_symlink_canonical: false,
        package_type: "file",
        subagent_key: "reviewer",
        symlink_target: "/tmp/home/.agents/subagents/reviewer.md",
        model: null,
        tools: [],
        codex_tools_ignored: false,
      },
      {
        id: "sub-project",
        name: "Reviewer (Project)",
        description: "Project reviewer",
        scope: "project",
        workspace: "/tmp/workspace-a",
        canonical_source_path: "/tmp/workspace-a/.claude/agents/reviewer.md",
        target_paths: ["/tmp/workspace-a/.cursor/agents/reviewer.md"],
        exists: true,
        is_symlink_canonical: false,
        package_type: "file",
        subagent_key: "reviewer",
        symlink_target: "/tmp/workspace-a/.claude/agents/reviewer.md",
        model: null,
        tools: [],
        codex_tools_ignored: false,
      },
    ]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Subagents" }));
    await user.click(
      screen.getByRole("button", { name: /Reviewer \(Project\)/ }),
    );

    await waitFor(() => {
      expect(tauriApi.getSubagentDetails).toHaveBeenLastCalledWith(
        "sub-project",
      );
    });
  });

  it("renders only codex and claude toggles for global mcp server", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "global",
          workspace: null,
          transport: "http",
          command: null,
          args: [],
          url: "https://mcp.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: true,
            project: false,
          },
          targets: ["/tmp/home/.codex/config.toml"],
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

    await user.click(screen.getByRole("button", { name: "MCP" }));
    await screen.findByRole("heading", { name: "exa" });

    expect(
      screen.getByRole("switch", { name: "codex toggle" }),
    ).toHaveAttribute("aria-checked", "true");
    expect(
      screen.getByRole("switch", { name: "claude toggle" }),
    ).toHaveAttribute("aria-checked", "true");
    expect(
      screen.queryByRole("switch", { name: /project toggle/i }),
    ).not.toBeInTheDocument();
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

    await user.click(screen.getByRole("button", { name: "MCP" }));
    await screen.findByRole("heading", { name: "exa" });
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

  it("targets the correct workspace when same project mcp key appears twice", async () => {
    const workspaceA = "/tmp/workspace-a";
    const workspaceB = "/tmp/workspace-b";
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: workspaceA,
          transport: "http",
          command: null,
          args: [],
          url: "https://a.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: true,
            project: true,
          },
          targets: [`${workspaceA}/.mcp.json`],
          warnings: [],
        },
        {
          server_key: "exa",
          scope: "project",
          workspace: workspaceB,
          transport: "http",
          command: null,
          args: [],
          url: "https://b.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: true,
            project: true,
          },
          targets: [`${workspaceB}/.mcp.json`],
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

    await user.click(screen.getByRole("button", { name: "MCP" }));
    await screen.findAllByText(workspaceA);
    await user.click(screen.getAllByText(workspaceB)[0].closest("button")!);
    await screen.findByRole("heading", { name: "exa" });
    await user.click(screen.getByRole("switch", { name: "project toggle" }));

    await waitFor(() => {
      expect(tauriApi.setMcpServerEnabled).toHaveBeenCalledWith(
        "exa",
        "project",
        false,
        "project",
        workspaceB,
      );
    });
  });
});
