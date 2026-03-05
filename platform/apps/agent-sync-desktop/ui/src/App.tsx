import { useCallback, useEffect, useMemo, useState } from "react";
import { AgentLogoIcon } from "./components/catalog/AgentLogoIcon";
import { SkillListPanel } from "./components/catalog/SkillListPanel";
import { SubagentListPanel } from "./components/catalog/SubagentListPanel";
import { ScopeMarker } from "./components/catalog/ScopeMarker";
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
import { getSubagentDetails, setSkillStarred } from "./tauriApi";
import { useEntityDetails } from "./hooks/useEntityDetails";
import { useFavorites } from "./hooks/useFavorites";
import { useSyncState, mcpSelectionKey } from "./hooks/useSyncState";
import { StarIcon } from "./components/ui/StarIcon";
import {
  toTitleCase,
  subagentStatus,
  mcpStatus,
  statusRank,
  sortAndFilter,
  syncStatusVariant,
  warningMentionsServer,
  syncWarningFixSummary,
  isFixableSyncWarning,
  severityRank,
  severityDotClass,
  readStoredFocusKind,
  mcpTarget,
  mcpDeleteLabel,
  CATALOG_FOCUS_STORAGE_KEY,
} from "./lib/catalogUtils";
import { getVisibleMcpAgents } from "./lib/mcpAgents";
import { compactPath } from "./lib/formatting";
import { cn, errorMessage } from "./lib/utils";
import {
  listDotagentsMcp,
  listDotagentsSkills,
  mutateCatalogItem,
  mutateSkill,
  openSubagentPath,
  openSkillPath,
  renameSkill,
  migrateDotagents,
  fixSyncWarning,
  runDotagentsSync,
  setAllowFilesystemChanges,
  setMcpServerEnabled,
} from "./tauriApi";
import { normalizeSkillKey, sortAndFilterSkills } from "./skillUtils";
import type {
  CatalogMutationRequest,
  FocusKind,
  McpServerRecord,
  MutationCommand,
} from "./types";

type DeleteDialogState = {
  request: CatalogMutationRequest;
  label: string;
} | null;
type OpenTargetMenu = "skill" | "subagent" | null;
type ActionsMenuTarget = "skill" | "subagent" | "mcp" | null;
type DotagentsProofStatus = "idle" | "running" | "ok" | "error";
const DOTAGENTS_MIGRATION_REQUIRED =
  "migration required before strict dotagents sync";
