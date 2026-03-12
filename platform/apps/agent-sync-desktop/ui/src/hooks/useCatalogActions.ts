import type { Dispatch, SetStateAction } from "react";
import {
  deleteUnmanagedMcp,
  mutateCatalogItem,
  openSkillPath,
  openSubagentPath,
  renameSkill,
  setAllowFilesystemChanges,
  setMcpServerEnabled,
  setSkillStarred,
} from "../tauriApi";
import type {
  CatalogMutationRequest,
  McpServerRecord,
  RuntimeControls,
  SyncState,
} from "../types";
import type { OpenTargetMenu } from "../lib/uiStateTypes";

type RunAppAction = (
  action: () => Promise<void>,
  options?: {
    clearError?: boolean;
    onError?: (message: string) => void | Promise<void>;
    skipIfBusy?: boolean;
    withBusy?: boolean;
  },
) => Promise<boolean>;

type UseCatalogActionsParams = {
  runAppAction: RunAppAction;
  selectedSkillKey: string | null;
  starredSkillSet: Set<string>;
  setStarredSkillIds: Dispatch<SetStateAction<string[]>>;
  setRuntimeControls: Dispatch<SetStateAction<RuntimeControls | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setOpenTargetMenu: Dispatch<SetStateAction<OpenTargetMenu>>;
  loadRuntimeControls: () => Promise<RuntimeControls | null>;
  refreshState: (options?: {
    preferredSkillKey?: string | null;
    withBusy?: boolean;
  }) => Promise<SyncState | null>;
  applyState: (next: SyncState, preferredSkillKey?: string | null) => void;
  applySubagents: (
    nextSubagents: import("../types").SubagentRecord[],
    preferredSubagentId?: string | null,
  ) => void;
};

type UseCatalogActionsResult = {
  handleToggleSkillStar: (skillId: string) => Promise<void>;
  handleAllowToggle: (allow: boolean) => Promise<void>;
  executeCatalogMutation: (
    request: CatalogMutationRequest,
    preferredSkillKey?: string | null,
  ) => Promise<void>;
  handleRenameSkill: (skillKey: string, rawTitle: string) => Promise<void>;
  handleOpenSkillPath: (
    skillKey: string,
    target: "folder" | "file",
  ) => Promise<void>;
  handleOpenSubagentPath: (
    subagentId: string,
    target: "folder" | "file",
  ) => Promise<void>;
  handleSetMcpEnabled: (
    server: McpServerRecord,
    agent: "codex" | "claude",
    enabled: boolean,
  ) => Promise<void>;
  handleDeleteUnmanagedMcp: (serverKey: string) => Promise<void>;
};

export function useCatalogActions({
  runAppAction,
  selectedSkillKey,
  starredSkillSet,
  setStarredSkillIds,
  setRuntimeControls,
  setError,
  setOpenTargetMenu,
  loadRuntimeControls,
  refreshState,
  applyState,
  applySubagents,
}: UseCatalogActionsParams): UseCatalogActionsResult {
  async function handleToggleSkillStar(skillId: string): Promise<void> {
    const isCurrentlyStarred = starredSkillSet.has(skillId);
    await runAppAction(
      async () => {
        const next = await setSkillStarred(skillId, !isCurrentlyStarred);
        setStarredSkillIds(next);
      },
      { skipIfBusy: true, withBusy: false },
    );
  }

  async function handleAllowToggle(allow: boolean): Promise<void> {
    await runAppAction(
      async () => {
        const next = await setAllowFilesystemChanges(allow);
        setRuntimeControls(next);
        await refreshState({
          preferredSkillKey: selectedSkillKey,
          withBusy: false,
        });
      },
      {
        onError: async (message) => {
          setError(message);
          await loadRuntimeControls();
        },
        skipIfBusy: true,
      },
    );
  }

  async function executeCatalogMutation(
    request: CatalogMutationRequest,
    preferredSkillKey?: string | null,
  ): Promise<void> {
    await runAppAction(
      async () => {
        const next = await mutateCatalogItem(request);
        applySubagents(next.subagents);
        applyState(next, preferredSkillKey ?? selectedSkillKey);
      },
      { skipIfBusy: true },
    );
  }

  async function handleRenameSkill(
    skillKey: string,
    rawTitle: string,
  ): Promise<void> {
    const newTitle = rawTitle.trim();
    if (!newTitle) {
      setError("Rename failed: title cannot be empty.");
      return;
    }

    await runAppAction(
      async () => {
        const result = await renameSkill(skillKey, newTitle);
        applyState(result.state, result.renamed_skill_key);
      },
      { skipIfBusy: true },
    );
  }

  async function handleOpenSkillPath(
    skillKey: string,
    target: "folder" | "file",
  ): Promise<void> {
    setOpenTargetMenu(null);
    await runAppAction(
      async () => {
        await openSkillPath(skillKey, target);
      },
      { skipIfBusy: true },
    );
  }

  async function handleOpenSubagentPath(
    subagentId: string,
    target: "folder" | "file",
  ): Promise<void> {
    setOpenTargetMenu(null);
    await runAppAction(
      async () => {
        await openSubagentPath(subagentId, target);
      },
      { skipIfBusy: true },
    );
  }

  async function handleSetMcpEnabled(
    server: McpServerRecord,
    agent: "codex" | "claude",
    enabled: boolean,
  ): Promise<void> {
    await runAppAction(
      async () => {
        const next = await setMcpServerEnabled(
          server.server_key,
          agent,
          enabled,
          server.scope,
          server.workspace,
        );
        applyState(next, selectedSkillKey);
      },
      { skipIfBusy: true },
    );
  }

  async function handleDeleteUnmanagedMcp(serverKey: string): Promise<void> {
    await runAppAction(
      async () => {
        const next = await deleteUnmanagedMcp(serverKey);
        applyState(next, selectedSkillKey);
      },
      { skipIfBusy: true },
    );
  }

  return {
    handleToggleSkillStar,
    handleAllowToggle,
    executeCatalogMutation,
    handleRenameSkill,
    handleOpenSkillPath,
    handleOpenSubagentPath,
    handleSetMcpEnabled,
    handleDeleteUnmanagedMcp,
  };
}
