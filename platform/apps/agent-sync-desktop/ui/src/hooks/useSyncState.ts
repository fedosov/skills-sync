import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import {
  getAgentsContextReport,
  getRuntimeControls,
  getState,
  listSubagents,
  loadDashboardSnapshot,
  runSync,
} from "../tauriApi";
import { pickPreferred } from "../lib/utils";
import { pickSelectedSkillKey } from "../skillUtils";
import type {
  AgentsContextReport,
  AgentContextEntry,
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
  agentsReport: AgentsContextReport | null;
  selectedSkillKey: string | null;
  selectedSubagentId: string | null;
  selectedMcpKey: string | null;
  selectedAgentEntryId: string | null;
  busy: boolean;
  error: string | null;
  setError: Dispatch<SetStateAction<string | null>>;
  setSelectedSkillKey: Dispatch<SetStateAction<string | null>>;
  setSelectedSubagentId: Dispatch<SetStateAction<string | null>>;
  setSelectedMcpKey: Dispatch<SetStateAction<string | null>>;
  setSelectedAgentEntryId: Dispatch<SetStateAction<string | null>>;
  setRuntimeControls: Dispatch<SetStateAction<RuntimeControls | null>>;
  setBusy: Dispatch<SetStateAction<boolean>>;
  loadRuntimeControls: () => Promise<RuntimeControls | null>;
  refreshState: (options?: RefreshOptions) => Promise<SyncState | null>;
  applyState: (next: SyncState, preferredSkillKey?: string | null) => void;
  applySubagents: (
    nextSubagents: SubagentRecord[],
    preferredSubagentId?: string | null,
  ) => void;
};

export function mcpSelectionKey(server: McpServerRecord): string {
  return `${server.scope}::${server.workspace ?? "global"}::${server.server_key}`;
}

function pickSubagentId(
  subagents: SubagentRecord[],
  preferredSubagentId: string | null | undefined,
  previousSubagentId: string | null,
): string | null {
  return pickPreferred(
    subagents,
    preferredSubagentId,
    previousSubagentId,
    (s) => s.id,
  );
}

function pickAgentEntryId(
  entries: AgentContextEntry[],
  preferredId: string | null | undefined,
  previousId: string | null,
): string | null {
  return pickPreferred(entries, preferredId, previousId, (e) => e.id);
}

export function useSyncState(): UseSyncStateResult {
  const [state, setState] = useState<SyncState | null>(null);
  const [runtimeControls, setRuntimeControls] =
    useState<RuntimeControls | null>(null);
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [agentsReport, setAgentsReport] = useState<AgentsContextReport | null>(
    null,
  );
  const [selectedSkillKey, setSelectedSkillKey] = useState<string | null>(null);
  const [selectedSubagentId, setSelectedSubagentId] = useState<string | null>(
    null,
  );
  const [selectedMcpKey, setSelectedMcpKey] = useState<string | null>(null);
  const [selectedAgentEntryId, setSelectedAgentEntryId] = useState<
    string | null
  >(null);
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

  const applyAgentsReport = useCallback(
    (nextReport: AgentsContextReport | null, preferredId?: string | null) => {
      setAgentsReport(nextReport);
      if (nextReport) {
        setSelectedAgentEntryId((prev) =>
          pickAgentEntryId(nextReport.entries, preferredId, prev),
        );
      }
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
        let nextAgentsReport: AgentsContextReport | null;

        if (syncFirst) {
          nextState = await runSync();
          const [subagentsResult, reportResult] = await Promise.all([
            listSubagents("all"),
            getAgentsContextReport(),
          ]);
          nextSubagents = subagentsResult;
          nextAgentsReport = reportResult;
        } else {
          const snapshot = await loadDashboardSnapshot();
          nextState = snapshot.state;
          nextSubagents = snapshot.subagents;
          nextAgentsReport = snapshot.agentsReport;
        }

        if (requestId !== refreshTokenRef.current) {
          return null;
        }

        applySubagents(nextSubagents, preferredSubagentId);
        applyAgentsReport(nextAgentsReport);
        applyState(nextState, preferredSkillKey);
        return nextState;
      } catch (invokeError) {
        if (requestId !== refreshTokenRef.current) {
          return null;
        }
        setError(String(invokeError));

        try {
          const [fallbackState, fallbackSubagents, fallbackReport] =
            await Promise.allSettled([
              getState(),
              listSubagents("all"),
              getAgentsContextReport(),
            ]);

          if (requestId !== refreshTokenRef.current) {
            return null;
          }

          if (fallbackState.status === "rejected") {
            throw fallbackState.reason;
          }
          if (fallbackSubagents.status === "rejected") {
            throw fallbackSubagents.reason;
          }

          applySubagents(fallbackSubagents.value, preferredSubagentId);
          applyAgentsReport(
            fallbackReport.status === "fulfilled" ? fallbackReport.value : null,
          );
          applyState(fallbackState.value, preferredSkillKey);
          return fallbackState.value;
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
    [applyAgentsReport, applyState, applySubagents],
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
    agentsReport,
    selectedSkillKey,
    selectedSubagentId,
    selectedMcpKey,
    selectedAgentEntryId,
    busy,
    error,
    setError,
    setSelectedSkillKey,
    setSelectedSubagentId,
    setSelectedMcpKey,
    setSelectedAgentEntryId,
    setRuntimeControls,
    setBusy,
    loadRuntimeControls,
    refreshState,
    applyState,
    applySubagents,
  };
}
