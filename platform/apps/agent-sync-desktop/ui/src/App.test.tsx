import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as tauriApi from "./tauriApi";
import type {
  AgentsContextReport,
  McpServerRecord,
  SkillDetails,
  SkillRecord,
  SyncState,
} from "./types";

vi.mock("./tauriApi", () => ({
  getRuntimeControls: vi.fn(),
  setAllowFilesystemChanges: vi.fn(),
  listAuditEvents: vi.fn(),
  clearAuditEvents: vi.fn(),
  getAgentsContextReport: vi.fn(),
  getState: vi.fn(),
  runSync: vi.fn(),
  runDotagentsSync: vi.fn(),
  listDotagentsSkills: vi.fn(),
  listDotagentsMcp: vi.fn(),
  migrateDotagents: vi.fn(),
  getSkillDetails: vi.fn(),
  getSubagentDetails: vi.fn(),
  listSubagents: vi.fn(),
  mutateCatalogItem: vi.fn(),
  mutateSkill: vi.fn(),
  renameSkill: vi.fn(),
  openSkillPath: vi.fn(),
  openSubagentPath: vi.fn(),
  getStarredSkillIds: vi.fn(),
  setSkillStarred: vi.fn(),
  setMcpServerEnabled: vi.fn(),
}));

let clipboardWriteSpy: ReturnType<typeof vi.fn>;
const CATALOG_FOCUS_STORAGE_KEY = "agent-sync.catalog.focusKind.v1";

function createLocalStorageMock(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => {
      values.clear();
    },
    getItem: (key: string) => values.get(key) ?? null,
    key: (index: number) => Array.from(values.keys())[index] ?? null,
    removeItem: (key: string) => {
      values.delete(key);
    },
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
  };
}

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
  syncWarnings: string[] = [],
): SyncState {
  const mcpRecordWarningCount = mcpServers.reduce(
    (total, item) => total + item.warnings.length,
    0,
  );
  return {
    version: 2,
    generated_at: "2026-02-20T17:00:00Z",
    sync: { status: "ok", error: null, warnings: syncWarnings },
    summary: {
      global_count: skills.filter(
        (skill) => skill.scope === "global" && skill.status === "active",
      ).length,
      project_count: skills.filter(
        (skill) => skill.scope === "project" && skill.status === "active",
      ).length,
      conflict_count: 0,
      mcp_count: mcpServers.length,
      mcp_warning_count:
        syncWarnings.length > 0 ? syncWarnings.length : mcpRecordWarningCount,
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

function buildAgentsReport(
  overrides?: Partial<AgentsContextReport>,
): AgentsContextReport {
  return {
    generated_at: "2026-02-23T18:00:00Z",
    limits: {
      include_max_depth: 5,
      file_warning_tokens: 2000,
      file_critical_tokens: 4000,
      total_warning_tokens: 8000,
      total_critical_tokens: 16000,
      tokens_formula: "ceil(rendered_chars / 4)",
    },
    totals: {
      roots_count: 1,
      rendered_chars: 3200,
      rendered_lines: 120,
      tokens_estimate: 800,
      include_count: 2,
      missing_include_count: 0,
      cycle_count: 0,
      max_depth_reached_count: 0,
      severity: "ok",
    },
    warning_count: 0,
    critical_count: 0,
    entries: [
      {
        id: "global|global|/tmp/home/AGENTS.md",
        scope: "global",
        workspace: null,
        root_path: "/tmp/home/AGENTS.md",
        exists: true,
        severity: "ok",
        raw_chars: 1200,
        raw_lines: 40,
        rendered_chars: 3200,
        rendered_lines: 120,
        tokens_estimate: 800,
        include_count: 2,
        missing_includes: [],
        cycles_detected: [],
        max_depth_reached: false,
        diagnostics: [],
        segments: [
          {
            path: "/tmp/home/AGENTS.md",
            depth: 0,
            chars: 1200,
            lines: 40,
            tokens_estimate: 300,
          },
          {
            path: "/tmp/home/shared/policy.md",
            depth: 1,
            chars: 2000,
            lines: 80,
            tokens_estimate: 500,
          },
        ],
      },
    ],
    ...overrides,
  };
}

function setApiDefaults(
  state: SyncState,
  detailsBySkillKey: Record<string, SkillDetails>,
) {
  vi.mocked(tauriApi.getState).mockResolvedValue(state);
  vi.mocked(tauriApi.getRuntimeControls).mockResolvedValue({
    allow_filesystem_changes: false,
    auto_watch_active: false,
  });
  vi.mocked(tauriApi.setAllowFilesystemChanges).mockResolvedValue({
    allow_filesystem_changes: true,
    auto_watch_active: true,
  });
  vi.mocked(tauriApi.listAuditEvents).mockResolvedValue([]);
  vi.mocked(tauriApi.clearAuditEvents).mockResolvedValue(undefined);
  vi.mocked(tauriApi.getAgentsContextReport).mockResolvedValue(
    buildAgentsReport(),
  );
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
  vi.mocked(tauriApi.runDotagentsSync).mockResolvedValue(undefined);
  vi.mocked(tauriApi.listDotagentsSkills).mockResolvedValue(state.skills);
  vi.mocked(tauriApi.listDotagentsMcp).mockResolvedValue(
    state.mcp_servers ?? [],
  );
  vi.mocked(tauriApi.migrateDotagents).mockResolvedValue(undefined);
  vi.mocked(tauriApi.mutateCatalogItem).mockResolvedValue(state);
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
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: createLocalStorageMock(),
  });
  clipboardWriteSpy = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", {
    clipboard: { writeText: clipboardWriteSpy },
  });
});

