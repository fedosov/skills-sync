import { useEffect, useMemo, useState } from "react";
import { AgentsListPanel } from "./components/catalog/AgentsListPanel";
import { McpListPanel } from "./components/catalog/McpListPanel";
import { SkillListPanel } from "./components/catalog/SkillListPanel";
import { SubagentListPanel } from "./components/catalog/SubagentListPanel";
import { AuditLogDialog } from "./components/AuditLogDialog";
import { AgentsDetailsPanel } from "./components/details/AgentsDetailsPanel";
import { McpDetailsPanel } from "./components/details/McpDetailsPanel";
import { SkillDetailsPanel } from "./components/details/SkillDetailsPanel";
import { SubagentDetailsPanel } from "./components/details/SubagentDetailsPanel";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { useSkillDetails } from "./hooks/useSkillDetails";
import { useEntityDetails } from "./hooks/useEntityDetails";
import { useFavorites } from "./hooks/useFavorites";
import { useSyncState } from "./hooks/useSyncState";
import {
  toTitleCase,
  subagentStatus,
  mcpStatus,
  syncStatusVariant,
  warningMentionsServer,
  syncWarningFixSummary,
  isFixableSyncWarning,
  severityDotClass,
  readStoredFocusKind,
  mcpTarget,
  mcpDeleteLabel,
  mcpSelectionKey,
  sortAndFilterAgentEntries,
  sortAndFilterMcpServers,
  sortAndFilterSubagents,
  CATALOG_FOCUS_STORAGE_KEY,
} from "./lib/catalogUtils";
import { cn, errorMessage } from "./lib/utils";
import {
  deleteUnmanagedMcp,
  fixSyncWarning,
  getSubagentDetails,
  listDotagentsMcp,
  listDotagentsSkills,
  migrateDotagents,
  mutateCatalogItem,
  mutateSkill,
  openSubagentPath,
  openSkillPath,
  renameSkill,
  runDotagentsSync,
  setAllowFilesystemChanges,
  setSkillStarred,
  setMcpServerEnabled,
} from "./tauriApi";
import { sortAndFilterSkills } from "./skillUtils";
import type {
  CatalogMutationRequest,
  FocusKind,
  McpServerRecord,
  MutationCommand,
} from "./types";

type DeleteDialogState = {
  request: CatalogMutationRequest | null;
  label: string;
  onConfirmOverride?: () => Promise<void>;
} | null;
type OpenTargetMenu = "skill" | "subagent" | null;
type ActionsMenuTarget = "skill" | "subagent" | "mcp" | null;
type DotagentsProofStatus = "idle" | "running" | "ok" | "error";
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
const DOTAGENTS_MIGRATION_REQUIRED =
  "migration required before strict dotagents sync";
const FILESYSTEM_DISABLED_MESSAGE =
  "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.";
const EMPTY_PROJECT_GROUP_STATE: CatalogProjectGroupState = {
  skills: {},
  subagents: {},
  mcp: {},
  agents: {},
};

