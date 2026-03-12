import { useEffect, useMemo, useState } from "react";
import { CatalogPane } from "./components/app/CatalogPane";
import { buildDetailsPaneProps } from "./components/app/buildDetailsPaneProps";
import { DetailsPane } from "./components/app/DetailsPane";
import { AuditLogDialog } from "./components/AuditLogDialog";
import { AppHeader } from "./components/AppHeader";
import { SyncWarningsBanner } from "./components/SyncWarningsBanner";
import { DeleteConfirmDialog } from "./components/DeleteConfirmDialog";
import { Card, CardContent } from "./components/ui/card";
import { useCatalogActions } from "./hooks/useCatalogActions";
import { useCatalogCounts } from "./hooks/useCatalogCounts";
import { useSkillDetails } from "./hooks/useSkillDetails";
import { useEntityDetails } from "./hooks/useEntityDetails";
import { useFavorites } from "./hooks/useFavorites";
import { useSyncState } from "./hooks/useSyncState";
import { useDotagentsVerification } from "./hooks/useDotagentsVerification";
import { useSyncWarnings } from "./hooks/useSyncWarnings";
import { useAppMenuState } from "./hooks/useAppMenuState";
import {
  readStoredFocusKind,
  writeStoredFocusKind,
  sortAndFilterAgentEntries,
  sortAndFilterMcpServers,
  sortAndFilterSubagents,
} from "./lib/catalogUtils";
import type { CatalogProjectGroupState } from "./lib/uiStateTypes";
import { errorMessage } from "./lib/utils";
import { getSubagentDetails } from "./tauriApi";
import { sortAndFilterSkills } from "./skillUtils";
import type { FocusKind } from "./types";
type AppActionOptions = {
  clearError?: boolean;
  onError?: (message: string) => void | Promise<void>;
  skipIfBusy?: boolean;
  withBusy?: boolean;
};
const FILESYSTEM_DISABLED_MESSAGE =
  "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.";
const EMPTY_PROJECT_GROUP_STATE: CatalogProjectGroupState = {
  skills: {},
  subagents: {},
  mcp: {},
  agents: {},
};

