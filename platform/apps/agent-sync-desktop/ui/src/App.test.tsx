import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as tauriApi from "./tauriApi";
import type {
  AppContext,
  DotagentsCommandResult,
  DotagentsMcpListItem,
  DotagentsRuntimeStatus,
  DotagentsSkillListItem,
} from "./types";

vi.mock("./tauriApi", () => ({
  getRuntimeStatus: vi.fn(),
  getAppContext: vi.fn(),
  setScope: vi.fn(),
  setProjectRoot: vi.fn(),
  listSkills: vi.fn(),
  listMcpServers: vi.fn(),
  runDotagentsCommand: vi.fn(),
  openAgentsToml: vi.fn(),
  openAgentsDir: vi.fn(),
  openUserHome: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

const runtimeReady: DotagentsRuntimeStatus = {
  available: true,
  expectedVersion: "1.4.0",
};

function buildContext(overrides: Partial<AppContext> = {}): AppContext {
  return {
    activeProjectContext: {
      mode: "user",
      projectRoot: null,
    },
    userHome: "/Users/tester",
    userAgentsDir: "/Users/tester/.agents",
    userAgentsTomlPath: "/Users/tester/.agents/agents.toml",
    userInitialized: true,
    projectAgentsTomlPath: null,
    projectInitialized: null,
    ...overrides,
  };
}

function buildProjectContext(overrides: Partial<AppContext> = {}): AppContext {
  return {
    ...buildContext(),
    activeProjectContext: {
      mode: "project",
      projectRoot: "/tmp/workspace",
    },
    projectAgentsTomlPath: "/tmp/workspace/agents.toml",
    projectInitialized: true,
    ...overrides,
  };
}

function commandResult(
  overrides: Partial<DotagentsCommandResult> = {},
): DotagentsCommandResult {
  return {
    success: true,
    command: "dotagents sync",
    cwd: "/tmp/workspace",
    scope: "project",
    exitCode: 0,
    durationMs: 32,
    stdout: "done",
    stderr: "",
    ...overrides,
  };
}

const sampleSkills: DotagentsSkillListItem[] = [
  {
    name: "lint",
    source: "owner/repo",
    status: "ok",
    commit: "deadbeef",
  },
  {
    name: "shared",
    source: "owner/repo",
    status: "ok",
    wildcard: "owner/repo",
  },
];

const sampleMcp: DotagentsMcpListItem[] = [
  {
    name: "github",
    transport: "stdio",
    target: "npx",
    env: ["GITHUB_TOKEN"],
  },
];

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(tauriApi.getRuntimeStatus).mockResolvedValue(runtimeReady);
  vi.mocked(tauriApi.getAppContext).mockResolvedValue(buildContext());
  vi.mocked(tauriApi.setScope).mockResolvedValue(buildProjectContext());
  vi.mocked(tauriApi.setProjectRoot).mockResolvedValue(buildProjectContext());
  vi.mocked(tauriApi.listSkills).mockResolvedValue(sampleSkills);
  vi.mocked(tauriApi.listMcpServers).mockResolvedValue(sampleMcp);
  vi.mocked(tauriApi.runDotagentsCommand).mockResolvedValue(commandResult());
  vi.mocked(tauriApi.openAgentsToml).mockResolvedValue();
  vi.mocked(tauriApi.openAgentsDir).mockResolvedValue();
  vi.mocked(tauriApi.openUserHome).mockResolvedValue();
});

describe("Dotagents Desktop UI", () => {
  it("switches between user and project scope", async () => {
    const user = userEvent.setup();
    let currentContext = buildContext();
    vi.mocked(tauriApi.getAppContext).mockImplementation(() =>
      Promise.resolve(currentContext),
    );
    vi.mocked(tauriApi.setScope).mockImplementation((scope) => {
      currentContext =
        scope === "project"
          ? buildProjectContext({
              activeProjectContext: { mode: "project", projectRoot: null },
              projectAgentsTomlPath: null,
              projectInitialized: null,
            })
          : buildContext();
      return Promise.resolve(currentContext);
    });

    render(<App />);

    await screen.findByText("User scope");
    await user.click(screen.getByRole("button", { name: "Project" }));

    await waitFor(() => {
      expect(tauriApi.setScope).toHaveBeenCalledWith("project");
    });
    await waitFor(() => {
      expect(screen.getByText("Project scope")).toBeInTheDocument();
    });
  });

  it("shows project empty state when no folder is selected", async () => {
    vi.mocked(tauriApi.getAppContext).mockResolvedValue(
      buildProjectContext({
        activeProjectContext: { mode: "project", projectRoot: null },
        projectAgentsTomlPath: null,
        projectInitialized: null,
      }),
    );

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Choose a project folder" }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: "Choose project folder" }).length,
    ).toBeGreaterThan(0);
  });

  it("renders skills list with status badges and MCP servers", async () => {
    render(<App />);

    expect(await screen.findByText("lint")).toBeInTheDocument();
    expect(screen.getByText("shared")).toBeInTheDocument();
    expect(screen.getByText(/wildcard/i)).toBeInTheDocument();

    expect(screen.getByText("github")).toBeInTheDocument();
    expect(screen.getByText("stdio")).toBeInTheDocument();
    expect(screen.getByText("npx")).toBeInTheDocument();
  });

  it("shows sync button as disabled when all skills are ok", async () => {
    render(<App />);

    const syncButtons = await screen.findAllByRole("button", {
      name: "All synced",
    });
    expect(syncButtons[0]).toBeDisabled();
  });

  it("shows sync button as active when skills need sync", async () => {
    vi.mocked(tauriApi.listSkills).mockResolvedValue([
      { name: "lint", source: "owner/repo", status: "modified" },
      { name: "missing-skill", source: "owner/repo", status: "missing" },
    ]);

    render(<App />);

    const syncButton = await screen.findByRole("button", {
      name: "Sync needed",
    });
    expect(syncButton).toBeEnabled();
  });

  it("shows fix hints for skills with non-ok status", async () => {
    vi.mocked(tauriApi.listSkills).mockResolvedValue([
      { name: "mod-skill", source: "owner/repo", status: "modified" },
      { name: "miss-skill", source: "owner/repo", status: "missing" },
      { name: "unlock-skill", source: "owner/repo", status: "unlocked" },
    ]);

    render(<App />);

    expect(
      await screen.findByText(
        "Local changes — sync will reset to declared state",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Not installed — sync will install"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Not pinned — sync will lock to a commit"),
    ).toBeInTheDocument();
  });

  it("runs sync when sync button is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriApi.listSkills).mockResolvedValue([
      { name: "lint", source: "owner/repo", status: "modified" },
    ]);

    render(<App />);

    const syncButton = await screen.findByRole("button", {
      name: "Sync needed",
    });
    await user.click(syncButton);

    await waitFor(() => {
      expect(tauriApi.runDotagentsCommand).toHaveBeenCalledWith({
        kind: "sync",
      });
    });

    expect(await screen.findByText("Output")).toBeInTheDocument();
    expect(screen.getByText("dotagents sync")).toBeInTheDocument();
    expect(screen.getByText("done")).toBeInTheDocument();
  });

  it("requires confirmation before removing a skill", async () => {
    const user = userEvent.setup();

    render(<App />);

    const removeButtons = await screen.findAllByRole("button", {
      name: "Remove",
    });
    await user.click(removeButtons[0]);

    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Confirm remove" }),
    ).toBeInTheDocument();
    expect(tauriApi.runDotagentsCommand).not.toHaveBeenCalledWith({
      kind: "skillRemove",
      name: "lint",
    });

    await user.click(screen.getByRole("button", { name: "Confirm remove" }));

    await waitFor(() => {
      expect(tauriApi.runDotagentsCommand).toHaveBeenCalledWith({
        kind: "skillRemove",
        name: "lint",
      });
    });
  });

  it("allows canceling a pending skill removal", async () => {
    const user = userEvent.setup();

    render(<App />);

    const removeButtons = await screen.findAllByRole("button", {
      name: "Remove",
    });
    await user.click(removeButtons[0]);
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(
      screen.queryByRole("button", { name: "Confirm remove" }),
    ).not.toBeInTheDocument();
    expect(tauriApi.runDotagentsCommand).not.toHaveBeenCalledWith({
      kind: "skillRemove",
      name: "lint",
    });
  });

  it("requires confirmation before removing an MCP server", async () => {
    const user = userEvent.setup();

    render(<App />);

    const removeButtons = await screen.findAllByRole("button", {
      name: "Remove",
    });
    await user.click(removeButtons[2]);
    await user.click(screen.getByRole("button", { name: "Confirm remove" }));

    await waitFor(() => {
      expect(tauriApi.runDotagentsCommand).toHaveBeenCalledWith({
        kind: "mcpRemove",
        name: "github",
      });
    });
  });

  it("shows missing-config empty states for project and user scopes", async () => {
    let currentContext = buildProjectContext({
      activeProjectContext: { mode: "project", projectRoot: "/tmp/workspace" },
      projectInitialized: false,
    });
    vi.mocked(tauriApi.getAppContext).mockImplementation(() =>
      Promise.resolve(currentContext),
    );
    vi.mocked(tauriApi.setScope).mockImplementation(() => {
      currentContext = buildContext({
        userInitialized: false,
      });
      return Promise.resolve(currentContext);
    });

    const user = userEvent.setup();
    render(<App />);

    expect(
      await screen.findByRole("heading", {
        name: "Selected folder is not initialized",
      }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Switch to user scope" }),
    );

    expect(
      await screen.findByRole("heading", {
        name: "User scope is not initialized",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: "Open ~/.agents" }).length,
    ).toBeGreaterThan(0);
  });
});
