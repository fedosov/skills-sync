import { useCallback, useEffect, useMemo, useState } from "react";
import { McpAgentStatusStrip } from "./components/catalog/McpAgentStatusStrip";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { getVisibleMcpAgents } from "./lib/mcpAgents";
import { cn } from "./lib/utils";
import {
  getRuntimeControls,
  getSkillDetails,
  getState,
  getSubagentDetails,
  listAuditEvents,
  listSubagents,
  mutateSkill,
  openSubagentPath,
  openSkillPath,
  renameSkill,
  runSync,
  setAllowFilesystemChanges,
  setMcpServerEnabled,
} from "./tauriApi";
import {
  formatUnixTime,
  normalizeSkillKey,
  pickSelectedSkillKey,
  sortAndFilterSkills,
} from "./skillUtils";
import type {
  AuditEvent,
  AuditEventStatus,
  McpServerRecord,
  MutationCommand,
  RuntimeControls,
  SubagentDetails,
  SubagentRecord,
  SkillDetails,
  SyncHealthStatus,
  SyncState,
} from "./types";

type FocusKind = "skills" | "subagents" | "mcp";
type DeleteDialogState = { skillKey: string; confirmText: string } | null;
type OpenTargetMenu = "skill" | "subagent" | null;
type AuditStatusFilter = AuditEventStatus | "all";
const CATALOG_FOCUS_STORAGE_KEY = "skillssync.catalog.focusKind.v1";

function toTitleCase(value: string): string {
  if (!value) {
    return value;
  }
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}

function mcpSelectionKey(server: McpServerRecord): string {
  return `${server.scope}::${server.workspace ?? "global"}::${server.server_key}`;
}

function syncStatusVariant(status: SyncHealthStatus | undefined) {
  switch (status) {
    case "ok":
      return "success" as const;
    case "failed":
      return "error" as const;
    case "syncing":
      return "warning" as const;
    default:
      return "outline" as const;
  }
}

function compactPath(path: string | null | undefined): string {
  if (!path) {
    return "-";
  }
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= 3) {
    return path;
  }
  return `/${segments[0]}/.../${segments[segments.length - 1]}`;
}

function formatIsoTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function parseAuditStatusFilter(value: string): AuditStatusFilter {
  switch (value) {
    case "success":
    case "failed":
    case "blocked":
    case "all":
      return value;
    default:
      return "all";
  }
}

function parseFocusKind(value: string | null): FocusKind {
  if (value === "skills" || value === "subagents" || value === "mcp") {
    return value;
  }
  return "skills";
}

function readStoredFocusKind(): FocusKind {
  if (typeof window === "undefined") {
    return "skills";
  }
  try {
    return parseFocusKind(
      window.localStorage.getItem(CATALOG_FOCUS_STORAGE_KEY),
    );
  } catch {
    return "skills";
  }
}

function ScopeMarker({ scope }: { scope: string }) {
  const scopeLabel = scope === "global" ? "Global" : "Project";
  return (
    <span className="inline-flex items-center gap-1">
      <span
        aria-hidden="true"
        title={scopeLabel}
        className={cn(
          "inline-block h-2 w-2 rounded-full",
          scope === "global" ? "bg-emerald-500/80" : "bg-sky-500/80",
        )}
      />
      <span className="text-[10px] text-muted-foreground">{scopeLabel}</span>
    </span>
  );
}

