import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { cn } from "./lib/utils";
import {
  getStarredSkillIds,
  getSkillDetails,
  getSubagentDetails,
  getState,
  listSubagents,
  mutateSkill,
  openSubagentPath,
  openSkillPath,
  renameSkill,
  runSync,
  setMcpServerEnabled,
  setSkillStarred,
} from "./tauriApi";
import {
  formatUnixTime,
  normalizeSkillKey,
  pickSelectedSkillKey,
  sortAndFilterSkills,
} from "./skillUtils";
import type {
  MutationCommand,
  McpServerRecord,
  SubagentDetails,
  SubagentRecord,
  SkillDetails,
  SkillLifecycleStatus,
  SyncHealthStatus,
  SyncState,
} from "./types";

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

function lifecycleVariant(status: SkillLifecycleStatus) {
  return status === "active" ? ("success" as const) : ("outline" as const);
}

type StarIconProps = {
  active: boolean;
  className?: string;
};

function StarIcon({ active, className }: StarIconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      fill={active ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth={1.75}
      aria-hidden="true"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="m12 3.5 2.67 5.41 5.98.87-4.32 4.21 1.02 5.95L12 17.13 6.65 19.94l1.02-5.95-4.32-4.21 5.98-.87L12 3.5Z"
      />
    </svg>
  );
}

type PendingMutation = { command: MutationCommand; skillKey: string };
type CatalogTab = "skills" | "subagents" | "mcp";

