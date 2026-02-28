import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useSyncState } from "./useSyncState";
import * as tauriApi from "../tauriApi";
import type { DashboardSnapshot } from "../types";

vi.mock("../tauriApi", () => ({
  getRuntimeControls: vi.fn(),
  loadDashboardSnapshot: vi.fn(),
  runSync: vi.fn(),
  getState: vi.fn(),
  listSubagents: vi.fn(),
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
  };
}

describe("useSyncState", () => {
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

    resolveFirst?.(snapshot("stale"));
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
});