const FILESYSTEM_DISABLED_MESSAGE =
  "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.";

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

  async function handleToggleSkillStar(skillId: string) {
    const isCurrentlyStarred = starredSkillSet.has(skillId);
    try {
      const next = await setSkillStarred(skillId, !isCurrentlyStarred);
      setStarredSkillIds(next);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    }
  }

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

  const handleAllowToggle = useCallback(
    async (allow: boolean) => {
      setBusy(true);
      setError(null);
      try {
        const next = await setAllowFilesystemChanges(allow);
        setRuntimeControls(next);
        await refreshState({ preferredSkillKey: selectedSkillKey });
      } catch (invokeError) {
        setError(errorMessage(invokeError));
        await loadRuntimeControls();
      } finally {
        setBusy(false);
      }
    },
    [
      loadRuntimeControls,
      refreshState,
      selectedSkillKey,
      setBusy,
      setError,
      setRuntimeControls,
    ],
  );

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
    const favSet = favorites.subagents;
    const favRank = (id: string) => (favSet.has(id) ? 0 : 1);
    return sortAndFilter(
      subagents,
      q,
      (lhs, rhs) =>
        statusRank(subagentStatus(lhs)) - statusRank(subagentStatus(rhs)) ||
        favRank(lhs.id) - favRank(rhs.id) ||
        lhs.name.localeCompare(rhs.name) ||
        lhs.scope.localeCompare(rhs.scope) ||
        (lhs.workspace ?? "").localeCompare(rhs.workspace ?? ""),
      (item) => [
        item.name,
        item.subagent_key,
        item.scope,
        item.workspace ?? "",
        item.description,
      ],
    );
  }, [favorites.subagents, focusKind, query, subagents]);

  const filteredMcpServers = useMemo(() => {
    const q = focusKind === "mcp" ? query : "";
    const mcpFavSet = favorites.mcp;
    const mcpFavRank = (server: McpServerRecord) =>
      mcpFavSet.has(mcpSelectionKey(server)) ? 0 : 1;
    return sortAndFilter(
      state?.mcp_servers ?? [],
      q,
      (lhs, rhs) =>
        statusRank(mcpStatus(lhs)) - statusRank(mcpStatus(rhs)) ||
        mcpFavRank(lhs) - mcpFavRank(rhs) ||
        lhs.server_key.localeCompare(rhs.server_key) ||
        lhs.scope.localeCompare(rhs.scope) ||
        (lhs.workspace ?? "").localeCompare(rhs.workspace ?? ""),
      (item) => [
        item.server_key,
        item.scope,
        item.workspace ?? "",
        item.transport,
        item.command ?? "",
        item.url ?? "",
      ],
    );
  }, [favorites.mcp, focusKind, query, state]);

  const filteredAgentEntries = useMemo(() => {
    const q = focusKind === "agents" ? query : "";
    const agentFavSet = favorites.agents;
    const agentFavRank = (id: string) => (agentFavSet.has(id) ? 0 : 1);
    return sortAndFilter(
      agentsReport?.entries ?? [],
      q,
      (lhs, rhs) =>
        severityRank(rhs.severity) - severityRank(lhs.severity) ||
        agentFavRank(lhs.id) - agentFavRank(rhs.id) ||
        lhs.scope.localeCompare(rhs.scope) ||
        (lhs.workspace ?? "").localeCompare(rhs.workspace ?? "") ||
        lhs.root_path.localeCompare(rhs.root_path),
      (entry) => [
        entry.root_path,
        entry.scope,
        entry.workspace ?? "",
        entry.severity,
      ],
    );
  }, [agentsReport, favorites.agents, focusKind, query]);

  const selectedMcpServer =
    state?.mcp_servers?.find(
      (item) => mcpSelectionKey(item) === selectedMcpKey,
    ) ?? null;
  const syncWarnings = state?.sync.warnings ?? [];
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

  async function withErrorGuard(fn: () => Promise<void>): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setBusy(false);
    }
  }

  async function withBusyGuard(fn: () => Promise<void>): Promise<void> {
    if (busy) return;
    await withErrorGuard(fn);
  }

  async function executeSkillMutation(
    command: MutationCommand,
    skillKey: string,
  ) {
    await withBusyGuard(async () => {
      const next = await mutateSkill(command, skillKey);
      applyState(next, skillKey);
    });
  }

  async function executeCatalogMutation(
    request: CatalogMutationRequest,
    preferredSkillKey?: string | null,
  ) {
    await withBusyGuard(async () => {
      const next = await mutateCatalogItem(request);
      applySubagents(next.subagents);
      applyState(next, preferredSkillKey ?? selectedSkillKey);
    });
  }

  async function handleRenameSkill(skillKey: string, rawTitle: string) {
    const newTitle = rawTitle.trim();
    if (!newTitle) {
      setError("Rename failed: title cannot be empty.");
      return;
    }

    const normalizedKey = normalizeSkillKey(newTitle);
    if (!normalizedKey) {
      setError("Rename failed: title must produce non-empty key.");
      return;
    }

    await withBusyGuard(async () => {
      const next = await renameSkill(skillKey, newTitle);
      applyState(next, normalizedKey);
    });
  }

  async function handleOpenSkillPath(
    skillKey: string,
    target: "folder" | "file",
  ) {
    setOpenTargetMenu(null);
    await withErrorGuard(async () => {
      await openSkillPath(skillKey, target);
    });
  }

  async function handleOpenSubagentPath(
    subagentId: string,
    target: "folder" | "file",
  ) {
    setOpenTargetMenu(null);
    await withErrorGuard(async () => {
      await openSubagentPath(subagentId, target);
    });
  }

  async function handleSetMcpEnabled(
    server: McpServerRecord,
    agent: "codex" | "claude",
    enabled: boolean,
  ) {
    await withErrorGuard(async () => {
      const next = await setMcpServerEnabled(
        server.server_key,
        agent,
        enabled,
        server.scope,
        server.workspace,
      );
      applyState(next, selectedSkillKey);
    });
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
    if (busy || fixingSyncWarning) {
      return;
    }

    setFixingSyncWarning(warning);
    setError(null);
    try {
      await fixSyncWarning(warning);
      await refreshState({
        preferredSkillKey: selectedSkillKey,
        syncFirst: true,
      });
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setFixingSyncWarning(null);
    }
  }

  async function verifyDotagents(withBusy: boolean) {
    if (withBusy) {
      setBusy(true);
    }

    setError(null);
    setDotagentsNeedsMigration(false);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Verifying dotagents commands...");

    try {
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
    } catch (invokeError) {
      const message = errorMessage(invokeError);
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
    } finally {
      if (withBusy) {
        setBusy(false);
      }
    }
  }

  async function handleVerifyDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    await verifyDotagents(true);
  }

  async function handleInitializeDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    setBusy(true);
    setError(null);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Initializing dotagents contracts...");

    try {
      await migrateDotagents("all");
      setDotagentsNeedsMigration(false);
      await verifyDotagents(false);
    } catch (invokeError) {
      const message = errorMessage(invokeError);
      setDotagentsProofStatus("error");
      setDotagentsProofSummary(`Dotagents initialization failed: ${message}`);
      setError(message);
    } finally {
      setBusy(false);
    }
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

              <section className="space-y-1.5 border-t border-border/50 pt-3">
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
                    selectedSkillKey={selectedSkillKey}
                    favorites={starredSkillSet}
                    emptyText={activeCatalogEmptyText}
                    onSelect={(skillKey) => {
                      setSelectedSkillKey(skillKey);
                      closeMenus();
                    }}
                    onCloseMenus={closeMenus}
                  />
                ) : null}

                {focusKind === "subagents" ? (
                  <SubagentListPanel
                    subagents={filteredSubagents}
                    selectedSubagentId={selectedSubagentId}
                    favorites={favorites.subagents}
                    emptyText={activeCatalogEmptyText}
                    onSelect={(subagentId) => {
                      setSelectedSubagentId(subagentId);
                      closeMenus();
                    }}
                    onCloseMenus={closeMenus}
                  />
                ) : null}

                {focusKind === "mcp" ? (
                  filteredMcpServers.length === 0 ? (
                    <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
                      {activeCatalogEmptyText}
                    </p>
                  ) : (
                    <ul className="space-y-0.5">
                      {filteredMcpServers.map((server) => {
                        const key = mcpSelectionKey(server);
                        const selected = key === selectedMcpKey;
                        const rowAgents = getVisibleMcpAgents().map(
                          (agent) => ({
                            agent,
                            enabled: server.enabled_by_agent[agent],
                          }),
                        );
                        return (
                          <li key={key}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                                selected
                                  ? "bg-accent/85 text-foreground"
                                  : "hover:bg-accent/55",
                              )}
                              onClick={() => {
                                setSelectedMcpKey(key);
                                closeMenus();
                              }}
                            >
                              <div className="flex items-start justify-between gap-2">
                                <span className="flex min-w-0 items-center gap-1">
                                  {favorites.mcp.has(key) ? (
                                    <StarIcon
                                      filled
                                      className="h-3 w-3 shrink-0 text-amber-400"
                                    />
                                  ) : null}
                                  <span className="truncate text-sm font-medium">
                                    {server.server_key}
                                  </span>
                                </span>
                                <span className="inline-flex items-center gap-1.5">
                                  <ScopeMarker scope={server.scope} />
                                  {mcpStatus(server) === "archived" ? (
                                    <span className="text-[10px] text-muted-foreground">
                                      Archived
                                    </span>
                                  ) : null}
                                </span>
                              </div>
                              <div className="mt-0.5 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
                                <span className="flex min-w-0 items-center gap-1.5">
                                  <span className="shrink-0 font-medium uppercase tracking-wide">
                                    {server.transport.toUpperCase()}
                                  </span>
                                  {server.scope === "project" &&
                                  server.workspace ? (
                                    <span
                                      className="min-w-0 truncate text-[10px]"
                                      title={server.workspace}
                                    >
                                      {server.workspace}
                                    </span>
                                  ) : null}
                                </span>
                                <ul className="flex shrink-0 items-center gap-1.5">
                                  {rowAgents.map(({ agent, enabled }) => (
                                    <li key={agent}>
                                      <span
                                        role="img"
                                        aria-label={`${agent} ${enabled ? "connected" : "disabled"}`}
                                        className={cn(
                                          "inline-flex items-center",
                                          enabled
                                            ? "text-emerald-500"
                                            : "text-muted-foreground/50 opacity-30",
                                        )}
                                      >
                                        <AgentLogoIcon
                                          agent={agent}
                                          className="h-3.5 w-3.5"
                                        />
                                      </span>
                                    </li>
                                  ))}
                                </ul>
                              </div>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  )
                ) : null}

                {focusKind === "agents" ? (
                  filteredAgentEntries.length === 0 ? (
                    <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
                      {activeCatalogEmptyText}
                    </p>
                  ) : (
                    <ul className="space-y-0.5">
                      {filteredAgentEntries.map((entry) => {
                        const selected = entry.id === selectedAgentEntryId;
                        return (
                          <li key={entry.id}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                                selected
                                  ? "bg-accent/85 text-foreground"
                                  : "hover:bg-accent/55",
                              )}
                              onClick={() => {
                                setSelectedAgentEntryId(entry.id);
                                closeMenus();
                              }}
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="flex min-w-0 items-center gap-1.5">
                                  {favorites.agents.has(entry.id) ? (
                                    <StarIcon
                                      filled
                                      className="h-3 w-3 shrink-0 text-amber-400"
                                    />
                                  ) : null}
                                  <span
                                    aria-hidden="true"
                                    className={severityDotClass(entry.severity)}
                                  />
                                  <span className="truncate text-sm font-medium">
                                    {entry.scope === "global"
                                      ? "Global"
                                      : "Project"}
                                  </span>
                                </span>
                                <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
                                  {entry.severity}
                                </span>
                              </div>
                              <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                                {entry.scope === "project" && entry.workspace
                                  ? `${entry.workspace} · ${compactPath(entry.root_path)}`
                                  : compactPath(entry.root_path)}
                              </p>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  )
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
                  setDeleteDialog({
                    request: {
                      action: "delete",
                      target: mcpTarget(selectedMcpServer),
                      confirmed: true,
                    },
                    label: mcpDeleteLabel(selectedMcpServer),
                  });
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
                  const request = deleteDialog.request;
                  setDeleteDialog(null);
                  void executeCatalogMutation(request);
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
