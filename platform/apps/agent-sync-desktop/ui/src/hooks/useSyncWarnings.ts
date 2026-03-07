import { useMemo, useState } from "react";
import { fixSyncWarning as fixSyncWarningApi } from "../tauriApi";
import {
  mcpStatus,
  warningMentionsServer,
  mcpSelectionKey,
} from "../lib/catalogUtils";
import type { SyncState } from "../types";

type RunAppAction = (
  action: () => Promise<void>,
  options?: {
    skipIfBusy?: boolean;
  },
) => Promise<boolean>;

type UseSyncWarningsOptions = {
  state: SyncState | null;
  runAppAction: RunAppAction;
  runtimeControls: { allow_filesystem_changes: boolean } | null;
  setError: (error: string) => void;
  refreshState: (options?: {
    preferredSkillKey?: string | null;
    syncFirst?: boolean;
    withBusy?: boolean;
  }) => Promise<unknown>;
  selectedSkillKey: string | null;
  selectedMcpKey: string | null;
};

export function useSyncWarnings({
  state,
  runAppAction,
  runtimeControls,
  setError,
  refreshState,
  selectedSkillKey,
  selectedMcpKey,
}: UseSyncWarningsOptions) {
  const [syncWarningsExpanded, setSyncWarningsExpanded] = useState(false);
  const [fixingSyncWarning, setFixingSyncWarning] = useState<string | null>(
    null,
  );

  const FILESYSTEM_DISABLED_MESSAGE =
    "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.";

  const syncWarnings = useMemo(() => {
    const allWarnings = state?.sync.warnings ?? [];
    const unmanagedKeys = new Set(
      (state?.mcp_servers ?? [])
        .filter((s) => mcpStatus(s) === "unmanaged")
        .map((s) => s.server_key),
    );
    if (unmanagedKeys.size === 0) return allWarnings;
    return allWarnings.filter(
      (w) => ![...unmanagedKeys].some((key) => warningMentionsServer(w, key)),
    );
  }, [state]);

  const selectedMcpServer =
    state?.mcp_servers?.find(
      (item) => mcpSelectionKey(item) === selectedMcpKey,
    ) ?? null;

  const selectedMcpWarnings = useMemo(() => {
    if (!selectedMcpServer) {
      return [];
    }
    const warnings = state?.sync.warnings ?? [];
    const merged = [
      ...selectedMcpServer.warnings,
      ...warnings.filter((warning) =>
        warningMentionsServer(warning, selectedMcpServer.server_key),
      ),
    ];
    return Array.from(new Set(merged));
  }, [selectedMcpServer, state?.sync.warnings]);

  async function handleFixSyncWarning(warning: string) {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }
    if (fixingSyncWarning) {
      return;
    }

    setFixingSyncWarning(warning);
    try {
      await runAppAction(
        async () => {
          await fixSyncWarningApi(warning);
          await refreshState({
            preferredSkillKey: selectedSkillKey,
            syncFirst: false,
            withBusy: false,
          });
        },
        { skipIfBusy: true },
      );
    } finally {
      setFixingSyncWarning(null);
    }
  }

  return {
    syncWarnings,
    syncWarningsExpanded,
    setSyncWarningsExpanded,
    fixingSyncWarning,
    handleFixSyncWarning,
    selectedMcpServer,
    selectedMcpWarnings,
  };
}
