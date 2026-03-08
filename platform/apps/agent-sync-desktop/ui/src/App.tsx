import { useEffect, useMemo, useState } from "react";
import { AgentsListPanel } from "./components/catalog/AgentsListPanel";
import { McpListPanel } from "./components/catalog/McpListPanel";
import { SkillListPanel } from "./components/catalog/SkillListPanel";
import { SubagentListPanel } from "./components/catalog/SubagentListPanel";
import { AuditLogDialog } from "./components/AuditLogDialog";
import { AppHeader } from "./components/AppHeader";
import { SyncWarningsBanner } from "./components/SyncWarningsBanner";
import { DeleteConfirmDialog } from "./components/DeleteConfirmDialog";
import { AgentsDetailsPanel } from "./components/details/AgentsDetailsPanel";
import { McpDetailsPanel } from "./components/details/McpDetailsPanel";
import { SkillDetailsPanel } from "./components/details/SkillDetailsPanel";
import { SubagentDetailsPanel } from "./components/details/SubagentDetailsPanel";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
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
  mcpStatus,
  readStoredFocusKind,
  writeStoredFocusKind,
  mcpTarget,
  mcpDeleteLabel,
  sortAndFilterAgentEntries,
  sortAndFilterMcpServers,
  sortAndFilterSubagents,
} from "./lib/catalogUtils";
import { cn, errorMessage } from "./lib/utils";
import { getSubagentDetails } from "./tauriApi";
import { sortAndFilterSkills } from "./skillUtils";
import type { FocusKind } from "./types";

type CatalogProjectGroupState = Record<
  FocusKind,
  Record<string, boolean | undefined>