export function App() {
  const [state, setState] = useState<SyncState | null>(null);
  const [details, setDetails] = useState<SkillDetails | null>(null);
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [subagentDetails, setSubagentDetails] =
    useState<SubagentDetails | null>(null);
  const [activeTab, setActiveTab] = useState<CatalogTab>("skills");
  const [selectedSkillKey, setSelectedSkillKey] = useState<string | null>(null);
  const [selectedSubagentId, setSelectedSubagentId] = useState<string | null>(
    null,
  );
  const [selectedMcpKey, setSelectedMcpKey] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [renameDraft, setRenameDraft] = useState("");
  const [starredSkillIds, setStarredSkillIds] = useState<string[]>([]);
  const [pendingMutation, setPendingMutation] =
    useState<PendingMutation | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const applyState = useCallback(
    (next: SyncState, preferredKey?: string | null) => {
      setState(next);
      setSelectedSkillKey((previousKey) =>
        pickSelectedSkillKey(next.skills, preferredKey, previousKey),
      );
    },
    [],
  );

  const refreshState = useCallback(
    async (preferredKey?: string | null) => {
      setBusy(true);
      setError(null);
      try {
        const next = await runSync();
        const [nextStarred, nextSubagents] = await Promise.all([
          getStarredSkillIds(),
          listSubagents("all"),
        ]);
        setStarredSkillIds(nextStarred);
        setSubagents(nextSubagents);
        setSelectedSubagentId((prev) => {
          if (
            preferredKey &&
            nextSubagents.some((i) => i.id === preferredKey)
          ) {
            return preferredKey;
          }
          if (prev && nextSubagents.some((i) => i.id === prev)) {
            return prev;
          }
          return nextSubagents[0]?.id ?? null;
        });
        applyState(next, preferredKey);
      } catch (invokeError) {
        setError(String(invokeError));
        try {
          const [fallbackState, nextStarred, nextSubagents] = await Promise.all(
            [getState(), getStarredSkillIds(), listSubagents("all")],
          );
          setStarredSkillIds(nextStarred);
          setSubagents(nextSubagents);
          setSelectedSubagentId((prev) => {
            if (prev && nextSubagents.some((i) => i.id === prev)) {
              return prev;
            }
            return nextSubagents[0]?.id ?? null;
          });
          applyState(fallbackState, preferredKey);
        } catch (fallbackError) {
          setError(
            `${String(invokeError)}\nFallback failed: ${String(fallbackError)}`,
          );
        }
      } finally {
        setBusy(false);
      }
    },
    [applyState],
  );

  useEffect(() => {
    void refreshState();
  }, [refreshState]);

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

  const filteredSkills = useMemo(() => {
    if (!state) return [];
    return sortAndFilterSkills(state.skills, query, starredSkillIds);
  }, [query, state, starredSkillIds]);

  const filteredSubagents = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
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
  }, [query, subagents]);

  const filteredMcpServers = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
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
  }, [query, state]);

  async function handleSetSkillStarred(skillId: string, starred: boolean) {
    setBusy(true);
    setError(null);
    try {
      const next = await setSkillStarred(skillId, starred);
      setStarredSkillIds(next);
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setBusy(false);
    }
  }

  function requestMutation(command: MutationCommand, skillKey: string) {
    setPendingMutation({ command, skillKey });
  }

  async function handlePendingMutation() {
    if (!pendingMutation) return;
    const { command, skillKey } = pendingMutation;

    setBusy(true);
    setError(null);
    setPendingMutation(null);
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

  const activeSkillCount =
    state?.skills.filter((skill) => skill.status === "active").length ?? 0;
  const archivedSkillCount =
    state?.skills.filter((skill) => skill.status === "archived").length ?? 0;
  const isDetailsSkillStarred =
    details != null && starredSkillIds.includes(details.skill.id);
  const activeSubagentCount = subagents.length;
  const selectedMcpServer =
    state?.mcp_servers?.find(
      (item) => mcpSelectionKey(item) === selectedMcpKey,
    ) ?? null;
  const mcpCount = state?.summary.mcp_count ?? state?.mcp_servers?.length ?? 0;
  const skillsFiltered = filteredSkills.length;
  const skillsTotal = state?.skills.length ?? 0;
  const subagentsFiltered = filteredSubagents.length;
  const subagentsTotal = subagents.length;
  const mcpFiltered = filteredMcpServers.length;
  const mcpTotal = mcpCount;

  return (
    <div className="min-h-full bg-background text-foreground lg:h-screen lg:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1400px] flex-col gap-2.5 p-3 lg:h-full lg:min-h-0 lg:p-4">
        <header className="shrink-0 rounded-lg border border-border/90 bg-card/92 px-3 py-2 shadow-[0_1px_0_hsl(var(--foreground)/0.04)]">
          <div className="flex flex-wrap items-start justify-between gap-3">
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
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={busy}
                onClick={() => void refreshState(selectedSkillKey)}
              >
                Refresh
              </Button>
            </div>
          </div>
          <div className="mt-2">
            <Input
              value={query}
              placeholder="Search by name, key, scope or workspace"
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Button
                aria-label="Skills"
                size="default"
                variant={activeTab === "skills" ? "default" : "outline"}
                className={cn(
                  "h-[calc(var(--control-height)+4px)] rounded-lg px-4 text-[13px]",
                  activeTab === "skills"
                    ? "border-transparent"
                    : "border-border/85",
                )}
                disabled={busy}
                onClick={() => setActiveTab("skills")}
              >
                <span className="text-dense font-semibold tracking-tight">
                  Skills
                </span>
                <span
                  aria-hidden="true"
                  className={cn(
                    "ml-2 inline-flex min-w-12 items-center justify-center rounded-md px-2 py-0.5 text-[11px] font-semibold tabular-nums leading-none",
                    activeTab === "skills"
                      ? "bg-primary-foreground/16 text-primary-foreground"
                      : "bg-muted text-muted-foreground",
                  )}
                >
                  {skillsFiltered}/{skillsTotal}
                </span>
              </Button>
              <Button
                aria-label="Subagents"
                size="default"
                variant={activeTab === "subagents" ? "default" : "outline"}
                className={cn(
                  "h-[calc(var(--control-height)+4px)] rounded-lg px-4 text-[13px]",
                  activeTab === "subagents"
                    ? "border-transparent"
                    : "border-border/85",
                )}
                disabled={busy}
                onClick={() => setActiveTab("subagents")}
              >
                <span className="text-dense font-semibold tracking-tight">
                  Subagents
                </span>
                <span
                  aria-hidden="true"
                  className={cn(
                    "ml-2 inline-flex min-w-12 items-center justify-center rounded-md px-2 py-0.5 text-[11px] font-semibold tabular-nums leading-none",
                    activeTab === "subagents"
                      ? "bg-primary-foreground/16 text-primary-foreground"
                      : "bg-muted text-muted-foreground",
                  )}
                >
                  {subagentsFiltered}/{subagentsTotal}
                </span>
              </Button>
              <Button
                aria-label="MCP"
                size="default"
                variant={activeTab === "mcp" ? "default" : "outline"}
                className={cn(
                  "h-[calc(var(--control-height)+4px)] rounded-lg px-4 text-[13px]",
                  activeTab === "mcp" ? "border-transparent" : "border-border/85",
                )}
                disabled={busy}
                onClick={() => setActiveTab("mcp")}
              >
                <span className="text-dense font-semibold tracking-tight">
                  MCP
                </span>
                <span
                  aria-hidden="true"
                  className={cn(
                    "ml-2 inline-flex min-w-12 items-center justify-center rounded-md px-2 py-0.5 text-[11px] font-semibold tabular-nums leading-none",
                    activeTab === "mcp"
                      ? "bg-primary-foreground/16 text-primary-foreground"
                      : "bg-muted text-muted-foreground",
                  )}
                >
                  {mcpFiltered}/{mcpTotal}
                </span>
              </Button>
            </div>
          </div>
        </header>

        {error ? (
          <Card className="shrink-0 border-destructive/60 bg-destructive/10">
            <CardContent className="p-2 text-xs text-destructive">
              {error}
            </CardContent>
          </Card>
        ) : null}

        {state?.sync.error ? (
          <Card className="shrink-0 border-destructive/60 bg-destructive/10">
            <CardContent className="p-2 text-xs text-destructive">
              {state.sync.error}
            </CardContent>
          </Card>
        ) : null}

        {pendingMutation ? (
          <Card className="shrink-0 border-amber-500/60 bg-amber-500/10">
            <CardContent className="flex flex-wrap items-center justify-between gap-2 p-2">
              <p className="text-xs text-foreground">
                Review action: {pendingMutation.command} on{" "}
                {pendingMutation.skillKey}.
              </p>
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() => void handlePendingMutation()}
                >
                  Apply change
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy}
                  onClick={() => setPendingMutation(null)}
                >
                  Cancel
                </Button>
              </div>
            </CardContent>
          </Card>
        ) : null}

        <main className="grid gap-2.5 lg:min-h-0 lg:flex-1 lg:grid-cols-[340px_minmax(0,1fr)]">
          <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
            <CardHeader className="border-b border-border/80 pb-2">
              <div className="flex items-center justify-between gap-2">
                <CardTitle>
                  {activeTab === "skills"
                    ? "Skills"
                    : activeTab === "subagents"
                      ? "Subagents"
                      : "MCP Servers"}
                </CardTitle>
                <Badge variant="outline">
                  {activeTab === "skills"
                    ? filteredSkills.length
                    : activeTab === "subagents"
                      ? filteredSubagents.length
                      : filteredMcpServers.length}
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="p-2 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
              <ul className="space-y-1">
                {activeTab === "subagents"
                  ? filteredSubagents.map((subagent) => {
                      const selected = subagent.id === selectedSubagentId;
                      return (
                        <li key={subagent.id}>
                          <button
                            type="button"
                            className={cn(
                              "w-full rounded-md border px-2.5 py-2 text-left transition-colors duration-150",
                              "hover:border-border hover:bg-accent/70",
                              selected
                                ? "border-primary/45 bg-accent/80 text-foreground"
                                : "border-border/70 bg-transparent text-foreground",
                            )}
                            onClick={() => setSelectedSubagentId(subagent.id)}
                          >
                            <div className="flex items-center justify-between gap-2">
                              <p className="truncate text-sm font-medium leading-tight">
                                {subagent.name}
                              </p>
                              <Badge variant="outline">
                                {toTitleCase(subagent.scope)}
                              </Badge>
                            </div>
                            <p className="mt-1 truncate font-mono text-[11px] text-muted-foreground">
                              {subagent.subagent_key}
                            </p>
                            {subagent.workspace ? (
                              <p className="mt-1 truncate text-[11px] text-muted-foreground">
                                {subagent.workspace}
                              </p>
                            ) : null}
                          </button>
                        </li>
                      );
                    })
                  : activeTab === "mcp"
                    ? filteredMcpServers.map((server) => {
                        const selectionKey = mcpSelectionKey(server);
                        const selected = selectionKey === selectedMcpKey;
                        return (
                          <li key={selectionKey}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md border px-2.5 py-2 text-left transition-colors duration-150",
                                "hover:border-border hover:bg-accent/70",
                                selected
                                  ? "border-primary/45 bg-accent/80 text-foreground"
                                  : "border-border/70 bg-transparent text-foreground",
                              )}
                              onClick={() => setSelectedMcpKey(selectionKey)}
                            >
                              <div className="flex items-center justify-between gap-2">
                                <p className="truncate text-sm font-medium leading-tight">
                                  {server.server_key}
                                </p>
                                <Badge variant="outline">
                                  {`${server.transport.toUpperCase()} · ${toTitleCase(server.scope)}`}
                                </Badge>
                              </div>
                              <p className="mt-1 truncate text-[11px] text-muted-foreground">
                                {server.command ??
                                  server.url ??
                                  "No command/url"}
                              </p>
                              {server.workspace ? (
                                <p className="mt-1 truncate text-[11px] text-muted-foreground">
                                  {server.workspace}
                                </p>
                              ) : null}
                            </button>
                          </li>
                        );
                      })
                    : filteredSkills.map((skill) => {
                        const selected = skill.skill_key === selectedSkillKey;
                        const isSkillStarred = starredSkillIds.includes(
                          skill.id,
                        );
                        const hasDistinctSkillKey =
                          skill.name.trim().toLowerCase() !==
                          skill.skill_key.trim().toLowerCase();
                        return (
                          <li key={skill.id}>
                            <button
                              type="button"
                              className={cn(
                                "w-full rounded-md border px-2.5 py-2 text-left transition-colors duration-150",
                                "hover:border-border hover:bg-accent/70",
                                selected
                                  ? "border-primary/45 bg-accent/80 text-foreground"
                                  : "border-border/70 bg-transparent text-foreground",
                              )}
                              onClick={() =>
                                setSelectedSkillKey(skill.skill_key)
                              }
                            >
                              <div className="flex items-center justify-between gap-2">
                                <p className="truncate text-sm font-medium leading-tight">
                                  {isSkillStarred ? (
                                    <StarIcon
                                      active
                                      className="mr-1 inline-block h-3.5 w-3.5 align-[-2px] text-amber-500"
                                    />
                                  ) : null}
                                  {skill.name}
                                </p>
                                <div className="flex items-center gap-1.5">
                                  {skill.status !== "active" ? (
                                    <Badge
                                      variant={lifecycleVariant(skill.status)}
                                    >
                                      {toTitleCase(skill.status)}
                                    </Badge>
                                  ) : null}
                                  <Badge variant="outline">
                                    {toTitleCase(skill.scope)}
                                  </Badge>
                                </div>
                              </div>
                              {hasDistinctSkillKey ? (
                                <p className="mt-1 truncate font-mono text-[11px] text-muted-foreground">
                                  {skill.skill_key}
                                </p>
                              ) : null}
                              {skill.workspace ? (
                                <p className="mt-1 truncate text-[11px] text-muted-foreground">
                                  {skill.workspace}
                                </p>
                              ) : null}
                            </button>
                          </li>
                        );
                      })}
              </ul>
            </CardContent>
          </Card>

          <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
            {activeTab === "skills" && !details ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select a skill to view details.
              </CardContent>
            ) : activeTab === "subagents" && !subagentDetails ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select a subagent to view details.
              </CardContent>
            ) : activeTab === "mcp" && !selectedMcpServer ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select an MCP server to view details.
              </CardContent>
            ) : activeTab === "skills" && details ? (
              <>
                <CardHeader className="border-b border-border/80 pb-2">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <div className="flex items-center gap-2">
                        <CardTitle className="text-base">
                          {details.skill.name}
                        </CardTitle>
                        <button
                          type="button"
                          disabled={busy}
                          aria-label={isDetailsSkillStarred ? "Unstar" : "Star"}
                          title={isDetailsSkillStarred ? "Unstar" : "Star"}
                          className={cn(
                            "inline-flex h-6 w-6 items-center justify-center rounded-md border transition-colors",
                            "disabled:pointer-events-none disabled:opacity-50",
                            isDetailsSkillStarred
                              ? "border-amber-500/65 bg-amber-500/15 text-amber-700 hover:bg-amber-500/25 dark:text-amber-300"
                              : "border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground",
                          )}
                          onClick={() =>
                            void handleSetSkillStarred(
                              details.skill.id,
                              !isDetailsSkillStarred,
                            )
                          }
                        >
                          <StarIcon
                            active={isDetailsSkillStarred}
                            className="h-4 w-4"
                          />
                        </button>
                      </div>
                      <p className="mt-1 font-mono text-xs text-muted-foreground">
                        {details.skill.skill_key}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      {details.skill.status !== "active" ? (
                        <Badge variant={lifecycleVariant(details.skill.status)}>
                          {toTitleCase(details.skill.status)}
                        </Badge>
                      ) : null}
                      <Badge variant="outline">
                        {toTitleCase(details.skill.scope)}
                      </Badge>
                    </div>
                  </div>
                </CardHeader>

                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-3 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="mb-1 text-muted-foreground">Workspace</dt>
                      <dd className="break-all font-mono">
                        {details.skill.workspace ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Updated</dt>
                      <dd>
                        {formatUnixTime(details.last_modified_unix_seconds)}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Main file</dt>
                      <dd className="break-all font-mono">
                        {details.main_file_path}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">
                        Canonical path
                      </dt>
                      <dd className="break-all font-mono">
                        {details.skill.canonical_source_path}
                      </dd>
                    </div>
                  </dl>

                  <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border/80 pt-3">
                    <div className="flex flex-wrap items-center gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={busy}
                        onClick={() =>
                          void handleOpenSkillPath(
                            details.skill.skill_key,
                            "folder",
                          )
                        }
                      >
                        Open folder
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={busy || !details.main_file_exists}
                        onClick={() =>
                          void handleOpenSkillPath(
                            details.skill.skill_key,
                            "file",
                          )
                        }
                      >
                        Open file
                      </Button>
                    </div>

                    <div className="ml-auto flex flex-wrap items-center gap-2">
                      {details.skill.status === "active" ? (
                        <>
                          <Button
                            size="sm"
                            variant="ghost"
                            disabled={busy}
                            onClick={() =>
                              requestMutation(
                                "archive_skill",
                                details.skill.skill_key,
                              )
                            }
                          >
                            Archive
                          </Button>
                          {details.skill.scope === "project" ? (
                            <Button
                              size="sm"
                              variant="ghost"
                              disabled={busy}
                              onClick={() =>
                                requestMutation(
                                  "make_global",
                                  details.skill.skill_key,
                                )
                              }
                            >
                              Make global
                            </Button>
                          ) : null}
                        </>
                      ) : (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={busy}
                          onClick={() =>
                            requestMutation(
                              "restore_skill",
                              details.skill.skill_key,
                            )
                          }
                        >
                          Restore
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="destructive"
                        disabled={busy}
                        onClick={() =>
                          requestMutation(
                            "delete_skill",
                            details.skill.skill_key,
                          )
                        }
                      >
                        Delete
                      </Button>
                    </div>
                  </div>

                  {details.skill.status === "active" ? (
                    <form
                      className="flex flex-wrap items-center gap-2 border-t border-border/80 pt-3"
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

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      SKILL.md preview
                    </h3>
                    {details.main_file_body_preview ? (
                      <>
                        <pre className="max-h-64 overflow-auto rounded-md border border-border/70 bg-muted/35 p-2 font-mono text-[11px] leading-relaxed">
                          {details.main_file_body_preview}
                        </pre>
                        {details.main_file_body_preview_truncated ? (
                          <p className="text-[11px] text-muted-foreground">
                            Preview truncated.{" "}
                            <button
                              type="button"
                              disabled={busy || !details.main_file_exists}
                              className="underline underline-offset-2 transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50"
                              onClick={() =>
                                void handleOpenSkillPath(
                                  details.skill.skill_key,
                                  "file",
                                )
                              }
                            >
                              watch full
                            </button>
                            .
                          </p>
                        ) : null}
                      </>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        No readable preview available.
                      </p>
                    )}
                  </section>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      SKILL dir tree
                    </h3>
                    {details.skill_dir_tree_preview ? (
                      <>
                        <pre className="max-h-48 overflow-auto rounded-md border border-border/70 bg-muted/35 p-2 font-mono text-[11px] leading-relaxed">
                          {details.skill_dir_tree_preview}
                        </pre>
                        {details.skill_dir_tree_preview_truncated ? (
                          <p className="text-[11px] text-muted-foreground">
                            Tree preview truncated for performance.
                          </p>
                        ) : null}
                      </>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        No readable directory tree available.
                      </p>
                    )}
                  </section>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
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
                            className="break-all rounded-md border border-border/60 bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                </CardContent>
              </>
            ) : activeTab === "mcp" && selectedMcpServer ? (
              <>
                <CardHeader className="border-b border-border/80 pb-2">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <CardTitle className="text-base">
                        {selectedMcpServer.server_key}
                      </CardTitle>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {`${selectedMcpServer.transport.toUpperCase()} · ${toTitleCase(selectedMcpServer.scope)}`}
                      </p>
                    </div>
                    <Badge variant="outline">
                      {selectedMcpServer.warnings.length > 0
                        ? `${selectedMcpServer.warnings.length} warning(s)`
                        : "Clean"}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-3 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="mb-1 text-muted-foreground">Command</dt>
                      <dd className="break-all font-mono">
                        {selectedMcpServer.command ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">URL</dt>
                      <dd className="break-all font-mono">
                        {selectedMcpServer.url ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Scope</dt>
                      <dd>{toTitleCase(selectedMcpServer.scope)}</dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Workspace</dt>
                      <dd className="break-all font-mono">
                        {selectedMcpServer.workspace ?? "-"}
                      </dd>
                    </div>
                  </dl>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Enable by agent
                    </h3>
                    <div className="flex flex-wrap gap-3">
                      {(selectedMcpServer.scope === "global"
                        ? (["codex", "claude"] as const)
                        : (["codex", "claude", "project"] as const)
                      ).map((agent) => {
                        const enabled =
                          selectedMcpServer.enabled_by_agent[agent];
                        return (
                          <div
                            key={agent}
                            className="inline-flex items-center gap-2 rounded-md border border-border/70 bg-muted/20 px-2 py-1"
                          >
                            <span className="text-xs font-medium">{agent}</span>
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
                      })}
                    </div>
                  </section>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Args
                    </h3>
                    {selectedMcpServer.args.length === 0 ? (
                      <p className="text-xs text-muted-foreground">No args.</p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {selectedMcpServer.args.map((arg) => (
                          <li
                            key={arg}
                            className="break-all rounded-md border border-border/60 bg-muted/20 p-2 font-mono"
                          >
                            {arg}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
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
                            className="break-all rounded-md border border-border/60 bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>

                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Env
                    </h3>
                    {Object.keys(selectedMcpServer.env).length === 0 ? (
                      <p className="text-xs text-muted-foreground">
                        No env values.
                      </p>
                    ) : (
                      <ul className="space-y-1 text-xs">
                        {Object.entries(selectedMcpServer.env).map(
                          ([key, value]) => (
                            <li
                              key={key}
                              className="break-all rounded-md border border-border/60 bg-muted/20 p-2 font-mono"
                            >
                              {key}={value}
                            </li>
                          ),
                        )}
                      </ul>
                    )}
                  </section>
                </CardContent>
              </>
            ) : subagentDetails ? (
              <>
                <CardHeader className="border-b border-border/80 pb-2">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <CardTitle className="text-base">
                        {subagentDetails.subagent.name}
                      </CardTitle>
                      <p className="mt-1 font-mono text-xs text-muted-foreground">
                        {subagentDetails.subagent.subagent_key}
                      </p>
                    </div>
                    <Badge variant="outline">
                      {toTitleCase(subagentDetails.subagent.scope)}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
                  <dl className="grid gap-x-3 gap-y-2 text-xs sm:grid-cols-2">
                    <div>
                      <dt className="mb-1 text-muted-foreground">Workspace</dt>
                      <dd className="break-all font-mono">
                        {subagentDetails.subagent.workspace ?? "-"}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Updated</dt>
                      <dd>
                        {formatUnixTime(
                          subagentDetails.last_modified_unix_seconds,
                        )}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">Main file</dt>
                      <dd className="break-all font-mono">
                        {subagentDetails.main_file_path}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">
                        Canonical path
                      </dt>
                      <dd className="break-all font-mono">
                        {subagentDetails.subagent.canonical_source_path}
                      </dd>
                    </div>
                    <div>
                      <dt className="mb-1 text-muted-foreground">
                        Description
                      </dt>
                      <dd className="break-all">
                        {subagentDetails.subagent.description}
                      </dd>
                    </div>
                  </dl>
                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Symlink metadata
                    </h3>
                    <dl className="grid gap-x-3 gap-y-2 text-xs sm:grid-cols-2">
                      <div>
                        <dt className="mb-1 text-muted-foreground">
                          Symlink target (recorded)
                        </dt>
                        <dd className="break-all font-mono">
                          {subagentDetails.subagent.symlink_target || "-"}
                        </dd>
                      </div>
                      <div>
                        <dt className="mb-1 text-muted-foreground">
                          Is canonical symlink
                        </dt>
                        <dd>
                          {subagentDetails.subagent.is_symlink_canonical
                            ? "Yes"
                            : "No"}
                        </dd>
                      </div>
                    </dl>
                  </section>
                  <div className="flex flex-wrap items-center gap-2 border-t border-border/80 pt-3">
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={() =>
                        void handleOpenSubagentPath(
                          subagentDetails.subagent.id,
                          "folder",
                        )
                      }
                    >
                      Open folder
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy || !subagentDetails.main_file_exists}
                      onClick={() =>
                        void handleOpenSubagentPath(
                          subagentDetails.subagent.id,
                          "file",
                        )
                      }
                    >
                      Open file
                    </Button>
                  </div>
                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
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
                            className="break-all rounded-md border border-border/60 bg-muted/20 p-2 font-mono"
                          >
                            {path}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Target link status
                    </h3>
                    {subagentDetails.target_statuses.length === 0 ? (
                      <p className="text-xs text-muted-foreground">
                        No target diagnostics available.
                      </p>
                    ) : (
                      <ul className="space-y-2 text-xs">
                        {subagentDetails.target_statuses.map((target) => (
                          <li
                            key={target.path}
                            className="space-y-1 rounded-md border border-border/70 bg-muted/20 p-2"
                          >
                            <p className="break-all font-mono text-[11px]">
                              {target.path}
                            </p>
                            <div className="flex flex-wrap gap-2">
                              <Badge variant="outline">{target.kind}</Badge>
                              <Badge variant="outline">
                                exists:{target.exists ? "yes" : "no"}
                              </Badge>
                              <Badge variant="outline">
                                symlink:{target.is_symlink ? "yes" : "no"}
                              </Badge>
                              <Badge variant="outline">
                                canonical:
                                {target.points_to_canonical ? "yes" : "no"}
                              </Badge>
                            </div>
                            <p className="break-all font-mono text-[11px] text-muted-foreground">
                              symlink_target: {target.symlink_target ?? "-"}
                            </p>
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section className="space-y-2 border-t border-border/80 pt-3">
                    <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
                      Subagent prompt preview
                    </h3>
                    {subagentDetails.main_file_body_preview ? (
                      <pre className="max-h-64 overflow-auto rounded-md border border-border/70 bg-muted/35 p-2 font-mono text-[11px] leading-relaxed">
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
    </div>
  );
}
