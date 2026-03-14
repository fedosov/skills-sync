import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  expectedVersion: "0.10.0",
  actualVersion: "0.10.0",
  binaryPath: "/tmp/bin/dotagents",
};

function buildUserContext(overrides: Partial<AppContext> = {}): AppContext {
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
    activeProjectContext: {
      mode: "project",
      projectRoot: "/tmp/workspace",
    },
    userHome: "/Users/tester",
    userAgentsDir: "/Users/tester/.agents",
    userAgentsTomlPath: "/Users/tester/.agents/agents.toml",
    userInitialized: true,
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
  vi.mocked(tauriApi.getAppContext).mockResolvedValue(buildUserContext());
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
    let currentContext = buildUserContext();
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
          : buildUserContext();
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

  it("renders skills and marks wildcard rows as non-removable", async () => {
    render(<App />);

    expect(await screen.findByText("lint")).toBeInTheDocument();
    const sharedRow = screen.getByText("shared").closest("div");
    expect(sharedRow).not.toBeNull();
    expect(screen.getByText(/managed by wildcard source/i)).toBeInTheDocument();
    const removeButtons = screen.getAllByRole("button", { name: "Remove" });
    expect(removeButtons[1]).toBeDisabled();
  });

  it("requires source plus explicit name or wildcard mode for add skill", async () => {
    render(<App />);

    const addSkillButton = await screen.findByRole("button", {
      name: "Add skill",
    });
    expect(addSkillButton).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText(/owner\/repo/i), {
      target: { value: "owner/repo" },
    });
    expect(addSkillButton).toBeDisabled();

    fireEvent.change(await screen.findByPlaceholderText("skill-name"), {
      target: { value: "lint" },
    });
    await waitFor(() => {
      expect(screen.getByDisplayValue("lint")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Add skill" })).toBeEnabled();
    });

    fireEvent.change(screen.getByPlaceholderText("skill-name"), {
      target: { value: "" },
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "Wildcard --all" }),
    );
    await waitFor(() => {
      expect(
        screen.queryByPlaceholderText("skill-name"),
      ).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Add skill" })).toBeEnabled();
    });
  });

  it("runs MCP add and remove flows", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "MCP" }));
    fireEvent.change(screen.getByPlaceholderText("github"), {
      target: { value: "remote" },
    });
    await user.click(await screen.findByRole("button", { name: "--url" }));
    fireEvent.change(
      screen.getByPlaceholderText("https://mcp.example.com/sse"),
      {
        target: { value: "https://mcp.example.com/sse" },
      },
    );
    await waitFor(() => {
      expect(screen.getByDisplayValue("remote")).toBeInTheDocument();
      expect(
        screen.getByDisplayValue("https://mcp.example.com/sse"),
      ).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Add MCP server" }));

    await waitFor(() => {
      expect(tauriApi.runDotagentsCommand).toHaveBeenCalledWith({
        kind: "mcpAddHttp",
        name: "remote",
        url: "https://mcp.example.com/sse",
        headers: [],
        env: [],
      });
    });

    await user.click(screen.getByRole("button", { name: "Remove" }));

    await waitFor(() => {
      expect(tauriApi.runDotagentsCommand).toHaveBeenCalledWith({
        kind: "mcpRemove",
        name: "github",
      });
    });
  });

  it("renders output transcripts for success and failure", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriApi.runDotagentsCommand)
      .mockResolvedValueOnce(
        commandResult({
          command: "dotagents sync",
          stdout: "synced",
          stderr: "",
        }),
      )
      .mockResolvedValueOnce(
        commandResult({
          success: false,
          command: "dotagents remove broken",
          exitCode: 1,
          stdout: "",
          stderr: "remove failed",
        }),
      );

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Sync" }));
    await user.click(screen.getByRole("button", { name: "Output" }));
    expect(await screen.findByText("dotagents sync")).toBeInTheDocument();
    expect(screen.getByText("synced")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Skills" }));
    await user.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => {
      expect(screen.getAllByText("remove failed").length).toBeGreaterThan(0);
    });
    expect(screen.getByText("dotagents remove broken")).toBeInTheDocument();
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
      currentContext = buildUserContext({
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
