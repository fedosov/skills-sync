import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { cn } from "./lib/utils";
import {
  getSkillDetails,
  getState,
  mutateSkill,
  openSkillPath,
  renameSkill,
} from "./tauriApi";
import {
  formatUnixTime,
  normalizeSkillKey,
  pickSelectedSkillKey,
  sortAndFilterSkills,
} from "./skillUtils";
import type {
  MutationCommand,
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

type PendingMutation = { command: MutationCommand; skillKey: string };

export function App() {
  const [state, setState] = useState<SyncState | null>(null);
  const [details, setDetails] = useState<SkillDetails | null>(null);
  const [selectedSkillKey, setSelectedSkillKey] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [renameDraft, setRenameDraft] = useState("");
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
        const next = await getState();
        applyState(next, preferredKey);
      } catch (invokeError) {
        setError(String(invokeError));
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

  const filteredSkills = useMemo(() => {
    if (!state) return [];
    return sortAndFilterSkills(state.skills, query);
  }, [query, state]);

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

  const activeSkillCount =
    state?.skills.filter((skill) => skill.status === "active").length ?? 0;
  const archivedSkillCount =
    state?.skills.filter((skill) => skill.status === "archived").length ?? 0;

  return (
    <div className="min-h-full bg-background text-foreground lg:h-screen lg:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1400px] flex-col gap-3 p-3 lg:h-full lg:min-h-0 lg:p-4">
        <header className="shrink-0 rounded-lg border border-border/90 bg-card/85 px-3 py-2 backdrop-blur">
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
                Total {state?.skills.length ?? 0}
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
                Confirm {pendingMutation.command} for {pendingMutation.skillKey}
                ?
              </p>
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() => void handlePendingMutation()}
                >
                  Confirm action
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

        <main className="grid gap-3 lg:min-h-0 lg:flex-1 lg:grid-cols-[340px_minmax(0,1fr)]">
          <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
            <CardHeader className="border-b border-border/80 pb-2">
              <div className="flex items-center justify-between gap-2">
                <CardTitle>Skills</CardTitle>
                <Badge variant="outline">{filteredSkills.length}</Badge>
              </div>
            </CardHeader>
            <CardContent className="p-2 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
              <ul className="space-y-1">
                {filteredSkills.map((skill) => {
                  const selected = skill.skill_key === selectedSkillKey;
                  const hasDistinctSkillKey =
                    skill.name.trim().toLowerCase() !==
                    skill.skill_key.trim().toLowerCase();
                  return (
                    <li key={skill.id}>
                      <button
                        type="button"
                        className={cn(
                          "w-full rounded-md border px-2.5 py-2 text-left transition-colors",
                          "hover:border-border hover:bg-accent/60",
                          selected
                            ? "border-primary/45 bg-accent text-foreground"
                            : "border-border/70 bg-transparent text-foreground",
                        )}
                        onClick={() => setSelectedSkillKey(skill.skill_key)}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <p className="truncate text-sm font-medium leading-tight">
                            {skill.name}
                          </p>
                          <div className="flex items-center gap-1.5">
                            <Badge variant={lifecycleVariant(skill.status)}>
                              {toTitleCase(skill.status)}
                            </Badge>
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
            {!details ? (
              <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
                Select a skill to view details.
              </CardContent>
            ) : (
              <>
                <CardHeader className="border-b border-border/80 pb-2">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <CardTitle className="text-base">
                        {details.skill.name}
                      </CardTitle>
                      <p className="mt-1 font-mono text-xs text-muted-foreground">
                        {details.skill.skill_key}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge variant={lifecycleVariant(details.skill.status)}>
                        {toTitleCase(details.skill.status)}
                      </Badge>
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

                  <div className="flex flex-wrap gap-2 border-t border-border/80 pt-3">
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
                        requestMutation("delete_skill", details.skill.skill_key)
                      }
                    >
                      Delete
                    </Button>
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
                        className="h-8 min-w-[220px] flex-1"
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
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
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
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
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
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
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
            )}
          </Card>
        </main>
      </div>
    </div>
  );
}
