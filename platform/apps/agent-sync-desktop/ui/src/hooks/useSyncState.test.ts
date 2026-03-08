import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSyncState } from "./useSyncState";
import * as tauriApi from "../tauriApi";
import type { DashboardSnapshot } from "../types";

vi.mock("../tauriApi", () => ({
  getRuntimeControls: vi.fn(),
  getAgentsContextReport: vi.fn(),
  getStarredSkillIds: vi.fn(),
  listSubagents: vi.fn(),
  loadDashboardSnapshot: vi.fn(),
  runSync: vi.fn(),
}));

function snapshot(version: string): DashboardSnapshot {
  return {
    state: {
      generated_at: version,
      sync: { status: "ok", error: null },
      summary: {
        global_count: 1,
        project_count: 0,
        conflict_count: 0,
        mcp_count: 0,
        mcp_warning_count: 0,
      },
      subagent_summary: {
        global_count: 0,
        project_count: 0,
        conflict_count: 0,
        mcp_count: 0,
        mcp_warning_count: 0,
      },
      skills: [
        {
          id: `skill-${version}`,
          name: `Skill ${version}`,
          scope: "global",
          workspace: null,
          canonical_source_path: "/tmp/skill",
          target_paths: [],
          status: "active",
          package_type: "dir",
          skill_key: `skill-${version}`,
        },
      ],
      subagents: [],
    },
    starredSkillIds: [],
    subagents: [],
    agentsReport: null,
  };
}

function agentsReport() {
  return {
    generated_at: "2026-03-08T00:00:00Z",
    limits: {
      include_max_depth: 5,
      file_warning_tokens: 1000,
      file_critical_tokens: 2000,
      total_warning_tokens: 3000,
      total_critical_tokens: 4000,
      tokens_formula: "chars / 4",
    },
    totals: {
      roots_count: 0,
      rendered_chars: 0,
      rendered_lines: 0,
      tokens_estimate: 0,
      include_count: 0,
      missing_include_count: 0,
      cycle_count: 0,
      max_depth_reached_count: 0,
      severity: "ok" as const,
    },
    warning_count: 0,
    critical_count: 0,
    entries: [],
  };
}

describe("useSyncState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauriApi.getAgentsContextReport).mockResolvedValue(
      agentsReport(),
    );
    vi.mocked(tauriApi.getStarredSkillIds).mockResolvedValue([]);
    vi.mocked(tauriApi.listSubagents).mockResolvedValue([]);
  });

  it("ignores stale refresh responses", async () => {
    vi.mocked(tauriApi.getRuntimeControls).mockResolvedValue({
      allow_filesystem_changes: true,
      auto_watch_active: false,
    });

    let resolveFirst:
      | ((value: DashboardSnapshot | PromiseLike<DashboardSnapshot>) => void)
      | null = null;
    vi.mocked(tauriApi.loadDashboardSnapshot)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockResolvedValueOnce(snapshot("latest"));

    const { result } = renderHook(() => useSyncState());

    await waitFor(() => {
      expect(tauriApi.loadDashboardSnapshot).toHaveBeenCalledTimes(1);
    });

    const secondRefresh = result.current.refreshState();
    await waitFor(() => {
      expect(tauriApi.loadDashboardSnapshot).toHaveBeenCalledTimes(2);
    });

    resolveFirst!(snapshot("stale"));
    await secondRefresh;

    await waitFor(() => {
      expect(result.current.state?.generated_at).toBe("latest");
    });
  });

  it("clears busy when non-busy refresh supersedes busy refresh", async () => {
    vi.mocked(tauriApi.getRuntimeControls).mockResolvedValue({
      allow_filesystem_changes: true,
      auto_watch_active: false,
    });

    const resolvers: Array<
      (value: DashboardSnapshot | PromiseLike<DashboardSnapshot>) => void
    > = [];
    vi.mocked(tauriApi.loadDashboardSnapshot).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvers.push(resolve);
        }),
    );

    const { result } = renderHook(() => useSyncState());

    await waitFor(() => {
      expect(resolvers.length).toBeGreaterThan(0);
      expect(result.current.busy).toBe(true);
    });

    const callsBeforeNonBusyRefresh = resolvers.length;
    const nonBusyRefresh = result.current.refreshState({ withBusy: false });

    await waitFor(() => {
      expect(resolvers.length).toBeGreaterThan(callsBeforeNonBusyRefresh);
    });
    const latestResolver = resolvers[resolvers.length - 1];
    latestResolver(snapshot("latest"));
    await nonBusyRefresh;

    resolvers
      .slice(0, -1)
      .forEach((resolveStaleSnapshot) =>
        resolveStaleSnapshot(snapshot("stale")),
      );

    await waitFor(() => {
      expect(result.current.state?.generated_at).toBe("latest");
    });

    await waitFor(() => {
      expect(result.current.busy).toBe(false);
    });
  });

  it("retries refresh through the shared dashboard snapshot loader", async () => {
    vi.mocked(tauriApi.getRuntimeControls).mockResolvedValue({
      allow_filesystem_changes: true,
      auto_watch_active: false,
    });
    vi.mocked(tauriApi.loadDashboardSnapshot)
      .mockRejectedValueOnce(new Error("transient"))
      .mockResolvedValueOnce(snapshot("recovered"));

    const { result } = renderHook(() => useSyncState());

    await waitFor(() => {
      expect(tauriApi.loadDashboardSnapshot).toHaveBeenCalledTimes(2);
    });

    await waitFor(() => {
      expect(result.current.state?.generated_at).toBe("recovered");
    });

    expect(result.current.error).toBeNull();
  });

  it("reloads sync-first refresh through the shared dashboard snapshot loader", async () => {
    vi.mocked(tauriApi.getRuntimeControls).mockResolvedValue({
      allow_filesystem_changes: true,
      auto_watch_active: false,
    });
    vi.mocked(tauriApi.loadDashboardSnapshot)
      .mockResolvedValueOnce(snapshot("initial"))
      .mockResolvedValueOnce(snapshot("after-sync"));
    vi.mocked(tauriApi.runSync).mockResolvedValue(snapshot("ignored").state);

    const { result } = renderHook(() => useSyncState());

    await waitFor(() => {
      expect(result.current.state?.generated_at).toBe("initial");
    });

    await result.current.refreshState({ syncFirst: true });

    expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    expect(tauriApi.loadDashboardSnapshot).toHaveBeenCalledTimes(2);
    expect(tauriApi.listSubagents).not.toHaveBeenCalled();
    expect(tauriApi.getAgentsContextReport).not.toHaveBeenCalled();
    expect(tauriApi.getStarredSkillIds).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(result.current.state?.generated_at).toBe("after-sync");
    });
  });
});