export function App() {
  const [state, setState] = useState<SyncState | null>(null);
  const [runtimeControls, setRuntimeControls] =
    useState<RuntimeControls | null>(null);
  const [details, setDetails] = useState<SkillDetails | null>(null);
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [subagentDetails, setSubagentDetails] =
    useState<SubagentDetails | null>(null);
  const [auditOpen, setAuditOpen] = useState(false);
  const [auditEvents, setAuditEvents] = useState<AuditEvent[]>([]);
  const [auditStatusFilter, setAuditStatusFilter] =
    useState<AuditStatusFilter>("all");
  const [auditActionFilter, setAuditActionFilter] = useState("");
  const [auditBusy, setAuditBusy] = useState(false);
  const [focusKind, setFocusKind] = useState<FocusKind>(() =>
    readStoredFocusKind(),
  );
  const [selectedSkillKey, setSelectedSkillKey] = useState<string | null>(null);
  const [selectedSubagentId, setSelectedSubagentId] = useState<string | null>(
    null,
  );
  const [selectedMcpKey, setSelectedMcpKey] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [renameDraft, setRenameDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [openTargetMenu, setOpenTargetMenu] = useState<OpenTargetMenu>(null);
  const [actionsMenuOpen, setActionsMenuOpen] = useState(false);
  const [deleteDialog, setDeleteDialog] = useState<DeleteDialogState>(null);

  const applyState = useCallback(
    (next: SyncState, preferredKey?: string | null) => {
      setState(next);
      setSelectedSkillKey((previousKey) =>
        pickSelectedSkillKey(next.skills, preferredKey, previousKey),
      );
    },
    [],
  );

  const applySubagents = useCallback(async (preferredKey?: string | null) => {
    const nextSubagents = await listSubagents("all");
    setSubagents(nextSubagents);
    setSelectedSubagentId((prev) => {
      if (
        preferredKey &&
        nextSubagents.some((item) => item.id === preferredKey)
      ) {
        return preferredKey;
      }
      if (prev && nextSubagents.some((item) => item.id === prev)) {
        return prev;
      }
      return nextSubagents[0]?.id ?? null;
    });
  }, []);

  const refreshState = useCallback(
    async (
      preferredKey?: string | null,
      syncFirst = false,
      withBusy = true,
    ) => {
      if (withBusy) {
        setBusy(true);
      }
      setError(null);
      try {
        const next = syncFirst ? await runSync() : await getState();
        await applySubagents(preferredKey);
        applyState(next, preferredKey);
      } catch (invokeError) {
        setError(String(invokeError));
        try {
          const fallbackState = await getState();
          await applySubagents(preferredKey);
          applyState(fallbackState, preferredKey);
        } catch (fallbackError) {
          setError(
            `${String(invokeError)}\nFallback failed: ${String(fallbackError)}`,
          );
        }
      } finally {
        if (withBusy) {
          setBusy(false);
        }
      }
    },
    [applyState, applySubagents],
  );

  const loadAudit = useCallback(async () => {
    setAuditBusy(true);
    try {
      const next = await listAuditEvents({
        limit: 200,
        status: auditStatusFilter === "all" ? undefined : auditStatusFilter,
        action: auditActionFilter,
      });
      setAuditEvents(next);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setAuditBusy(false);
    }
  }, [auditActionFilter, auditStatusFilter]);

  const loadRuntime = useCallback(async () => {
    try {
      const next = await getRuntimeControls();
      setRuntimeControls(next);
    } catch (invokeError) {
      setError(String(invokeError));
    }
  }, []);

  const handleAllowToggle = useCallback(
    async (allow: boolean) => {
      setBusy(true);
      setError(null);
      try {
        const next = await setAllowFilesystemChanges(allow);
        setRuntimeControls(next);
        await refreshState(selectedSkillKey, false);
      } catch (invokeError) {
        setError(String(invokeError));
        await loadRuntime();
      } finally {
        setBusy(false);
      }
    },
    [loadRuntime, refreshState, selectedSkillKey],
  );

  useEffect(() => {
    void (async () => {
      await loadRuntime();
      await refreshState(undefined, false);
    })();
  }, [loadRuntime, refreshState]);

  useEffect(() => {
    if (!state || state.skills.length === 0) {
      setSelectedSkillKey(null);
      setDetails(null);
      return;
    }
    setSelectedSkillKey((current) =>
      pickSelectedSkillKey(state.skills, current),
    );
  }, [state]);

  useEffect(() => {
    try {
      window.localStorage.setItem(CATALOG_FOCUS_STORAGE_KEY, focusKind);
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, [focusKind]);

  useEffect(() => {
    if (!selectedSkillKey) {
      setDetails(null);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const next = await getSkillDetails(selectedSkillKey);
        if (!cancelled) {
          setDetails(next);
          setRenameDraft(next.skill.name);
        }
      } catch (invokeError) {
        if (!cancelled) {
          setError(String(invokeError));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [selectedSkillKey]);

  useEffect(() => {
    if (!selectedSubagentId) {
      setSubagentDetails(null);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const next = await getSubagentDetails(selectedSubagentId);
        if (!cancelled) {
          setSubagentDetails(next);
        }
      } catch (invokeError) {
        if (!cancelled) {
          setError(String(invokeError));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [selectedSubagentId]);

  useEffect(() => {
    const servers = state?.mcp_servers ?? [];
    setSelectedMcpKey((current) => {
      if (
        current &&
        servers.some((item) => mcpSelectionKey(item) === current)
      ) {
        return current;
      }
      return servers[0] ? mcpSelectionKey(servers[0]) : null;
    });
  }, [state]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      setOpenTargetMenu(null);
      setActionsMenuOpen(false);
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
      void refreshState(undefined, false, false);
      if (auditOpen) {
        void loadAudit();
      }
    }, 3000);

    return () => {
      window.clearInterval(timer);
    };
  }, [
    auditOpen,
    loadAudit,
    refreshState,
    runtimeControls?.allow_filesystem_changes,
  ]);

  const filteredSkills = useMemo(() => {
    if (!state) return [];
    const activeQuery = focusKind === "skills" ? query : "";
    return sortAndFilterSkills(state.skills, activeQuery, []);
  }, [focusKind, query, state]);

  const filteredSubagents = useMemo(() => {
    const normalizedQuery =
      focusKind === "subagents" ? query.trim().toLowerCase() : "";
    const ordered = subagents
      .slice()
      .sort(
        (lhs, rhs) =>
          lhs.name.localeCompare(rhs.name) ||
          lhs.scope.localeCompare(rhs.scope),
      );
    if (!normalizedQuery) {
      return ordered;
    }
    return ordered.filter((item) => {
      return (
        item.name.toLowerCase().includes(normalizedQuery) ||
        item.subagent_key.toLowerCase().includes(normalizedQuery) ||
        item.scope.toLowerCase().includes(normalizedQuery) ||
        (item.workspace ?? "").toLowerCase().includes(normalizedQuery) ||
        item.description.toLowerCase().includes(normalizedQuery)
      );
    });
  }, [focusKind, query, subagents]);

  const filteredMcpServers = useMemo(() => {
    const normalizedQuery =
      focusKind === "mcp" ? query.trim().toLowerCase() : "";
    const servers = (state?.mcp_servers ?? []).slice().sort((lhs, rhs) => {
      return (
        lhs.server_key.localeCompare(rhs.server_key) ||
        lhs.scope.localeCompare(rhs.scope) ||
        (lhs.workspace ?? "").localeCompare(rhs.workspace ?? "")
      );
    });
    if (!normalizedQuery) {
      return servers;
    }
    return servers.filter((item) => {
      return (
        item.server_key.toLowerCase().includes(normalizedQuery) ||
        item.scope.toLowerCase().includes(normalizedQuery) ||
        (item.workspace ?? "").toLowerCase().includes(normalizedQuery) ||
        item.transport.toLowerCase().includes(normalizedQuery) ||
        (item.command ?? "").toLowerCase().includes(normalizedQuery) ||
        (item.url ?? "").toLowerCase().includes(normalizedQuery)
      );
    });
  }, [focusKind, query, state]);

  const selectedMcpServer =
    state?.mcp_servers?.find(
      (item) => mcpSelectionKey(item) === selectedMcpKey,
    ) ?? null;

  async function executeMutation(command: MutationCommand, skillKey: string) {
    if (busy) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await mutateSkill(command, skillKey);
      applyState(next, skillKey);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
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

    setBusy(true);
    setError(null);
    try {
      const next = await renameSkill(skillKey, newTitle);
      applyState(next, normalizedKey);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenSkillPath(
    skillKey: string,
    target: "folder" | "file",
  ) {
    setBusy(true);
    setError(null);
    setOpenTargetMenu(null);
    try {
      await openSkillPath(skillKey, target);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenSubagentPath(
    subagentId: string,
    target: "folder" | "file",
  ) {
    setBusy(true);
    setError(null);
    setOpenTargetMenu(null);
    try {
      await openSubagentPath(subagentId, target);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
  }

  async function handleSetMcpEnabled(
    server: McpServerRecord,
    agent: "codex" | "claude" | "project",
    enabled: boolean,
  ) {
    setBusy(true);
    setError(null);
    try {
      const next = await setMcpServerEnabled(
        server.server_key,
        agent,
        enabled,
        server.scope,
        server.workspace,
      );
      applyState(next, selectedSkillKey);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
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
      setError(
        "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.",
      );
      return;
    }
    await refreshState(selectedSkillKey, true);
  }

  async function handleOpenAuditLog() {
    setAuditOpen(true);
    await loadAudit();
  }

  function handleCatalogTabChange(next: FocusKind) {
    setFocusKind(next);
    setActionsMenuOpen(false);
    setOpenTargetMenu(null);
  }

  const activeSkillCount =
    state?.skills.filter((skill) => skill.status === "active").length ?? 0;
  const archivedSkillCount =
    state?.skills.filter((skill) => skill.status === "archived").length ?? 0;
  const activeSubagentCount = subagents.length;
  const mcpCount = state?.summary.mcp_count ?? state?.mcp_servers?.length ?? 0;
  const catalogTabCounts = {
    skills: state?.skills.length ?? 0,
    subagents: subagents.length,
    mcp: mcpCount,
  };
  const activeCatalogTitle =
    focusKind === "skills"
      ? "Skills"
      : focusKind === "subagents"
        ? "Subagents"
        : "MCP";
  const activeCatalogCount =
    focusKind === "skills"
      ? filteredSkills.length
      : focusKind === "subagents"
        ? filteredSubagents.length
        : filteredMcpServers.length;
  const activeCatalogTotal = catalogTabCounts[focusKind];
  const activeCatalogEmptyText =
    focusKind === "skills"
      ? "No skills found."
      : focusKind === "subagents"
        ? "No subagents found."
        : "No MCP servers found.";

  const showSkill = focusKind === "skills" && details;
  const showSubagent = focusKind === "subagents" && subagentDetails;
  const showMcp = focusKind === "mcp" && selectedMcpServer;

  return (
    <div className="min-h-full bg-background text-foreground lg:h-screen lg:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1500px] flex-col gap-3 p-3 lg:h-full lg:min-h-0 lg:p-4">
        <header className="shrink-0 border-b border-border/60 px-1 pb-3">
          <div className="flex flex-wrap items-start justify-between gap-2.5">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <h1 className="text-base font-semibold tracking-tight text-dense">
                  SkillsSync
                </h1>
                <Badge variant={syncStatusVariant(state?.sync.status)}>
                  {toTitleCase(state?.sync.status ?? "unknown")}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">
                Active {activeSkillCount} · Archived {archivedSkillCount} ·
                Skills {state?.skills.length ?? 0} · Subagents{" "}
                {activeSubagentCount} · MCP {mcpCount}
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
                variant="ghost"
                disabled={busy}
                onClick={() => void handleOpenAuditLog()}
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
                  filteredSkills.length === 0 ? (
                    <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
                      {activeCatalogEmptyText}
                    </p>
                  ) : (
                    <ul className="space-y-0.5">
                      {filteredSkills.map((skill) => {
                        const selected = skill.skill_key === selectedSkillKey;
                        return (
                          <li key={skill.id}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                                selected
                                  ? "bg-accent/85 text-foreground"
                                  : "hover:bg-accent/55",
                              )}
                              onClick={() => {
                                setSelectedSkillKey(skill.skill_key);
                                setActionsMenuOpen(false);
                                setOpenTargetMenu(null);
                              }}
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="truncate text-sm font-medium">
                                  {skill.name}
                                </span>
                                <ScopeMarker scope={skill.scope} />
                              </div>
                              <p
                                aria-hidden="true"
                                className="mt-0.5 truncate text-[11px] text-muted-foreground"
                              >
                                {skill.skill_key}
                              </p>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  )
                ) : null}

                {focusKind === "subagents" ? (
                  filteredSubagents.length === 0 ? (
                    <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
                      {activeCatalogEmptyText}
                    </p>
                  ) : (
                    <ul className="space-y-0.5">
                      {filteredSubagents.map((subagent) => {
                        const selected = subagent.id === selectedSubagentId;
                        return (
                          <li key={subagent.id}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                                selected
                                  ? "bg-accent/85 text-foreground"
                                  : "hover:bg-accent/55",
                              )}
                              onClick={() => {
                                setSelectedSubagentId(subagent.id);
                                setActionsMenuOpen(false);
                                setOpenTargetMenu(null);
                              }}
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="truncate text-sm font-medium">
                                  {subagent.name}
                                </span>
                                <ScopeMarker scope={subagent.scope} />
                              </div>
                              <p
                                aria-hidden="true"
                                className="mt-0.5 truncate text-[11px] text-muted-foreground"
                              >
                                {subagent.subagent_key}
                              </p>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  )
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
                                setActionsMenuOpen(false);
                                setOpenTargetMenu(null);
                              }}
                            >
                              <div className="flex items-start justify-between gap-2">
                                <span className="truncate text-sm font-medium">
                                  {server.server_key}
                                </span>
                                <div className="flex shrink-0 flex-col items-end gap-0.5">
                                  <ScopeMarker scope={server.scope} />
                                  <McpAgentStatusStrip
                                    scope={server.scope}
                                    enabledByAgent={server.enabled_by_agent}
                                  />
                                </div>
                              </div>
                              <p
                                aria-hidden="true"
                                className="mt-0.5 truncate text-[11px] text-muted-foreground"
                              >
                                {server.workspace ??
                                  server.transport.toUpperCase()}
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
            {!showSkill && !showSubagent && !showMcp ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select an item to view details.
              </CardContent>
            ) : null}

            {showSkill ? (
              <>
                <CardHeader className="border-b border-border/60 pb-3">
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div>
                      <CardTitle className="text-lg leading-tight">
                        {details.skill.name}
                      </CardTitle>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {details.skill.skill_key}
                      </p>
                    </div>
                    <div className="relative flex items-center gap-1.5">
                      <Button
                        size="sm"
                        variant="outline"
                        aria-expanded={openTargetMenu === "skill"}
                        onClick={() => {
                          setOpenTargetMenu((prev) =>
                            prev === "skill" ? null : "skill",
                          );
                          setActionsMenuOpen(false);
                        }}
                      >
                        Open…
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        aria-label="More actions"
                        disabled={busy}
                        aria-expanded={actionsMenuOpen}
                        onClick={() => {
                          setActionsMenuOpen((prev) => !prev);
                          setOpenTargetMenu(null);
                        }}
                      >
                        ⋯
                      </Button>

                      {openTargetMenu === "skill" ? (
                        <div
                          role="menu"
                          className="absolute right-14 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
                        >
                          <button
                            type="button"
                            role="menuitem"
                            className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent"
                            onClick={() =>
                              void handleOpenSkillPath(
                                details.skill.skill_key,
                                "folder",
                              )
                            }
                          >
                            Open folder
                          </button>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={!details.main_file_exists}
                            className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:opacity-50"
                            onClick={() =>
                              void handleOpenSkillPath(
                                details.skill.skill_key,
                                "file",
                              )
                            }
                          >
                            Open file
                          </button>
                        </div>
                      ) : null}

                      {actionsMenuOpen ? (
                        <div
                          role="menu"
                          className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
                        >
                          {details.skill.status === "active" ? (
                            <>
                              <button
                                type="button"
                                role="menuitem"
                                disabled={busy}
                                className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                                onClick={() => {
                                  setActionsMenuOpen(false);
                                  void executeMutation(
                                    "archive_skill",
                                    details.skill.skill_key,
                                  );
                                }}
                              >
                                Archive
                              </button>
                              {details.skill.scope === "project" ? (
                                <button
                                  type="button"
                                  role="menuitem"
                                  disabled={busy}
                                  className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                                  onClick={() => {
                                    setActionsMenuOpen(false);
                                    void executeMutation(
                                      "make_global",
                                      details.skill.skill_key,
                                    );
                                  }}
                                >
                                  Make global
                                </button>
                              ) : null}
                            </>
                          ) : (
                            <button
                              type="button"
                              role="menuitem"
                              disabled={busy}
                              className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                              onClick={() => {
                                setActionsMenuOpen(false);
                                void executeMutation(
                                  "restore_skill",
                                  details.skill.skill_key,
                                );
                              }}
                            >
                              Restore
                            </button>
                          )}
                          <button
                            type="button"
                            role="menuitem"
                            disabled={busy}
                            className="block w-full rounded-sm px-2 py-1.5 text-left text-xs text-destructive hover:bg-destructive/10 disabled:cursor-not-allowed disabled:opacity-50"
                            onClick={() => {
                              setActionsMenuOpen(false);
                              setDeleteDialog({
                                skillKey: details.skill.skill_key,
                                confirmText: "",
                              });
                            }}
                          >
                            Delete
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </div>
                </CardHeader>

                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="text-muted-foreground">Workspace</dt>
                      <dd className="mt-0.5 break-all font-mono">
                        {details.skill.workspace ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Updated</dt>
                      <dd className="mt-0.5">
                        {formatUnixTime(details.last_modified_unix_seconds)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Main file</dt>
                      <dd className="mt-0.5 flex items-center gap-2 font-mono">
                        <span title={details.main_file_path}>
                          {compactPath(details.main_file_path)}
                        </span>
                        <Button
                          size="sm"
                          variant="ghost"
                          aria-label="Copy main path"
                          onClick={() =>
                            void copyPath(
                              details.main_file_path,
                              "Copy main path failed.",
                            )
                          }
                        >
                          Copy
                        </Button>
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Canonical path</dt>
                      <dd className="mt-0.5 flex items-center gap-2 font-mono">
                        <span title={details.skill.canonical_source_path}>
                          {compactPath(details.skill.canonical_source_path)}
                        </span>
                        <Button
                          size="sm"
                          variant="ghost"
                          aria-label="Copy canonical path"
                          onClick={() =>
                            void copyPath(
                              details.skill.canonical_source_path,
                              "Copy canonical path failed.",
                            )
                          }
                        >
                          Copy
                        </Button>
                      </dd>
                    </div>
                  </dl>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      SKILL.md preview
                    </h3>
                    {details.main_file_body_preview ? (
                      <pre className="max-h-64 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
                        {details.main_file_body_preview}
                      </pre>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        No readable preview available.
                      </p>
                    )}
                  </section>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      SKILL dir tree
                    </h3>
                    {details.skill_dir_tree_preview ? (
                      <pre className="max-h-48 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
                        {details.skill_dir_tree_preview}
                      </pre>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        No readable directory tree available.
                      </p>
                    )}
                  </section>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Targets
                    </h3>
                    {details.skill.target_paths.length === 0 ? (
                      <p className="text-xs text-muted-foreground">
                        No target paths.
                      </p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {details.skill.target_paths.map((path) => (
                          <li
                            key={path}
                            className="rounded-md bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>

                  {details.skill.status === "active" ? (
                    <form
                      className="flex flex-wrap items-center gap-2 border-t border-border/50 pt-3"
                      onSubmit={(event) => {
                        event.preventDefault();
                        void handleRenameSkill(
                          details.skill.skill_key,
                          renameDraft,
                        );
                      }}
                    >
                      <Input
                        value={renameDraft}
                        onChange={(event) =>
                          setRenameDraft(event.currentTarget.value)
                        }
                        placeholder="New skill title"
                        className="min-w-[220px] flex-1"
                      />
                      <Button
                        type="submit"
                        size="sm"
                        disabled={
                          busy ||
                          renameDraft.trim().length === 0 ||
                          renameDraft.trim() === details.skill.name
                        }
                      >
                        Save name
                      </Button>
                    </form>
                  ) : null}
                </CardContent>
              </>
            ) : null}

            {showMcp ? (
              <>
                <CardHeader className="border-b border-border/60 pb-3">
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div>
                      <CardTitle className="text-lg leading-tight">
                        {selectedMcpServer.server_key}
                      </CardTitle>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {`${selectedMcpServer.transport.toUpperCase()} · ${toTitleCase(selectedMcpServer.scope)}`}
                      </p>
                    </div>
                    <div />
                  </div>
                </CardHeader>
                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="text-muted-foreground">Command</dt>
                      <dd className="mt-0.5 break-all font-mono">
                        {selectedMcpServer.command ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">URL</dt>
                      <dd className="mt-0.5 break-all font-mono">
                        {selectedMcpServer.url ?? "-"}
                      </dd>
                    </div>
                  </dl>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Enable by agent
                    </h3>
                    <div className="flex flex-wrap gap-3">
                      {getVisibleMcpAgents(selectedMcpServer.scope).map(
                        (agent) => {
                          const enabled =
                            selectedMcpServer.enabled_by_agent[agent];
                          return (
                            <div
                              key={agent}
                              className="inline-flex items-center gap-2 px-1 py-1"
                            >
                              <span className="text-xs font-medium">
                                {agent}
                              </span>
                              <button
                                type="button"
                                role="switch"
                                aria-label={`${agent} toggle`}
                                aria-checked={enabled}
                                disabled={busy}
                                onClick={() =>
                                  void handleSetMcpEnabled(
                                    selectedMcpServer,
                                    agent,
                                    !enabled,
                                  )
                                }
                                className={cn(
                                  "relative inline-flex h-6 w-11 items-center rounded-full border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60",
                                  enabled
                                    ? "border-primary/70 bg-primary/80"
                                    : "border-border bg-muted-foreground/25",
                                )}
                              >
                                <span
                                  aria-hidden="true"
                                  className={cn(
                                    "inline-block h-4 w-4 transform rounded-full bg-background shadow-sm transition-transform",
                                    enabled ? "translate-x-5" : "translate-x-1",
                                  )}
                                />
                              </button>
                            </div>
                          );
                        },
                      )}
                    </div>
                  </section>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Args
                    </h3>
                    {selectedMcpServer.args.length === 0 ? (
                      <p className="text-xs text-muted-foreground">No args.</p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {selectedMcpServer.args.map((arg) => (
                          <li
                            key={arg}
                            className="rounded-md bg-muted/20 p-2 font-mono"
                          >
                            {arg}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Targets
                    </h3>
                    {selectedMcpServer.targets.length === 0 ? (
                      <p className="text-xs text-muted-foreground">
                        No managed targets.
                      </p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {selectedMcpServer.targets.map((path) => (
                          <li
                            key={path}
                            className="rounded-md bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                </CardContent>
              </>
            ) : null}

            {showSubagent ? (
              <>
                <CardHeader className="border-b border-border/60 pb-3">
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div>
                      <CardTitle className="text-lg leading-tight">
                        {subagentDetails.subagent.name}
                      </CardTitle>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {subagentDetails.subagent.subagent_key}
                      </p>
                    </div>
                    <div className="relative flex items-center gap-1.5">
                      <Button
                        size="sm"
                        variant="outline"
                        aria-expanded={openTargetMenu === "subagent"}
                        onClick={() => {
                          setOpenTargetMenu((prev) =>
                            prev === "subagent" ? null : "subagent",
                          );
                          setActionsMenuOpen(false);
                        }}
                      >
                        Open…
                      </Button>

                      {openTargetMenu === "subagent" ? (
                        <div
                          role="menu"
                          className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
                        >
                          <button
                            type="button"
                            role="menuitem"
                            className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent"
                            onClick={() =>
                              void handleOpenSubagentPath(
                                subagentDetails.subagent.id,
                                "folder",
                              )
                            }
                          >
                            Open folder
                          </button>
                          <button
                            type="button"
                            role="menuitem"
                            disabled={!subagentDetails.main_file_exists}
                            className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:opacity-50"
                            onClick={() =>
                              void handleOpenSubagentPath(
                                subagentDetails.subagent.id,
                                "file",
                              )
                            }
                          >
                            Open file
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </div>
                </CardHeader>

                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="text-muted-foreground">Workspace</dt>
                      <dd className="mt-0.5 break-all font-mono">
                        {subagentDetails.subagent.workspace ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Updated</dt>
                      <dd className="mt-0.5">
                        {formatUnixTime(
                          subagentDetails.last_modified_unix_seconds,
                        )}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Main file</dt>
                      <dd className="mt-0.5 break-all font-mono">
                        {compactPath(subagentDetails.main_file_path)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground">Canonical path</dt>
                      <dd
                        className="mt-0.5 break-all font-mono"
                        title={subagentDetails.subagent.canonical_source_path}
                      >
                        {compactPath(
                          subagentDetails.subagent.canonical_source_path,
                        )}
                      </dd>
                    </div>
                  </dl>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Targets
                    </h3>
                    {subagentDetails.subagent.target_paths.length === 0 ? (
                      <p className="text-xs text-muted-foreground">
                        No target paths.
                      </p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {subagentDetails.subagent.target_paths.map((path) => (
                          <li
                            key={path}
                            className="rounded-md bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>

                  <section className="space-y-1.5 border-t border-border/50 pt-3">
                    <h3 className="text-xs font-semibold text-muted-foreground">
                      Subagent prompt preview
                    </h3>
                    {subagentDetails.main_file_body_preview ? (
                      <pre className="max-h-64 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
                        {subagentDetails.main_file_body_preview}
                      </pre>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        No readable preview available.
                      </p>
                    )}
                  </section>
                </CardContent>
              </>
            ) : null}
          </Card>
        </main>
      </div>

      {auditOpen ? (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4">
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Audit log"
            className="flex h-[80vh] w-full max-w-4xl flex-col rounded-md border border-border/70 bg-card p-4"
          >
            <div className="flex items-center justify-between gap-2">
              <h2 className="text-sm font-semibold">Audit log</h2>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAuditOpen(false)}
              >
                Close
              </Button>
            </div>
            <div className="mt-3 flex flex-wrap items-end gap-2">
              <label
                className="text-xs text-muted-foreground"
                htmlFor="audit-status-filter"
              >
                Status
                <select
                  id="audit-status-filter"
                  aria-label="Audit status filter"
                  className="mt-1 block rounded-md border border-border/70 bg-background px-2 py-1 text-xs"
                  value={auditStatusFilter}
                  onChange={(event) =>
                    setAuditStatusFilter(
                      parseAuditStatusFilter(event.currentTarget.value),
                    )
                  }
                >
                  <option value="all">all</option>
                  <option value="success">success</option>
                  <option value="failed">failed</option>
                  <option value="blocked">blocked</option>
                </select>
              </label>
              <label
                className="text-xs text-muted-foreground"
                htmlFor="audit-action-filter"
              >
                Action
                <Input
                  id="audit-action-filter"
                  aria-label="Audit action filter"
                  value={auditActionFilter}
                  placeholder="run_sync"
                  onChange={(event) =>
                    setAuditActionFilter(event.currentTarget.value)
                  }
                  className="mt-1 min-w-[220px]"
                />
              </label>
              <Button
                size="sm"
                variant="outline"
                disabled={auditBusy}
                onClick={() => void loadAudit()}
              >
                Apply
              </Button>
            </div>
            <div className="mt-3 min-h-0 flex-1 overflow-auto rounded-md border border-border/50">
              {auditEvents.length === 0 ? (
                <p className="p-3 text-xs text-muted-foreground">
                  No audit events.
                </p>
              ) : (
                <ul className="space-y-1 p-2">
                  {auditEvents.map((event) => (
                    <li
                      key={event.id}
                      className="rounded-md border border-border/40 bg-muted/20 p-2"
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <span className="font-mono text-[11px]">
                          {formatIsoTime(event.occurred_at)}
                        </span>
                        <Badge
                          variant={
                            event.status === "success"
                              ? "success"
                              : event.status === "blocked"
                                ? "warning"
                                : "error"
                          }
                        >
                          {event.status}
                        </Badge>
                      </div>
                      <p className="mt-1 text-xs font-medium">
                        {event.action}
                        {event.trigger ? ` (${event.trigger})` : ""}
                      </p>
                      <p className="mt-0.5 text-xs text-muted-foreground">
                        {event.summary}
                      </p>
                      {event.paths.length > 0 ? (
                        <p className="mt-1 truncate font-mono text-[11px]">
                          {event.paths.join(" · ")}
                        </p>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
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
              Type DELETE to remove this skill.
            </p>
            <label
              className="mt-3 block text-xs text-muted-foreground"
              htmlFor="delete-confirm-input"
            >
              Type DELETE to confirm
            </label>
            <Input
              id="delete-confirm-input"
              aria-label="Type DELETE to confirm"
              value={deleteDialog.confirmText}
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                setDeleteDialog((prev) =>
                  prev
                    ? {
                        ...prev,
                        confirmText: nextValue,
                      }
                    : prev,
                );
              }}
              className="mt-1"
            />
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
                disabled={deleteDialog.confirmText !== "DELETE" || busy}
                onClick={() => {
                  const skillKey = deleteDialog.skillKey;
                  setDeleteDialog(null);
                  void executeMutation("delete_skill", skillKey);
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