export function App() {
  const {
    state,
    runtimeControls,
    subagents,
    agentsReport,
    starredSkillIds,
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
    setStarredSkillIds,
    setBusy,
    loadRuntimeControls,
    refreshState,
    applyState,
    applySubagents,
  } = useSyncState();

  const { favorites, toggleFavorite } = useFavorites();

  const starredSkillSet = useMemo(
    () => new Set(starredSkillIds),
    [starredSkillIds],
  );

  const { details, renameDraft, setRenameDraft } = useSkillDetails({
    selectedSkillKey,
    onError: setError,
  });

  const subagentDetails = useEntityDetails(
    selectedSubagentId,
    getSubagentDetails,
    setError,
  );

  const [focusKind, setFocusKind] = useState<FocusKind>(() =>
    readStoredFocusKind(),
  );
  const [query, setQuery] = useState("");
  const [expandedProjectGroups, setExpandedProjectGroups] =
    useState<CatalogProjectGroupState>(EMPTY_PROJECT_GROUP_STATE);

  async function runAppAction(
    action: () => Promise<void>,
    {
      clearError = true,
      onError,
      skipIfBusy = false,
      withBusy = true,
    }: AppActionOptions = {},
  ): Promise<boolean> {
    if (skipIfBusy && busy) {
      return false;
    }
    if (withBusy) {
      setBusy(true);
    }
    if (clearError) {
      setError(null);
    }
    try {
      await action();
      return true;
    } catch (invokeError) {
      const message = errorMessage(invokeError);
      if (onError) {
        await onError(message);
      } else {
        setError(message);
      }
      return false;
    } finally {
      if (withBusy) {
        setBusy(false);
      }
    }
  }

  const {
    openTargetMenu,
    setOpenTargetMenu,
    actionsMenuTarget,
    setActionsMenuTarget,
    deleteDialog,
    setDeleteDialog,
    auditOpen,
    setAuditOpen,
    closeMenus,
  } = useAppMenuState();

  const {
    dotagentsProofStatus,
    dotagentsProofSummary,
    dotagentsNeedsMigration,
    handleVerifyDotagents,
    handleInitializeDotagents,
  } = useDotagentsVerification({
    runAppAction,
    runtimeControls,
    setError,
    refreshState,
    selectedSkillKey,
    busy,
  });

  const {
    syncWarnings,
    syncWarningsExpanded,
    setSyncWarningsExpanded,
    fixingSyncWarning,
    handleFixSyncWarning,
    selectedMcpServer,
    selectedMcpWarnings,
  } = useSyncWarnings({
    state,
    runAppAction,
    runtimeControls,
    setError,
    refreshState,
    selectedSkillKey,
    selectedMcpKey,
  });

  useEffect(() => {
    writeStoredFocusKind(focusKind);
  }, [focusKind]);

  useEffect(() => {
    if (!runtimeControls?.allow_filesystem_changes) {
      return;
    }

    const timer = window.setInterval(() => {
      void refreshState({ withBusy: false });
    }, 3000);

    return () => {
      window.clearInterval(timer);
    };
  }, [refreshState, runtimeControls?.allow_filesystem_changes]);

  const filteredSkills = useMemo(() => {
    if (!state) return [];
    const activeQuery = focusKind === "skills" ? query : "";
    return sortAndFilterSkills(state.skills, activeQuery, starredSkillIds);
  }, [focusKind, query, starredSkillIds, state]);

  const filteredSubagents = useMemo(() => {
    const q = focusKind === "subagents" ? query : "";
    return sortAndFilterSubagents(subagents, q, favorites.subagents);
  }, [favorites.subagents, focusKind, query, subagents]);

  const filteredMcpServers = useMemo(() => {
    const q = focusKind === "mcp" ? query : "";
    return sortAndFilterMcpServers(state?.mcp_servers ?? [], q, favorites.mcp);
  }, [favorites.mcp, focusKind, query, state]);

  const filteredAgentEntries = useMemo(() => {
    const q = focusKind === "agents" ? query : "";
    return sortAndFilterAgentEntries(
      agentsReport?.entries ?? [],
      q,
      favorites.agents,
    );
  }, [agentsReport, favorites.agents, focusKind, query]);

  const selectedAgentEntry =
    agentsReport?.entries.find((item) => item.id === selectedAgentEntryId) ??
    null;
  const selectedAgentTopSegments = useMemo(() => {
    if (!selectedAgentEntry) {
      return [];
    }
    return selectedAgentEntry.segments
      .slice()
      .sort(
        (lhs, rhs) =>
          rhs.tokens_estimate - lhs.tokens_estimate ||
          lhs.path.localeCompare(rhs.path),
      )
      .slice(0, 8);
  }, [selectedAgentEntry]);

  const {
    handleToggleSkillStar,
    handleAllowToggle,
    executeCatalogMutation,
    handleRenameSkill,
    handleOpenSkillPath,
    handleOpenSubagentPath,
    handleSetMcpEnabled,
    handleDeleteUnmanagedMcp,
  } = useCatalogActions({
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
  });

  async function copyPath(path: string, errorLabel: string) {
    try {
      await navigator.clipboard.writeText(path);
    } catch {
      setError(errorLabel);
    }
  }

  async function handleSync() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }
    await refreshState({
      preferredSkillKey: selectedSkillKey,
      syncFirst: true,
    });
  }

  function handleCatalogTabChange(next: FocusKind) {
    setFocusKind(next);
    setActionsMenuTarget(null);
    setOpenTargetMenu(null);
  }

  function toggleProjectGroup(
    kind: FocusKind,
    groupKey: string,
    currentExpanded: boolean,
  ) {
    setExpandedProjectGroups((previous) => ({
      ...previous,
      [kind]: {
        ...previous[kind],
        [groupKey]: !currentExpanded,
      },
    }));
  }

  const {
    activeSkillCount,
    archivedSkillCount,
    activeSubagentCount,
    archivedSubagentCount,
    activeMcpCount,
    archivedMcpCount,
    agentContextCount,
    catalogTabCounts,
    activeCatalogTitle,
    activeCatalogEmptyText,
    activeCatalogCount,
    activeCatalogTotal,
  } = useCatalogCounts({
    state,
    subagents,
    agentsReport,
    focusKind,
    filteredSkills,
    filteredSubagents,
    filteredMcpServers,
    filteredAgentEntries,
  });

  const detailsPaneProps = buildDetailsPaneProps({
    focusKind,
    busy,
    details,
    renameDraft,
    subagentDetails,
    selectedMcpServer,
    selectedMcpWarnings,
    selectedMcpKey,
    selectedAgentEntry,
    selectedAgentTopSegments,
    actionsMenuTarget,
    openTargetMenu,
    fixingSyncWarning,
    starredSkillSet,
    favorites,
    toggleFavorite,
    setOpenTargetMenu,
    setActionsMenuTarget,
    setDeleteDialog,
    setRenameDraft,
    handleToggleSkillStar,
    handleRenameSkill,
    handleOpenSkillPath,
    handleOpenSubagentPath,
    handleSetMcpEnabled,
    handleDeleteUnmanagedMcp,
    handleFixSyncWarning,
    executeCatalogMutation,
    copyPath,
  });

  return (
    <div className="min-h-full bg-background text-foreground lg:h-screen lg:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1500px] flex-col gap-3 p-3 lg:h-full lg:min-h-0 lg:p-4">
        <AppHeader
          state={state}
          runtimeControls={runtimeControls}
          busy={busy}
          query={query}
          setQuery={setQuery}
          activeSkillCount={activeSkillCount}
          archivedSkillCount={archivedSkillCount}
          activeSubagentCount={activeSubagentCount}
          archivedSubagentCount={archivedSubagentCount}
          activeMcpCount={activeMcpCount}
          archivedMcpCount={archivedMcpCount}
          agentContextCount={agentContextCount}
          agentsReport={agentsReport}
          dotagentsProofStatus={dotagentsProofStatus}
          dotagentsProofSummary={dotagentsProofSummary}
          dotagentsNeedsMigration={dotagentsNeedsMigration}
          onSync={() => void handleSync()}
          onVerifyDotagents={() => void handleVerifyDotagents()}
          onInitializeDotagents={() => void handleInitializeDotagents()}
          onAuditOpen={() => setAuditOpen(true)}
          onAllowToggle={(allow) => void handleAllowToggle(allow)}
        />

        {error ? (
          <Card className="shrink-0 border-destructive/35 bg-destructive/10">
            <CardContent className="p-2 text-xs text-destructive">
              {error}
            </CardContent>
          </Card>
        ) : null}

        {state?.sync.error ? (
          <Card className="shrink-0 border-destructive/35 bg-destructive/10">
            <CardContent className="p-2 text-xs text-destructive">
              {state.sync.error}
            </CardContent>
          </Card>
        ) : null}

        <SyncWarningsBanner
          syncWarnings={syncWarnings}
          syncWarningsExpanded={syncWarningsExpanded}
          onToggleExpanded={() =>
            setSyncWarningsExpanded((current) => !current)
          }
          fixingSyncWarning={fixingSyncWarning}
          busy={busy}
          runtimeControls={runtimeControls}
          onFixWarning={(warning) => void handleFixSyncWarning(warning)}
        />

        <main className="grid gap-3 lg:min-h-0 lg:flex-1 lg:grid-cols-[320px_minmax(0,1fr)]">
          <CatalogPane
            focusKind={focusKind}
            catalogTabCounts={catalogTabCounts}
            activeCatalogTitle={activeCatalogTitle}
            activeCatalogCount={activeCatalogCount}
            activeCatalogTotal={activeCatalogTotal}
            activeCatalogEmptyText={activeCatalogEmptyText}
            query={query}
            filteredSkills={filteredSkills}
            filteredSubagents={filteredSubagents}
            filteredMcpServers={filteredMcpServers}
            filteredAgentEntries={filteredAgentEntries}
            selectedSkillKey={selectedSkillKey}
            selectedSubagentId={selectedSubagentId}
            selectedMcpKey={selectedMcpKey}
            selectedAgentEntryId={selectedAgentEntryId}
            starredSkillSet={starredSkillSet}
            subagentFavorites={favorites.subagents}
            mcpFavorites={favorites.mcp}
            agentFavorites={favorites.agents}
            expandedProjectGroups={expandedProjectGroups}
            onTabChange={handleCatalogTabChange}
            onToggleProjectGroup={toggleProjectGroup}
            onSelectSkill={(skillKey) => {
              setSelectedSkillKey(skillKey);
              closeMenus();
            }}
            onSelectSubagent={(subagentId) => {
              setSelectedSubagentId(subagentId);
              closeMenus();
            }}
            onSelectMcp={(selectionKey) => {
              setSelectedMcpKey(selectionKey);
              closeMenus();
            }}
            onSelectAgent={(entryId) => {
              setSelectedAgentEntryId(entryId);
              closeMenus();
            }}
            onCloseMenus={closeMenus}
          />

          <DetailsPane {...detailsPaneProps} />
        </main>
      </div>

      {auditOpen ? (
        <AuditLogDialog
          onClose={() => setAuditOpen(false)}
          onError={setError}
        />
      ) : null}

      <DeleteConfirmDialog
        deleteDialog={deleteDialog}
        busy={busy}
        onClose={() => setDeleteDialog(null)}
        onConfirm={(request) => void executeCatalogMutation(request)}
      />
    </div>
  );
}
