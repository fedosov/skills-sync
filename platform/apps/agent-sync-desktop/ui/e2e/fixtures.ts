/**
 * Shared Playwright fixtures that mock Tauri's invoke IPC for browser-level e2e tests.
 *
 * The Vite dev server runs without the Tauri backend, so all `invoke()` calls
 * hit `window.__TAURI_INTERNALS__.invoke`. We intercept the page before load
 * and inject a mock handler that returns realistic fixture data.
 */

import { test as base, type Page } from "@playwright/test";

/* ------------------------------------------------------------------ */
/* Fixture data                                                        */
/* ------------------------------------------------------------------ */

const SYNC_STATE = {
  version: 1,
  generated_at: "2026-03-05T00:00:00Z",
  sync: { status: "ok", error: null, warnings: [] },
  summary: {
    global_count: 1,
    project_count: 1,
    conflict_count: 0,
    mcp_count: 1,
    mcp_warning_count: 0,
  },
  subagent_summary: {
    global_count: 0,
    project_count: 1,
    conflict_count: 0,
    mcp_count: 0,
    mcp_warning_count: 0,
  },
  skills: [
    {
      id: "global-1",
      name: "Global Skill",
      scope: "global",
      workspace: null,
      canonical_source_path: "/home/user/.config/ai-agents/skills/global-skill",
      target_paths: ["/home/user/.config/ai-agents/skills/global-skill"],
      status: "active",
      package_type: "dir",
      skill_key: "global-skill",
    },
    {
      id: "project-1",
      name: "Project Skill",
      scope: "project",
      workspace: "/tmp/workspace",
      canonical_source_path: "/tmp/workspace/.claude/skills/project-skill",
      target_paths: ["/tmp/workspace/.claude/skills/project-skill"],
      status: "active",
      package_type: "dir",
      skill_key: "project-skill",
    },
  ],
  subagents: [],
  mcp_servers: [
    {
      server_key: "test-mcp",
      scope: "global",
      workspace: null,
      transport: "stdio",
      command: "node",
      args: ["server.js"],
      url: null,
      env: {},
      enabled_by_agent: { codex: true, claude: true, project: false },
      targets: [],
      warnings: [],
    },
  ],
  top_skills: ["global-skill"],
  top_subagents: [],
};

const RUNTIME_CONTROLS = {
  allow_filesystem_changes: false,
  auto_watch_active: false,
};

const AGENTS_REPORT = {
  generated_at: "2026-03-05T00:00:00Z",
  limits: {
    include_max_depth: 5,
    file_warning_tokens: 5000,
    file_critical_tokens: 10000,
    total_warning_tokens: 20000,
    total_critical_tokens: 50000,
    tokens_formula: "chars / 4",
  },
  totals: {
    roots_count: 1,
    rendered_chars: 200,
    rendered_lines: 10,
    tokens_estimate: 50,
    include_count: 0,
    missing_include_count: 0,
    cycle_count: 0,
    max_depth_reached_count: 0,
    severity: "ok",
  },
  warning_count: 0,
  critical_count: 0,
  entries: [],
};

const SUBAGENTS = [
  {
    id: "sub-1",
    name: "Test Subagent",
    description: "A test subagent",
    scope: "project",
    workspace: "/tmp/workspace",
    canonical_source_path: "/tmp/workspace/.claude/agents/test-subagent.md",
    target_paths: ["/tmp/workspace/.claude/agents/test-subagent.md"],
    exists: true,
    is_symlink_canonical: true,
    package_type: "file",
    subagent_key: "test-subagent",
    symlink_target: "/tmp/workspace/.claude/agents/test-subagent.md",
    model: null,
    tools: [],
    codex_tools_ignored: false,
  },
];

/* ------------------------------------------------------------------ */
/* Invoke handler                                                      */
/* ------------------------------------------------------------------ */

const INVOKE_HANDLERS: Record<string, unknown> = {
  get_state: SYNC_STATE,
  get_runtime_controls: RUNTIME_CONTROLS,
  get_agents_context_report: AGENTS_REPORT,
  get_starred_skill_ids: [],
  list_subagents: SUBAGENTS,
  load_dashboard_snapshot: {
    state: SYNC_STATE,
    starredSkillIds: [],
    subagents: SUBAGENTS,
    agentsReport: AGENTS_REPORT,
  },
  list_audit_events: [],
  get_platform_context: { os: "macos", linux_desktop: null },
};

/* ------------------------------------------------------------------ */
/* Playwright fixture                                                  */
/* ------------------------------------------------------------------ */

async function injectTauriMock(page: Page) {
  // Inject fixture data first — init scripts run in registration order
  await page.addInitScript((h: Record<string, unknown>) => {
    (window as never as Record<string, unknown>).__E2E_INVOKE_HANDLERS__ = h;
  }, INVOKE_HANDLERS);

  await page.addInitScript(() => {
    const handlers: Record<string, unknown> = (
      window as never as Record<string, unknown>
    )["__E2E_INVOKE_HANDLERS__"] as Record<string, unknown>;

    (window as never as Record<string, unknown>).__TAURI_INTERNALS__ = {
      invoke: (cmd: string) => {
        if (handlers && cmd in handlers) {
          return Promise.resolve(JSON.parse(JSON.stringify(handlers[cmd])));
        }
        console.warn(`[e2e] unhandled invoke: ${cmd}`);
        return Promise.resolve(null);
      },
      metadata: { currentWebview: { label: "main" } },
    };
  });
}

export const test = base.extend<{ tauriPage: Page }>({
  tauriPage: async ({ page }, use) => {
    await injectTauriMock(page);
    await page.goto("/");
    await use(page);
  },
});

export { expect } from "@playwright/test";