function renderSyncWarningText(warning: string) {
  const term = "central catalog";
  const replacement = "Central Catalog (~/.config/ai-agents/config.toml)";
  const index = warning.indexOf(term);
  if (index === -1) {
    return warning;
  }

  const before = warning.slice(0, index);
  const after = warning.slice(index + term.length);
  return (
    <>
      {before}
      <code className="font-mono text-[11px]">{replacement}</code>
      {after}
    </>
  );
}

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

  const [auditOpen, setAuditOpen] = useState(false);
  const [focusKind, setFocusKind] = useState<FocusKind>(() =>
    readStoredFocusKind(),
  );
  const [query, setQuery] = useState("");
  const [expandedProjectGroups, setExpandedProjectGroups] =
    useState<CatalogProjectGroupState>(EMPTY_PROJECT_GROUP_STATE);
  const [openTargetMenu, setOpenTargetMenu] = useState<OpenTargetMenu>(null);
  const [actionsMenuTarget, setActionsMenuTarget] =
    useState<ActionsMenuTarget>(null);
  const [deleteDialog, setDeleteDialog] = useState<DeleteDialogState>(null);
  const [dotagentsProofStatus, setDotagentsProofStatus] =
    useState<DotagentsProofStatus>("idle");
  const [dotagentsProofSummary, setDotagentsProofSummary] = useState(
    "Dotagents check not run yet.",
  );
  const [dotagentsNeedsMigration, setDotagentsNeedsMigration] = useState(false);
  const [syncWarningsExpanded, setSyncWarningsExpanded] = useState(false);
  const [fixingSyncWarning, setFixingSyncWarning] = useState<string | null>(
    null,
  );

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

  async function handleToggleSkillStar(skillId: string) {
    const isCurrentlyStarred = starredSkillSet.has(skillId);
    await runAppAction(
      async () => {
        const next = await setSkillStarred(skillId, !isCurrentlyStarred);
        setStarredSkillIds(next);
      },
      { skipIfBusy: true, withBusy: false },
    );
  }

  async function handleAllowToggle(allow: boolean) {
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

  useEffect(() => {
    try {
      window.localStorage.setItem(CATALOG_FOCUS_STORAGE_KEY, focusKind);
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, [focusKind]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      setOpenTargetMenu(null);
      setActionsMenuTarget(null);
      setDeleteDialog(null);
      setAuditOpen(false);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

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

  const selectedMcpServer =
    state?.mcp_servers?.find(
      (item) => mcpSelectionKey(item) === selectedMcpKey,
    ) ?? null;
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

  async function executeSkillMutation(
    command: MutationCommand,
    skillKey: string,
  ) {
    await runAppAction(
      async () => {
        const next = await mutateSkill(command, skillKey);
        applyState(next, skillKey);
      },
      { skipIfBusy: true },
    );
  }

  async function executeCatalogMutation(
    request: CatalogMutationRequest,
    preferredSkillKey?: string | null,
  ) {
    await runAppAction(
      async () => {
        const next = await mutateCatalogItem(request);
        applySubagents(next.subagents);
        applyState(next, preferredSkillKey ?? selectedSkillKey);
      },
      { skipIfBusy: true },
    );
  }

  async function handleRenameSkill(skillKey: string, rawTitle: string) {
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
  ) {
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
  ) {
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
  ) {
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

  async function handleDeleteUnmanagedMcp(serverKey: string) {
    await runAppAction(
      async () => {
        const next = await deleteUnmanagedMcp(serverKey);
        applyState(next, selectedSkillKey);
      },
      { skipIfBusy: true },
    );
  }

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
          await fixSyncWarning(warning);
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

  async function verifyDotagentsContracts() {
    await runDotagentsSync("all");
    const [skills, mcp] = await Promise.all([
      listDotagentsSkills("all"),
      listDotagentsMcp("all"),
    ]);
    setDotagentsProofStatus("ok");
    setDotagentsProofSummary(
      `Dotagents verified: skills=${skills.length}, mcp=${mcp.length}.`,
    );
    await refreshState({
      preferredSkillKey: selectedSkillKey,
      withBusy: false,
    });
  }

  function applyDotagentsVerificationError(message: string) {
    const migrationRequired = message
      .toLowerCase()
      .includes(DOTAGENTS_MIGRATION_REQUIRED);
    setDotagentsNeedsMigration(migrationRequired);
    setDotagentsProofStatus("error");
    setDotagentsProofSummary(
      migrationRequired
        ? "Dotagents contracts are missing. Run Initialize dotagents."
        : `Dotagents check failed: ${message}`,
    );
    setError(message);
  }

  async function handleVerifyDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    setDotagentsNeedsMigration(false);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Verifying dotagents commands...");
    await runAppAction(verifyDotagentsContracts, {
      onError: applyDotagentsVerificationError,
      skipIfBusy: true,
    });
  }

  async function handleInitializeDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    setDotagentsNeedsMigration(false);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Initializing dotagents contracts...");
    await runAppAction(
      async () => {
        await migrateDotagents("all");
        setDotagentsProofSummary("Verifying dotagents commands...");
        await verifyDotagentsContracts();
      },
      {
        onError: (message) => {
          setDotagentsProofStatus("error");
          setDotagentsProofSummary(
            `Dotagents initialization failed: ${message}`,
          );
          setError(message);
        },
        skipIfBusy: true,
      },
    );
  }

  function handleCatalogTabChange(next: FocusKind) {
    setFocusKind(next);
    setActionsMenuTarget(null);
    setOpenTargetMenu(null);
  }

  function closeMenus() {
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

  const activeSkillCount =
    state?.skills.filter((skill) => skill.status === "active").length ?? 0;
  const archivedSkillCount =
    state?.skills.filter((skill) => skill.status === "archived").length ?? 0;
  const activeSubagentCount = subagents.filter(
    (subagent) => subagentStatus(subagent) === "active",
  ).length;
  const archivedSubagentCount = subagents.length - activeSubagentCount;
  const activeMcpCount = (state?.mcp_servers ?? []).filter(
    (server) => mcpStatus(server) === "active",
  ).length;
  const archivedMcpCount = (state?.mcp_servers ?? []).filter(
    (server) => mcpStatus(server) === "archived",
  ).length;
  const mcpCount = state?.mcp_servers?.length ?? state?.summary.mcp_count ?? 0;
  const agentContextCount = agentsReport?.entries.length ?? 0;
  const catalogTabCounts = {
    skills: state?.skills.length ?? 0,
    subagents: subagents.length,
    mcp: mcpCount,
    agents: agentContextCount,
  };
  const CATALOG_META: Record<FocusKind, { title: string; emptyText: string }> =
    {
      skills: { title: "Skills", emptyText: "No skills found." },
      subagents: { title: "Subagents", emptyText: "No subagents found." },
      mcp: { title: "MCP", emptyText: "No MCP servers found." },
      agents: { title: "Agents.md", emptyText: "No AGENTS.md entries found." },
    };
  const activeCatalogTitle = CATALOG_META[focusKind].title;
  const activeCatalogEmptyText = CATALOG_META[focusKind].emptyText;
  const catalogFilteredCounts: Record<FocusKind, number> = {
    skills: filteredSkills.length,
    subagents: filteredSubagents.length,
    mcp: filteredMcpServers.length,
    agents: filteredAgentEntries.length,
  };
  const activeCatalogCount = catalogFilteredCounts[focusKind];
  const activeCatalogTotal = catalogTabCounts[focusKind];

  const showSkill = focusKind === "skills" && details;
  const showSubagent = focusKind === "subagents" && subagentDetails;
  const showMcp = focusKind === "mcp" && selectedMcpServer;
  const showAgents = focusKind === "agents" && selectedAgentEntry;
  const dotagentsIndicatorColors: Record<string, string> = {
    ok: "bg-emerald-400",
    running: "bg-amber-400",
    error: "bg-red-400",
  };
  const dotagentsIndicatorClass = cn(
    "inline-block h-2 w-2 rounded-full",
    dotagentsIndicatorColors[dotagentsProofStatus] ?? "bg-muted-foreground/40",
  );
  const agentsTotals = agentsReport?.totals;
  const agentsIndicatorClass = severityDotClass(agentsTotals?.severity ?? "ok");

  return (
    <div className="min-h-full bg-background text-foreground lg:h-screen lg:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1500px] flex-col gap-3 p-3 lg:h-full lg:min-h-0 lg:p-4">
        <header className="shrink-0 border-b border-border/60 px-1 pb-3">
          <div className="flex flex-wrap items-start justify-between gap-2.5">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <h1 className="text-base font-semibold tracking-tight text-dense">
                  Agent Sync
                </h1>
                <Badge variant={syncStatusVariant(state?.sync.status)}>
                  {toTitleCase(state?.sync.status ?? "unknown")}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">
                Active {activeSkillCount} · Archived {archivedSkillCount} ·
                Skills {state?.skills.length ?? 0} · Subagents A{" "}
                {activeSubagentCount}/R {archivedSubagentCount} · MCP A{" "}
                {activeMcpCount}/R {archivedMcpCount} · Agents{" "}
                {agentContextCount}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={busy || !runtimeControls?.allow_filesystem_changes}
                aria-label="Sync"
                onClick={() => void handleSync()}
              >
                Sync
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={busy || !runtimeControls?.allow_filesystem_changes}
                aria-label="Verify dotagents"
                onClick={() => void handleVerifyDotagents()}
              >
                Verify dotagents
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => setAuditOpen(true)}
              >
                Audit log
              </Button>
              <div className="inline-flex items-center gap-2 rounded-md border border-border/70 px-2 py-1">
                <span className="flex flex-col leading-tight">
                  <span className="text-xs text-muted-foreground">Allow</span>
                  <span className="text-[10px] text-muted-foreground/90">
                    access to disk
                  </span>
                </span>
                <button
                  type="button"
                  role="switch"
                  aria-label="Allow filesystem changes"
                  aria-checked={
                    runtimeControls?.allow_filesystem_changes ?? false
                  }
                  disabled={busy}
                  onClick={() =>
                    void handleAllowToggle(
                      !(runtimeControls?.allow_filesystem_changes ?? false),
                    )
                  }
                  className={cn(
                    "relative inline-flex h-6 w-11 items-center rounded-full border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60",
                    runtimeControls?.allow_filesystem_changes
                      ? "border-primary/70 bg-primary/80"
                      : "border-border bg-muted-foreground/25",
                  )}
                >
                  <span
                    aria-hidden="true"
                    className={cn(
                      "inline-block h-4 w-4 transform rounded-full bg-background shadow-sm transition-transform",
                      runtimeControls?.allow_filesystem_changes
                        ? "translate-x-5"
                        : "translate-x-1",
                    )}
                  />
                </button>
              </div>
            </div>
          </div>
          {!runtimeControls?.allow_filesystem_changes ? (
            <p className="mt-2 text-xs text-muted-foreground">
              Read-only mode: filesystem changes are blocked.
            </p>
          ) : null}
          <p
            className="mt-1 text-xs text-muted-foreground"
            data-testid="dotagents-proof"
            aria-live="polite"
            data-status={dotagentsProofStatus}
          >
            <span className="inline-flex items-center gap-1.5">
              <span aria-hidden="true" className={dotagentsIndicatorClass} />
              <span className="font-medium">Dotagents</span>
              <span>{dotagentsProofSummary}</span>
            </span>
          </p>
          <p
            className="mt-1 text-xs text-muted-foreground"
            data-testid="agents-context-indicator"
            aria-live="polite"
          >
            <span className="inline-flex items-center gap-1.5">
              <span aria-hidden="true" className={agentsIndicatorClass} />
              <span className="font-medium">Agents context</span>
              <span>
                {agentsTotals
                  ? `${agentsTotals.tokens_estimate} est · warnings ${agentsReport?.warning_count ?? 0} / critical ${agentsReport?.critical_count ?? 0}`
                  : "loading..."}
              </span>
            </span>
          </p>
          {dotagentsNeedsMigration ? (
            <div className="mt-1.5">
              <Button
                size="sm"
                variant="outline"
                disabled={busy || !runtimeControls?.allow_filesystem_changes}
                aria-label="Initialize dotagents"
                onClick={() => void handleInitializeDotagents()}
              >
                Initialize dotagents
              </Button>
            </div>
          ) : null}
          <div className="mt-2.5">
            <Input
              value={query}
              placeholder="Search by name, key, scope or workspace"
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
          </div>
        </header>

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

        {syncWarnings.length > 0 ? (
          <Card
            className="shrink-0 border-amber-500/40 bg-amber-500/10"
            data-testid="sync-warning-banner"
          >
            <CardContent className="space-y-2 p-2 text-xs text-foreground">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-medium">
                  {`Sync warnings (${syncWarnings.length})`}
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-6 px-2 text-[11px]"
                  onClick={() => setSyncWarningsExpanded((current) => !current)}
                >
                  {syncWarningsExpanded ? "Hide warnings" : "Show warnings"}
                </Button>
              </div>
              {syncWarningsExpanded ? (
                <ul className="space-y-1">
                  {syncWarnings.map((warning) => (
                    <li
                      key={warning}
                      className="rounded-md border border-amber-600/35 bg-amber-500/15 p-2 text-foreground"
                    >
                      <div className="flex items-start justify-between gap-2">
                        <span className="min-w-0 flex-1 break-words">
                          {renderSyncWarningText(warning)}
                        </span>
                        {isFixableSyncWarning(warning) ? (
                          <div className="flex shrink-0 items-center gap-2 pl-2">
                            <span className="max-w-[220px] text-right text-[11px] font-medium leading-tight text-foreground/90">
                              {syncWarningFixSummary(warning)}
                            </span>
                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              className="h-6 shrink-0 border-amber-600/45 bg-card/70 px-2 text-[11px] text-foreground hover:bg-amber-500/20"
                              disabled={
                                busy ||
                                !runtimeControls?.allow_filesystem_changes ||
                                fixingSyncWarning !== null
                              }
                              onClick={() => void handleFixSyncWarning(warning)}
                            >
                              {fixingSyncWarning === warning
                                ? "Fixing..."
                                : "Fix"}
                            </Button>
                          </div>
                        ) : null}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : null}
            </CardContent>
          </Card>
        ) : null}

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
                  void executeSkillMutation(
                    "make_global",
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

      {deleteDialog ? (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4">
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Confirm delete"
            className="w-full max-w-sm rounded-md border border-border/70 bg-card p-4"
          >
            <h2 className="text-sm font-semibold">Confirm delete</h2>
            <p className="mt-2 text-xs text-muted-foreground">
              Remove {deleteDialog.label}? This action moves files to system
              Trash.
            </p>
            <div className="mt-3 flex items-center justify-end gap-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setDeleteDialog(null)}
              >
                Cancel
              </Button>
              <Button
                size="sm"
                variant="destructive"
                disabled={busy}
                onClick={() => {
                  const { request, onConfirmOverride } = deleteDialog;
                  setDeleteDialog(null);
                  if (onConfirmOverride) {
                    void onConfirmOverride();
                  } else if (request) {
                    void executeCatalogMutation(request);
                  }
                }}
              >
                Delete
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