describe("App quiet redesign", () => {
  it("renders catalog tabs and shows only active list by default", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
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
          targets: ["/tmp/workspace-a/.mcp.json"],
          warnings: [],
        },
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(
      screen.getByRole("button", { name: "Switch catalog to Skills" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("button", { name: "Switch catalog to Subagents" }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.getByRole("button", { name: "Switch catalog to Agents.md" }),
    ).toHaveAttribute("aria-pressed", "false");

    expect(
      screen.queryByRole("heading", { name: "Skills" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Subagents" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "MCP" }),
    ).not.toBeInTheDocument();
    expect(screen.getAllByText("1/1")).toHaveLength(1);
  });

  it("switches to subagents tab and renders only subagent catalog list", async () => {
    const subagent = {
      id: "sub-agent-uno",
      name: "Agent Uno",
      description: "Specialized helper",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/agent-uno.md",
      target_paths: ["/tmp/home/.claude/agents/agent-uno.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "agent-uno",
      symlink_target: "/tmp/home/.claude/agents/agent-uno.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
    };
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([subagent]);
    vi.mocked(tauriApi.getSubagentDetails).mockResolvedValue({
      subagent,
      main_file_path: "/tmp/home/.claude/agents/agent-uno.md",
      main_file_exists: true,
      main_file_body_preview: "# Agent Uno",
      main_file_body_preview_truncated: false,
      subagent_dir_tree_preview: "agents/\n`-- agent-uno.md",
      subagent_dir_tree_preview_truncated: false,
      last_modified_unix_seconds: 1_700_000_000,
      target_statuses: [],
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(
      screen.queryByRole("button", { name: /Agent Uno/i }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to Subagents" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Agent Uno" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Agent Uno/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Project Skill/i }),
    ).not.toBeInTheDocument();
  });

  it("filters only active tab and keeps results hidden in inactive tabs", async () => {
    const subagent = {
      id: "sub-agent-search",
      name: "Agent Search",
      description: "Search specialist",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/agent-search.md",
      target_paths: ["/tmp/home/.claude/agents/agent-search.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "agent-search",
      symlink_target: "/tmp/home/.claude/agents/agent-search.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
    };
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([subagent]);
    vi.mocked(tauriApi.getSubagentDetails).mockResolvedValue({
      subagent,
      main_file_path: "/tmp/home/.claude/agents/agent-search.md",
      main_file_exists: true,
      main_file_body_preview: "# Agent Search",
      main_file_body_preview_truncated: false,
      subagent_dir_tree_preview: "agents/\n`-- agent-search.md",
      subagent_dir_tree_preview_truncated: false,
      last_modified_unix_seconds: 1_700_000_000,
      target_statuses: [],
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.type(
      screen.getByPlaceholderText("Search by name, key, scope or workspace"),
      "agent-search",
    );
    expect(screen.getByText("No skills found.")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Agent Search/i }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to Subagents" }),
    );
    expect(screen.queryByText("No subagents found.")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Agent Search/i }),
    ).toBeInTheDocument();
  });

  it("switches to Agents.md tab and renders agents details", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.getAgentsContextReport).mockResolvedValue(
      buildAgentsReport({
        entries: [
          {
            id: "global|global|/tmp/home/AGENTS.md",
            scope: "global",
            workspace: null,
            root_path: "/tmp/home/AGENTS.md",
            exists: true,
            severity: "ok",
            raw_chars: 1200,
            raw_lines: 40,
            rendered_chars: 3200,
            rendered_lines: 120,
            tokens_estimate: 800,
            include_count: 2,
            missing_includes: [],
            cycles_detected: [],
            max_depth_reached: false,
            diagnostics: [],
            segments: [
              {
                path: "/tmp/home/AGENTS.md",
                depth: 0,
                chars: 1200,
                lines: 40,
                tokens_estimate: 300,
              },
            ],
          },
          {
            id: "project|/tmp/workspace|/tmp/workspace/AGENTS.md",
            scope: "project",
            workspace: "/tmp/workspace",
            root_path: "/tmp/workspace/AGENTS.md",
            exists: true,
            severity: "warning",
            raw_chars: 5000,
            raw_lines: 140,
            rendered_chars: 8800,
            rendered_lines: 220,
            tokens_estimate: 2200,
            include_count: 4,
            missing_includes: ["missing include"],
            cycles_detected: [],
            max_depth_reached: false,
            diagnostics: ["missing include"],
            segments: [
              {
                path: "/tmp/workspace/AGENTS.md",
                depth: 0,
                chars: 5000,
                lines: 140,
                tokens_estimate: 1250,
              },
            ],
          },
        ],
      }),
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to Agents.md" }),
    );

    expect(
      screen.getByRole("button", { name: "Switch catalog to Agents.md" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      await screen.findByRole("heading", { name: "Global AGENTS.md" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Top segments")).toBeInTheDocument();
    expect(screen.getAllByText("/tmp/home/AGENTS.md").length).toBeGreaterThan(
      0,
    );
  });

  it("shows agents header indicator with warning and critical counts", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.getAgentsContextReport).mockResolvedValue(
      buildAgentsReport({
        warning_count: 2,
        critical_count: 1,
        totals: {
          roots_count: 3,
          rendered_chars: 36_000,
          rendered_lines: 1200,
          tokens_estimate: 9000,
          include_count: 8,
          missing_include_count: 2,
          cycle_count: 1,
          max_depth_reached_count: 1,
          severity: "warning",
        },
      }),
    );

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const indicator = await screen.findByTestId("agents-context-indicator");
    expect(indicator).toHaveTextContent("Agents context");
    expect(indicator).toHaveTextContent("9000 est");
    expect(indicator).toHaveTextContent("warnings 2 / critical 1");
  });

  it("filters agents entries by workspace/path/scope/severity", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.getAgentsContextReport).mockResolvedValue(
      buildAgentsReport({
        entries: [
          {
            id: "global|global|/tmp/home/AGENTS.md",
            scope: "global",
            workspace: null,
            root_path: "/tmp/home/AGENTS.md",
            exists: true,
            severity: "ok",
            raw_chars: 100,
            raw_lines: 4,
            rendered_chars: 100,
            rendered_lines: 4,
            tokens_estimate: 25,
            include_count: 0,
            missing_includes: [],
            cycles_detected: [],
            max_depth_reached: false,
            diagnostics: [],
            segments: [],
          },
          {
            id: "project|/tmp/workspace-a|/tmp/workspace-a/AGENTS.md",
            scope: "project",
            workspace: "/tmp/workspace-a",
            root_path: "/tmp/workspace-a/AGENTS.md",
            exists: true,
            severity: "critical",
            raw_chars: 20_000,
            raw_lines: 450,
            rendered_chars: 20_000,
            rendered_lines: 450,
            tokens_estimate: 5000,
            include_count: 0,
            missing_includes: [],
            cycles_detected: [],
            max_depth_reached: false,
            diagnostics: [],
            segments: [],
          },
        ],
        warning_count: 0,
        critical_count: 1,
        totals: {
          roots_count: 2,
          rendered_chars: 20_100,
          rendered_lines: 454,
          tokens_estimate: 5025,
          include_count: 0,
          missing_include_count: 0,
          cycle_count: 0,
          max_depth_reached_count: 0,
          severity: "ok",
        },
      }),
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.type(
      screen.getByPlaceholderText("Search by name, key, scope or workspace"),
      "workspace-a",
    );
    expect(screen.getByText("No skills found.")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to Agents.md" }),
    );
    expect(screen.getByText(/workspace-a/)).toBeInTheDocument();

    await user.clear(
      screen.getByPlaceholderText("Search by name, key, scope or workspace"),
    );
    await user.type(
      screen.getByPlaceholderText("Search by name, key, scope or workspace"),
      "critical",
    );
    expect(screen.getByText(/workspace-a/)).toBeInTheDocument();
  });

  it("persists selected tab and restores it after remount", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
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
          targets: ["/tmp/workspace-a/.mcp.json"],
          warnings: [],
        },
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    const firstRender = render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );

    expect(
      await screen.findByRole("heading", { name: "exa" }),
    ).toBeInTheDocument();
    expect(window.localStorage.getItem(CATALOG_FOCUS_STORAGE_KEY)).toBe("mcp");
    firstRender.unmount();

    render(<App />);
    await screen.findByRole("heading", { name: "exa" });
    expect(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    ).toHaveAttribute("aria-pressed", "true");
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

  it("renders overflow actions and confirms delete in dialog", async () => {
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
    expect(confirmButton).toBeEnabled();

    await user.click(confirmButton);

    expect(tauriApi.mutateCatalogItem).toHaveBeenCalledWith({
      action: "delete",
      target: { kind: "skill", skillKey: projectSkill.skill_key },
      confirmed: true,
    });
  });

  it("archives subagent from lifecycle menu", async () => {
    const subagent = {
      id: "sub-agent-archive",
      name: "Agent Archive",
      description: "Lifecycle test",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/agent-archive.md",
      target_paths: ["/tmp/home/.claude/agents/agent-archive.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "agent-archive",
      symlink_target: "/tmp/home/.claude/agents/agent-archive.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
      status: "active" as const,
    };
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([subagent]);
    vi.mocked(tauriApi.getSubagentDetails).mockResolvedValue({
      subagent,
      main_file_path: subagent.canonical_source_path,
      main_file_exists: true,
      main_file_body_preview: "# Agent Archive",
      main_file_body_preview_truncated: false,
      subagent_dir_tree_preview: "agents/\n`-- agent-archive.md",
      subagent_dir_tree_preview_truncated: false,
      last_modified_unix_seconds: 1_700_000_000,
      target_statuses: [],
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });
    await user.click(
      screen.getByRole("button", { name: "Switch catalog to Subagents" }),
    );
    await screen.findByRole("heading", { name: "Agent Archive" });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Archive" }));

    expect(tauriApi.mutateCatalogItem).toHaveBeenCalledWith({
      action: "archive",
      target: { kind: "subagent", subagentId: subagent.id },
      confirmed: true,
    });
  });

  it("restores and deletes archived mcp server via shared dialog", async () => {
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
          targets: [],
          warnings: [],
          status: "archived",
          archived_at: "2026-02-25T09:10:11Z",
        },
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });
    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );
    await screen.findByRole("heading", { name: "exa" });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    expect(
      screen.getByRole("menuitem", { name: "Restore" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(
      screen.getByText(
        'Remove MCP server "exa"? This action moves files to system Trash.',
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(tauriApi.mutateCatalogItem).toHaveBeenCalledWith({
      action: "delete",
      target: {
        kind: "mcp",
        serverKey: "exa",
        scope: "project",
        workspace,
      },
      confirmed: true,
    });
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
    vi.mocked(tauriApi.mutateCatalogItem).mockReturnValue(pendingMutation);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Archive" }));
    expect(tauriApi.mutateCatalogItem).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "More actions" }));
    const archiveAgain = screen.queryByRole("menuitem", { name: "Archive" });
    if (archiveAgain) {
      await user.click(archiveAgain);
    }
    expect(tauriApi.mutateCatalogItem).toHaveBeenCalledTimes(1);

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

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );
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

  it("shows agent icons next to toggles in MCP details", async () => {
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
            project: false,
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

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );
    await user.click(screen.getByRole("button", { name: /exa/i }));

    const enableSection = screen
      .getByText("Enable by agent")
      .closest("section");
    expect(enableSection).not.toBeNull();
    const section = enableSection ?? document.body;
    const codexIcon = within(section).getByRole("img", {
      name: "codex agent",
    });
    const claudeIcon = within(section).getByRole("img", {
      name: "claude agent",
    });
    const projectIcon = within(section).getByRole("img", {
      name: "project agent",
    });
    expect(codexIcon).toBeInTheDocument();
    expect(claudeIcon).toBeInTheDocument();
    expect(projectIcon).toBeInTheDocument();
    expect(codexIcon).toHaveClass("text-emerald-500");
    expect(claudeIcon).toHaveClass("text-emerald-500");
    expect(projectIcon).toHaveClass("text-muted-foreground/70", "opacity-60");
  });

  it("renders sync warning banner with expandable warning list", async () => {
    const state = buildState(
      [projectSkill],
      [],
      [
        "Broken unmanaged Claude MCP 'claude-mem' in /tmp/home/.claude.json: stdio interpreter arg path does not exist: /tmp/missing/claude-mem.js",
        "MCP server 'exa' has inline secret-like argument '--foo_token=<redacted>'",
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const banner = screen.getByTestId("sync-warning-banner");
    expect(banner).toHaveTextContent("Sync warnings (2)");
    expect(
      screen.queryByText(/Broken unmanaged Claude MCP 'claude-mem'/i),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Show warnings" }));
    expect(
      screen.getByText(/Broken unmanaged Claude MCP 'claude-mem'/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/--foo_token=<redacted>/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Hide warnings" }));
    expect(
      screen.queryByText(/Broken unmanaged Claude MCP 'claude-mem'/i),
    ).not.toBeInTheDocument();
  });

  it("shows merged MCP warnings from record and sync warning feed", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
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
          targets: ["/tmp/workspace-a/.mcp.json"],
          warnings: ["record warning: exa needs auth refresh"],
        },
      ],
      [
        "MCP server 'exa' has inline secret-like argument '--foo_token=<redacted>'",
        "MCP server 'other' has inline secret-like argument '--bar_token=<redacted>'",
        "MCP server global::exa2 has outdated credential hint",
      ],
    );
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );
    await user.click(screen.getByRole("button", { name: /exa/i }));

    expect(screen.getByText("Warnings")).toBeInTheDocument();
    expect(
      screen.getByText("record warning: exa needs auth refresh"),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/MCP server 'exa' has inline secret-like argument/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/MCP server 'other' has inline secret-like argument/i),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        /MCP server global::exa2 has outdated credential hint/i,
      ),
    ).not.toBeInTheDocument();
  });

  it("renders MCP catalog row as two lines with transport and connected agents", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
          transport: "http",
          command: null,
          args: [],
          url: "https://mcp.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: false,
            project: true,
          },
          targets: ["/tmp/workspace-a/.mcp.json"],
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

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );

    const row = screen.getByRole("button", { name: /exa/i });
    expect(within(row).getByText("Project")).toBeInTheDocument();
    expect(within(row).getByText("HTTP")).toBeInTheDocument();
    expect(within(row).getByText("/tmp/workspace-a")).toBeInTheDocument();
    expect(within(row).queryByText("ON")).not.toBeInTheDocument();
    expect(within(row).queryByText("OFF")).not.toBeInTheDocument();
    expect(
      within(row).getByRole("img", { name: "codex connected" }),
    ).toBeInTheDocument();
    expect(
      within(row).getByRole("img", { name: "project connected" }),
    ).toBeInTheDocument();
    expect(
      within(row).getByRole("img", { name: "claude disabled" }),
    ).toBeInTheDocument();
  });

  it("shows workspace labels for duplicate project MCP server keys", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
          transport: "http",
          command: null,
          args: [],
          url: "https://mcp.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: true,
            claude: false,
            project: true,
          },
          targets: ["/tmp/workspace-a/.mcp.json"],
          warnings: [],
        },
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-b",
          transport: "http",
          command: null,
          args: [],
          url: "https://mcp.exa.ai/mcp",
          env: {},
          enabled_by_agent: {
            codex: false,
            claude: true,
            project: true,
          },
          targets: ["/tmp/workspace-b/.mcp.json"],
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

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );

    expect(screen.getByText("/tmp/workspace-a")).toBeInTheDocument();
    expect(screen.getByText("/tmp/workspace-b")).toBeInTheDocument();
  });

  it("hides project agent logo in MCP rows for global scope", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "ahrefs",
          scope: "global",
          workspace: null,
          transport: "stdio",
          command: "npx",
          args: ["-y", "@ahrefs/mcp-server"],
          url: null,
          env: {},
          enabled_by_agent: {
            codex: false,
            claude: true,
            project: true,
          },
          targets: [],
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

    await user.click(
      screen.getByRole("button", { name: "Switch catalog to MCP" }),
    );

    const row = screen.getByRole("button", { name: /ahrefs/i });
    expect(within(row).getByText("Global")).toBeInTheDocument();
    expect(within(row).getByText("STDIO")).toBeInTheDocument();
    expect(within(row).queryByText("ON")).not.toBeInTheDocument();
    expect(within(row).queryByText("OFF")).not.toBeInTheDocument();
    expect(
      within(row).queryByRole("img", { name: /project connected/i }),
    ).not.toBeInTheDocument();
    expect(
      within(row).getByRole("img", { name: "claude connected" }),
    ).toBeInTheDocument();
    expect(
      within(row).getByRole("img", { name: "codex disabled" }),
    ).toBeInTheDocument();
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

  it("does not run sync on startup when allow is disabled", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(tauriApi.runSync).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  it("runs sync only after allow is enabled", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    const user = userEvent.setup();

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const syncButton = screen.getByRole("button", { name: "Sync" });
    expect(syncButton).toBeDisabled();
    expect(tauriApi.runSync).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );

    await waitFor(() => {
      expect(tauriApi.setAllowFilesystemChanges).toHaveBeenCalledWith(true);
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Sync" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Sync" }));
    await waitFor(() => {
      expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    });
  });

  it("refreshes subagents and agents report after sync completes", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    const user = userEvent.setup();

    let resolveRunSync: ((value: SyncState) => void) | null = null;
    const runSyncPromise = new Promise<SyncState>((resolve) => {
      resolveRunSync = resolve;
    });
    vi.mocked(tauriApi.runSync).mockReturnValue(runSyncPromise);

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Sync" })).toBeEnabled();
    });

    const subagentCallsBeforeSync = vi.mocked(tauriApi.listSubagents).mock.calls
      .length;
    const reportCallsBeforeSync = vi.mocked(tauriApi.getAgentsContextReport)
      .mock.calls.length;

    await user.click(screen.getByRole("button", { name: "Sync" }));
    await waitFor(() => {
      expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    });

    expect(vi.mocked(tauriApi.listSubagents).mock.calls.length).toBe(
      subagentCallsBeforeSync,
    );
    expect(vi.mocked(tauriApi.getAgentsContextReport).mock.calls.length).toBe(
      reportCallsBeforeSync,
    );

    resolveRunSync?.(state);
    await waitFor(() => {
      expect(vi.mocked(tauriApi.listSubagents).mock.calls.length).toBe(
        subagentCallsBeforeSync + 1,
      );
      expect(vi.mocked(tauriApi.getAgentsContextReport).mock.calls.length).toBe(
        reportCallsBeforeSync + 1,
      );
    });
  });

  it("verifies dotagents via UI and shows counts", async () => {
    const state = buildState(
      [projectSkill],
      [
        {
          server_key: "exa",
          scope: "project",
          workspace: "/tmp/workspace-a",
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
          targets: ["/tmp/workspace-a/.mcp.json"],
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

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );
    await waitFor(() => {
      expect(tauriApi.setAllowFilesystemChanges).toHaveBeenCalledWith(true);
    });

    await user.click(screen.getByRole("button", { name: "Verify dotagents" }));

    await waitFor(() => {
      expect(tauriApi.runDotagentsSync).toHaveBeenCalledWith("all");
      expect(tauriApi.listDotagentsSkills).toHaveBeenCalledWith("all");
      expect(tauriApi.listDotagentsMcp).toHaveBeenCalledWith("all");
    });
    const proof = await screen.findByTestId("dotagents-proof");
    expect(proof).toHaveAttribute("data-status", "ok");
    expect(proof).toHaveTextContent("Dotagents");
    expect(proof).toHaveTextContent("skills=1, mcp=1");
  });

  it("lets user initialize dotagents from UI after migration-required error", async () => {
    const state = buildState([projectSkill], []);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.runDotagentsSync)
      .mockRejectedValueOnce(
        new Error(
          "migration required before strict dotagents sync: user scope is not initialized",
        ),
      )
      .mockResolvedValueOnce(undefined);
    vi.mocked(tauriApi.listDotagentsSkills).mockResolvedValue([
      {
        ...projectSkill,
      },
    ]);
    vi.mocked(tauriApi.listDotagentsMcp).mockResolvedValue([]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );
    await waitFor(() => {
      expect(tauriApi.setAllowFilesystemChanges).toHaveBeenCalledWith(true);
    });

    await user.click(screen.getByRole("button", { name: "Verify dotagents" }));

    expect(
      await screen.findByRole("button", { name: "Initialize dotagents" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Initialize dotagents" }),
    );

    await waitFor(() => {
      expect(tauriApi.migrateDotagents).toHaveBeenCalledWith("all");
    });
    const proof = await screen.findByTestId("dotagents-proof");
    expect(proof).toHaveAttribute("data-status", "ok");
    expect(proof).toHaveTextContent("skills=1, mcp=0");
  });

  it("infers project scope for dotagents initialization after migration-required error", async () => {
    const state = buildState([projectSkill], []);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.runDotagentsSync)
      .mockRejectedValueOnce(
        new Error(
          "migration required before strict dotagents sync: project scope is not initialized for 1 workspace(s): /tmp/workspace-a; run `agent-sync migrate-dotagents --scope project`",
        ),
      )
      .mockResolvedValueOnce(undefined);
    vi.mocked(tauriApi.listDotagentsSkills).mockResolvedValue([
      {
        ...projectSkill,
      },
    ]);
    vi.mocked(tauriApi.listDotagentsMcp).mockResolvedValue([]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );
    await waitFor(() => {
      expect(tauriApi.setAllowFilesystemChanges).toHaveBeenCalledWith(true);
    });

    await user.click(screen.getByRole("button", { name: "Verify dotagents" }));

    expect(
      await screen.findByRole("button", { name: "Initialize dotagents" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Initialize dotagents" }),
    );

    await waitFor(() => {
      expect(tauriApi.migrateDotagents).toHaveBeenCalledWith("all");
    });
    const proof = await screen.findByTestId("dotagents-proof");
    expect(proof).toHaveAttribute("data-status", "ok");
    expect(proof).toHaveTextContent("skills=1, mcp=0");
  });

  it("shows guidance when dotagents init fails because agents.toml already exists", async () => {
    const state = buildState([projectSkill], []);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.runDotagentsSync).mockRejectedValueOnce(
      new Error(
        "migration required before strict dotagents sync: user scope is not initialized",
      ),
    );
    vi.mocked(tauriApi.migrateDotagents).mockRejectedValueOnce(
      new Error("agents.toml already exists. Use --force to overwrite."),
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(
      screen.getByRole("switch", { name: "Allow filesystem changes" }),
    );
    await waitFor(() => {
      expect(tauriApi.setAllowFilesystemChanges).toHaveBeenCalledWith(true);
    });

    await user.click(screen.getByRole("button", { name: "Verify dotagents" }));
    await user.click(
      await screen.findByRole("button", { name: "Initialize dotagents" }),
    );

    const proof = await screen.findByTestId("dotagents-proof");
    expect(proof).toHaveAttribute("data-status", "error");
    expect(proof).toHaveTextContent("Dotagents initialization failed");
    expect(proof).toHaveTextContent("agents.toml already exists");
  });

  it("opens audit log panel and renders events", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listAuditEvents).mockResolvedValue([
      {
        id: "evt-1",
        occurred_at: "2026-02-21T12:00:00Z",
        action: "run_sync",
        status: "success",
        trigger: "manual",
        summary: "target paths +1 -0, canonical shifts 0",
        paths: ["/tmp/a", "/tmp/b"],
        details: null,
      },
    ]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Audit log" }));

    expect(
      screen.getByRole("dialog", { name: "Audit log" }),
    ).toBeInTheDocument();
    expect(await screen.findByText(/run_sync/i)).toBeInTheDocument();
  });

  it("opens clear logs confirm dialog from audit log", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Audit log" }));
    await user.click(screen.getByRole("button", { name: "Clear logs" }));

    expect(
      screen.getByRole("dialog", { name: "Clear audit logs" }),
    ).toBeInTheDocument();
  });

  it("does not clear audit log when cancel is clicked", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Audit log" }));
    await user.click(screen.getByRole("button", { name: "Clear logs" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(tauriApi.clearAuditEvents).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("dialog", { name: "Clear audit logs" }),
    ).not.toBeInTheDocument();
  });

  it("clears audit log after confirm and reloads empty state", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.listAuditEvents)
      .mockResolvedValueOnce([
        {
          id: "evt-1",
          occurred_at: "2026-02-21T12:00:00Z",
          action: "run_sync",
          status: "success",
          trigger: "manual",
          summary: "target paths +0 -0, canonical shifts 0, mcp changes 1",
          paths: [],
          details: "MCP updated (1): global::-::exa",
        },
      ])
      .mockResolvedValueOnce([]);

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Audit log" }));
    expect(await screen.findByText(/run_sync/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear logs" }));
    await user.click(screen.getByRole("button", { name: "Confirm" }));

    await waitFor(() => {
      expect(tauriApi.clearAuditEvents).toHaveBeenCalledTimes(1);
      expect(tauriApi.listAuditEvents).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText("No audit events.")).toBeInTheDocument();
  });

  it("shows backend blocked error when write action is denied", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    vi.mocked(tauriApi.mutateCatalogItem).mockRejectedValueOnce(
      new Error(
        "Filesystem changes are disabled. Enable 'Allow filesystem changes' to run mutate_catalog_item.",
      ),
    );

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "More actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText(/Filesystem changes are disabled/i),
    ).toBeInTheDocument();
  });
});