>;
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

  const showSkill = focusKind === "skills" && details;
  const showSubagent = focusKind === "subagents" && subagentDetails;
  const showMcp = focusKind === "mcp" && selectedMcpServer;
  const showAgents = focusKind === "agents" && selectedAgentEntry;

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
          <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
            <CardHeader className="pb-2">
              <CardTitle>Catalog</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 p-2 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
              <div className="flex flex-wrap items-center gap-1.5">
                {(
                  [
                    ["skills", "Skills"],
                    ["subagents", "Subagents"],
                    ["mcp", "MCP"],
                    ["agents", "Agents.md"],
                  ] as const
                ).map(([kind, label]) => {
                  const isActive = focusKind === kind;
                  return (
                    <Button
                      key={kind}
                      type="button"
                      size="sm"
                      variant={isActive ? "outline" : "ghost"}
                      aria-label={`Switch catalog to ${label}`}
                      aria-pressed={isActive}
                      className={cn(
                        "h-6 px-2 text-[11px]",
                        isActive ? "bg-accent/70" : "text-muted-foreground",
                      )}
                      onClick={() => handleCatalogTabChange(kind)}
                    >
                      {`${label} (${catalogTabCounts[kind]})`}
                    </Button>
                  );
                })}
              </div>

              <section
                className="space-y-1.5 border-t border-border/50 pt-3"
                data-testid="active-catalog-panel"
              >
                <div className="flex items-center justify-between">
                  <p className="text-xs font-semibold text-muted-foreground">
                    {activeCatalogTitle}
                  </p>
                  <span className="text-[11px] text-muted-foreground">
                    {activeCatalogCount}/{activeCatalogTotal}
                  </span>
                </div>

                {focusKind === "skills" ? (
                  <SkillListPanel
                    skills={filteredSkills}
                    query={query}
                    selectedSkillKey={selectedSkillKey}
                    favorites={starredSkillSet}
                    emptyText={activeCatalogEmptyText}
                    expandedProjectGroups={expandedProjectGroups.skills}
                    onSelect={(skillKey) => {
                      setSelectedSkillKey(skillKey);
                      closeMenus();
                    }}
                    onToggleProjectGroup={(groupKey, currentExpanded) =>
                      toggleProjectGroup("skills", groupKey, currentExpanded)
                    }
                    onCloseMenus={closeMenus}
                  />
                ) : null}

                {focusKind === "subagents" ? (
                  <SubagentListPanel
                    subagents={filteredSubagents}
                    query={query}
                    selectedSubagentId={selectedSubagentId}
                    favorites={favorites.subagents}
                    emptyText={activeCatalogEmptyText}
                    expandedProjectGroups={expandedProjectGroups.subagents}
                    onSelect={(subagentId) => {
                      setSelectedSubagentId(subagentId);
                      closeMenus();
                    }}
                    onToggleProjectGroup={(groupKey, currentExpanded) =>
                      toggleProjectGroup("subagents", groupKey, currentExpanded)
                    }
                    onCloseMenus={closeMenus}
                  />
                ) : null}

                {focusKind === "mcp" ? (
                  <McpListPanel
                    servers={filteredMcpServers}
                    query={query}
                    selectedMcpKey={selectedMcpKey}
                    favorites={favorites.mcp}
                    emptyText={activeCatalogEmptyText}
                    expandedProjectGroups={expandedProjectGroups.mcp}
                    onSelect={(key) => {
                      setSelectedMcpKey(key);
                      closeMenus();
                    }}
                    onToggleProjectGroup={(groupKey, currentExpanded) =>
                      toggleProjectGroup("mcp", groupKey, currentExpanded)
                    }
                    onCloseMenus={closeMenus}
                  />
                ) : null}

                {focusKind === "agents" ? (
                  <AgentsListPanel
                    entries={filteredAgentEntries}
                    query={query}
                    selectedAgentEntryId={selectedAgentEntryId}
                    favorites={favorites.agents}
                    emptyText={activeCatalogEmptyText}
                    expandedProjectGroups={expandedProjectGroups.agents}
                    onSelect={(entryId) => {
                      setSelectedAgentEntryId(entryId);
                      closeMenus();
                    }}
                    onToggleProjectGroup={(groupKey, currentExpanded) =>
                      toggleProjectGroup("agents", groupKey, currentExpanded)
                    }
                    onCloseMenus={closeMenus}
                  />
                ) : null}
              </section>
            </CardContent>
          </Card>

          <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
            {!showSkill && !showSubagent && !showMcp && !showAgents ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select an item to view details.
              </CardContent>
            ) : null}

            {showSkill ? (
              <SkillDetailsPanel
                details={details}
                busy={busy}
                isFavorite={starredSkillSet.has(details.skill.id)}
                onToggleFavorite={() =>
                  void handleToggleSkillStar(details.skill.id)
                }
                renameDraft={renameDraft}
                openTargetMenu={openTargetMenu === "skill"}
                actionsMenuOpen={actionsMenuTarget === "skill"}
                onRenameDraftChange={setRenameDraft}
                onRenameSubmit={() =>
                  void handleRenameSkill(details.skill.skill_key, renameDraft)
                }
                onToggleOpenTargetMenu={() => {
                  setOpenTargetMenu((prev) =>
                    prev === "skill" ? null : "skill",
                  );
                  setActionsMenuTarget(null);
                }}
                onToggleActionsMenu={() => {
                  setActionsMenuTarget((prev) =>
                    prev === "skill" ? null : "skill",
                  );
                  setOpenTargetMenu(null);
                }}
                onOpenPath={(target) =>
                  void handleOpenSkillPath(details.skill.skill_key, target)
                }
                onArchive={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation(
                    {
                      action: "archive",
                      target: {
                        kind: "skill",
                        skillKey: details.skill.skill_key,
                      },
                      confirmed: true,
                    },
                    details.skill.skill_key,
                  );
                }}
                onMakeGlobal={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation(
                    {
                      action: "make_global",
                      target: {
                        kind: "skill",
                        skillKey: details.skill.skill_key,
                      },
                      confirmed: true,
                    },
                    details.skill.skill_key,
                  );
                }}
                onRestore={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation(
                    {
                      action: "restore",
                      target: {
                        kind: "skill",
                        skillKey: details.skill.skill_key,
                      },
                      confirmed: true,
                    },
                    details.skill.skill_key,
                  );
                }}
                onRequestDelete={() => {
                  setActionsMenuTarget(null);
                  setDeleteDialog({
                    request: {
                      action: "delete",
                      target: {
                        kind: "skill",
                        skillKey: details.skill.skill_key,
                      },
                      confirmed: true,
                    },
                    label: `skill "${details.skill.name}"`,
                  });
                }}
                onCopyPath={(path, errorLabel) =>
                  void copyPath(path, errorLabel)
                }
              />
            ) : null}

            {showMcp ? (
              <McpDetailsPanel
                server={selectedMcpServer}
                warnings={selectedMcpWarnings}
                busy={busy}
                fixingWarning={fixingSyncWarning}
                isFavorite={favorites.mcp.has(selectedMcpKey!)}
                onToggleFavorite={() => toggleFavorite("mcp", selectedMcpKey!)}
                actionsMenuOpen={actionsMenuTarget === "mcp"}
                onToggleActionsMenu={() => {
                  setActionsMenuTarget((prev) =>
                    prev === "mcp" ? null : "mcp",
                  );
                  setOpenTargetMenu(null);
                }}
                onSetEnabled={(agent, enabled) =>
                  void handleSetMcpEnabled(selectedMcpServer, agent, enabled)
                }
                onFixWarning={(warning) => void handleFixSyncWarning(warning)}
                onArchive={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation({
                    action: "archive",
                    target: mcpTarget(selectedMcpServer),
                    confirmed: true,
                  });
                }}
                onMakeGlobal={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation({
                    action: "make_global",
                    target: mcpTarget(selectedMcpServer),
                    confirmed: true,
                  });
                }}
                onRestore={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation({
                    action: "restore",
                    target: mcpTarget(selectedMcpServer),
                    confirmed: true,
                  });
                }}
                onRequestDelete={() => {
                  setActionsMenuTarget(null);
                  if (mcpStatus(selectedMcpServer) === "unmanaged") {
                    const serverKey = selectedMcpServer.server_key;
                    setDeleteDialog({
                      request: null,
                      label: mcpDeleteLabel(selectedMcpServer),
                      onConfirmOverride: async () =>
                        handleDeleteUnmanagedMcp(serverKey),
                    });
                  } else {
                    setDeleteDialog({
                      request: {
                        action: "delete",
                        target: mcpTarget(selectedMcpServer),
                        confirmed: true,
                      },
                      label: mcpDeleteLabel(selectedMcpServer),
                    });
                  }
                }}
              />
            ) : null}

            {showAgents ? (
              <AgentsDetailsPanel
                entry={selectedAgentEntry}
                topSegments={selectedAgentTopSegments}
                isFavorite={favorites.agents.has(selectedAgentEntry.id)}
                onToggleFavorite={() =>
                  toggleFavorite("agents", selectedAgentEntry.id)
                }
              />
            ) : null}

            {showSubagent ? (
              <SubagentDetailsPanel
                subagentDetails={subagentDetails}
                busy={busy}
                isFavorite={favorites.subagents.has(
                  subagentDetails.subagent.id,
                )}
                onToggleFavorite={() =>
                  toggleFavorite("subagents", subagentDetails.subagent.id)
                }
                openTargetMenu={openTargetMenu === "subagent"}
                actionsMenuOpen={actionsMenuTarget === "subagent"}
                onToggleOpenTargetMenu={() => {
                  setOpenTargetMenu((prev) =>
                    prev === "subagent" ? null : "subagent",
                  );
                  setActionsMenuTarget(null);
                }}
                onToggleActionsMenu={() => {
                  setActionsMenuTarget((prev) =>
                    prev === "subagent" ? null : "subagent",
                  );
                  setOpenTargetMenu(null);
                }}
                onOpenPath={(target) =>
                  void handleOpenSubagentPath(
                    subagentDetails.subagent.id,
                    target,
                  )
                }
                onArchive={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation({
                    action: "archive",
                    target: {
                      kind: "subagent",
                      subagentId: subagentDetails.subagent.id,
                    },
                    confirmed: true,
                  });
                }}
                onRestore={() => {
                  setActionsMenuTarget(null);
                  void executeCatalogMutation({
                    action: "restore",
                    target: {
                      kind: "subagent",
                      subagentId: subagentDetails.subagent.id,
                    },
                    confirmed: true,
                  });
                }}
                onRequestDelete={() => {
                  setActionsMenuTarget(null);
                  setDeleteDialog({
                    request: {
                      action: "delete",
                      target: {
                        kind: "subagent",
                        subagentId: subagentDetails.subagent.id,
                      },
                      confirmed: true,
                    },
                    label: `subagent "${subagentDetails.subagent.name}"`,
                  });
                }}
              />
            ) : null}
          </Card>
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
