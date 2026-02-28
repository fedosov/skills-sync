import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import {
  getRuntimeControls,
  getState,
  listSubagents,
  loadDashboardSnapshot,
  runSync,
} from "../tauriApi";
import { pickSelectedSkillKey } from "../skillUtils";
import type {
  RuntimeControls,
  SubagentRecord,
  SyncState,
  McpServerRecord,
} from "../types";

type RefreshOptions = {
  preferredSkillKey?: string | null;
  preferredSubagentId?: string | null;
  syncFirst?: boolean;
  withBusy?: boolean;
};

type UseSyncStateResult = {
  state: SyncState | null;
  runtimeControls: RuntimeControls | null;
  subagents: SubagentRecord[];
  selectedSkillKey: string | null;
  selectedSubagentId: string | null;
  selectedMcpKey: string | null;
  busy: boolean;
  error: string | null;
  setError: Dispatch<SetStateAction<string | null>>;
  setSelectedSkillKey: Dispatch<SetStateAction<string | null>>;
  setSelectedSubagentId: Dispatch<SetStateAction<string | null>>;
  setSelectedMcpKey: Dispatch<SetStateAction<string | null>>;
  setRuntimeControls: Dispatch<SetStateAction<RuntimeControls | null>>;
  loadRuntimeControls: () => Promise<RuntimeControls | null>;
  refreshState: (options?: RefreshOptions) => Promise<SyncState | null>;
  applyState: (next: SyncState, preferredSkillKey?: string | null) => void;
};

function mcpSelectionKey(server: McpServerRecord): string {
  return `${server.scope}::${server.workspace ?? "global"}::${server.server_key}`;
}

function pickSubagentId(
  subagents: SubagentRecord[],
  preferredSubagentId: string | null | undefined,
  previousSubagentId: string | null,
): string | null {
  if (
    preferredSubagentId &&
    subagents.some((item) => item.id === preferredSubagentId)
  ) {
    return preferredSubagentId;
  }
  if (
    previousSubagentId &&
    subagents.some((item) => item.id === previousSubagentId)
  ) {
    return previousSubagentId;
  }
  return subagents[0]?.id ?? null;
}

export function useSyncState(): UseSyncStateResult {
  const [state, setState] = useState<SyncState | null>(null);
  const [runtimeControls, setRuntimeControls] =
    useState<RuntimeControls | null>(null);
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [selectedSkillKey, setSelectedSkillKey] = useState<string | null>(null);
  const [selectedSubagentId, setSelectedSubagentId] = useState<string | null>(
    null,
  );
  const [selectedMcpKey, setSelectedMcpKey] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refreshTokenRef = useRef(0);
  const busyRequestCountRef = useRef(0);

  const applyState = useCallback(
    (next: SyncState, preferredSkillKey?: string | null) => {
      setState(next);
      setSelectedSkillKey((previousSkillKey) =>
        pickSelectedSkillKey(next.skills, preferredSkillKey, previousSkillKey),
      );

      setSelectedMcpKey((previousKey) => {
        const servers = next.mcp_servers ?? [];
        if (
          previousKey &&
          servers.some((item) => mcpSelectionKey(item) === previousKey)
        ) {
          return previousKey;
        }
        return servers[0] ? mcpSelectionKey(servers[0]) : null;
      });
    },
    [],
  );

  const applySubagents = useCallback(
    (nextSubagents: SubagentRecord[], preferredSubagentId?: string | null) => {
      setSubagents(nextSubagents);
      setSelectedSubagentId((previousSubagentId) =>
        pickSubagentId(nextSubagents, preferredSubagentId, previousSubagentId),
      );
    },
    [],
  );

  const loadRuntimeControls = useCallback(async () => {
    try {
      const next = await getRuntimeControls();
      setRuntimeControls(next);
      return next;
    } catch (invokeError) {
      setError(String(invokeError));
      return null;
    }
  }, []);

  const refreshState = useCallback(
    async ({
      preferredSkillKey,
      preferredSubagentId,
      syncFirst = false,
      withBusy = true,
    }: RefreshOptions = {}) => {
      const requestId = ++refreshTokenRef.current;
      if (withBusy) {
        busyRequestCountRef.current += 1;
        setBusy(true);
      }
      setError(null);

      try {
        let nextState: SyncState;
        let nextSubagents: SubagentRecord[];

        if (syncFirst) {
          nextState = await runSync();
          nextSubagents = await listSubagents("all");
        } else {
          const snapshot = await loadDashboardSnapshot();
          nextState = snapshot.state;
          nextSubagents = snapshot.subagents;
        }

        if (requestId !== refreshTokenRef.current) {
          return null;
        }

        applySubagents(nextSubagents, preferredSubagentId);
        applyState(nextState, preferredSkillKey);
        return nextState;
      } catch (invokeError) {
        if (requestId !== refreshTokenRef.current) {
          return null;
        }
        setError(String(invokeError));

        try {
          const [fallbackState, fallbackSubagents] = await Promise.all([
            getState(),
            listSubagents("all"),
          ]);

          if (requestId !== refreshTokenRef.current) {
            return null;
          }

          applySubagents(fallbackSubagents, preferredSubagentId);
          applyState(fallbackState, preferredSkillKey);
          return fallbackState;
        } catch (fallbackError) {
          if (requestId !== refreshTokenRef.current) {
            return null;
          }
          setError(
            `${String(invokeError)}\nFallback failed: ${String(fallbackError)}`,
          );
          return null;
        }
      } finally {
        if (withBusy) {
          busyRequestCountRef.current = Math.max(
            0,
            busyRequestCountRef.current - 1,
          );
          if (busyRequestCountRef.current === 0) {
            setBusy(false);
          }
        }
      }
    },
    [applyState, applySubagents],
  );

  useEffect(() => {
    void (async () => {
      await loadRuntimeControls();
      await refreshState();
    })();
  }, [loadRuntimeControls, refreshState]);

  return {
    state,
    runtimeControls,
    subagents,
    selectedSkillKey,
    selectedSubagentId,
    selectedMcpKey,
    busy,
    error,
    setError,
    setSelectedSkillKey,
    setSelectedSubagentId,
    setSelectedMcpKey,
    setRuntimeControls,
    loadRuntimeControls,
    refreshState,
    applyState,
  };
}
