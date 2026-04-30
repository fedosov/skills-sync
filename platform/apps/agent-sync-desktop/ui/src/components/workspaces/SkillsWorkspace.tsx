import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";
import { cn, commandFailureMessage, errorMessage } from "../../lib/utils";
import { OutputPanel } from "../shared/OutputPanel";
import { RemovalActions } from "../shared/RemovalActions";
import {
  getSkillsWorkspaceContext,
  listSkillsCli,
  runSkillsCliCommand,
  setSkillsActiveAgents,
  setSkillsScope,
} from "../../tauriApi";
import type {
  SkillsCliCommandResult,
  SkillsCliListItem,
  SkillsCliScope,
  SkillsWorkspaceContext,
} from "../../types";

const KNOWN_AGENTS: readonly string[] = [
  "Claude Code",
  "Cursor",
  "Codex",
  "Cline",
  "Windsurf",
  "Continue",
  "Aider",
  "Roo Code",
];

function uniqueAgents(values: readonly string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    if (!seen.has(value)) {
      seen.add(value);
      result.push(value);
    }
  }
  return result;
}

type SkillsWorkspaceProps = {
  onReady?: () => void;
};

export function SkillsWorkspace({ onReady }: SkillsWorkspaceProps = {}) {
  const readyFiredRef = useRef(false);
  const [workspace, setWorkspace] = useState<SkillsWorkspaceContext | null>(
    null,
  );
  const [skills, setSkills] = useState<SkillsCliListItem[]>([]);
  const [lastCommand, setLastCommand] = useState<SkillsCliCommandResult | null>(
    null,
  );
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [pendingRemoval, setPendingRemoval] = useState<string | null>(null);
  const [addSource, setAddSource] = useState("");

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const ctx = await getSkillsWorkspaceContext();
      setWorkspace(ctx);
      if (ctx.runtimeStatus.available) {
        try {
          const items = await listSkillsCli();
          setSkills(items);
        } catch (listError) {
          // The skills CLI may not be installed yet; surface the error
          // but keep the rest of the UI usable.
          setSkills([]);
          setError(errorMessage(listError));
        }
      } else {
        setSkills([]);
      }
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setIsLoading(false);
      if (!readyFiredRef.current) {
        readyFiredRef.current = true;
        onReady?.();
      }
    }
  }, [onReady]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function runBusyAction(
    actionName: string,
    fn: () => Promise<void>,
  ): Promise<void> {
    setBusyAction(actionName);
    setError(null);
    try {
      await fn();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleScopeChange(scope: SkillsCliScope) {
    await runBusyAction(`scope:${scope}`, async () => {
      await setSkillsScope(scope);
      await refresh();
    });
  }

  async function handleToggleAgent(agent: string) {
    if (!workspace) {
      return;
    }
    const current = workspace.state.activeAgents;
    const next = current.includes(agent)
      ? current.filter((a) => a !== agent)
      : [...current, agent];
    await runBusyAction(`agents:toggle:${agent}`, async () => {
      const ctx = await setSkillsActiveAgents(next);
      setWorkspace(ctx);
    });
  }

  async function handleAdd() {
    if (!workspace) {
      return;
    }
    const trimmed = addSource.trim();
    if (!trimmed) {
      setError("Provide a skill source (e.g. owner/repo or a package name).");
      return;
    }
    if (workspace.state.activeAgents.length === 0) {
      setError("Select at least one target agent before adding a skill.");
      return;
    }
    await runBusyAction("add", async () => {
      const result = await runSkillsCliCommand({
        kind: "add",
        source: trimmed,
        agents: workspace.state.activeAgents,
        scope: workspace.state.scope,
      });
      setLastCommand(result);
      if (!result.success) {
        setError(commandFailureMessage(result, "skills command failed"));
        return;
      }
      setAddSource("");
      await refresh();
    });
  }

  async function handleRemove(name: string) {
    if (!workspace) {
      return;
    }
    if (workspace.state.activeAgents.length === 0) {
      setError("Select at least one target agent before removing a skill.");
      return;
    }
    await runBusyAction(`remove:${name}`, async () => {
      const result = await runSkillsCliCommand({
        kind: "remove",
        name,
        agents: workspace.state.activeAgents,
        scope: workspace.state.scope,
      });
      setLastCommand(result);
      if (!result.success) {
        setError(commandFailureMessage(result, "skills command failed"));
        return;
      }
      await refresh();
    });
    setPendingRemoval(null);
  }

  async function handleUpdateAll() {
    if (!workspace) {
      return;
    }
    await runBusyAction("update", async () => {
      const result = await runSkillsCliCommand({
        kind: "update",
        names: [],
        scope: workspace.state.scope,
      });
      setLastCommand(result);
      if (!result.success) {
        setError(commandFailureMessage(result, "skills command failed"));
        return;
      }
      await refresh();
    });
  }

  if (isLoading || !workspace) {
    return (
      <Card>
        <CardContent className="p-4 text-sm text-muted-foreground">
          Loading skills workspace…
        </CardContent>
      </Card>
    );
  }

  const { state, detectedAgents, runtimeStatus } = workspace;
  const allAgents = uniqueAgents([
    ...KNOWN_AGENTS,
    ...detectedAgents,
    ...state.activeAgents,
  ]);

  return (
    <>
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div className="flex items-center gap-3">
          <span
            className={cn(
              "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs font-medium",
              runtimeStatus.available
                ? "border-border/70 bg-muted/40 text-muted-foreground"
                : "border-destructive/30 bg-destructive/10 text-destructive",
            )}
          >
            {runtimeStatus.available ? (
              <>
                <span className="size-1.5 rounded-full bg-emerald-500" />
                skills@{runtimeStatus.expectedVersion}
              </>
            ) : (
              "Runtime unavailable"
            )}
          </span>
        </div>

        <div className="inline-flex overflow-hidden rounded-lg border border-border/50 bg-muted/30 p-0.5">
          <button
            type="button"
            className={cn(
              "rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
              state.scope === "global"
                ? "bg-primary text-primary-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            onClick={() => void handleScopeChange("global")}
            disabled={busyAction !== null}
          >
            Global
          </button>
          <button
            type="button"
            className={cn(
              "rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
              state.scope === "project"
                ? "bg-primary text-primary-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            onClick={() => void handleScopeChange("project")}
            disabled={busyAction !== null}
          >
            Project
          </button>
        </div>
      </div>

      <section>
        <h2 className="mb-2 text-sm font-medium text-foreground">
          Active agents
        </h2>
        <p className="mb-3 text-xs text-muted-foreground">
          Skills commands target the agents you select here. Auto-detected from{" "}
          <code className="rounded bg-muted/40 px-1 py-0.5 text-[11px]">
            ~/.claude
          </code>{" "}
          and friends on first run.
        </p>
        <div className="flex flex-wrap gap-2">
          {allAgents.map((agent) => {
            const active = state.activeAgents.includes(agent);
            const detected = detectedAgents.includes(agent);
            return (
              <button
                key={agent}
                type="button"
                onClick={() => void handleToggleAgent(agent)}
                disabled={busyAction !== null}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs font-medium transition-colors",
                  active
                    ? "border-primary/55 bg-primary text-primary-foreground"
                    : "border-border/70 bg-muted/30 text-foreground hover:bg-accent/60",
                )}
              >
                {agent}
                {detected && !active ? (
                  <span className="text-[10px] opacity-70">detected</span>
                ) : null}
              </button>
            );
          })}
        </div>
      </section>

      {error ? (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <section>
        <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold text-foreground">Skills</h2>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {skills.length} {skills.length === 1 ? "skill" : "skills"}{" "}
              installed
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => void handleUpdateAll()}
              disabled={busyAction !== null || skills.length === 0}
            >
              {busyAction === "update" ? "Updating…" : "Update all"}
            </Button>
          </div>
        </div>

        <div className="mb-4 flex flex-wrap items-center gap-2">
          <input
            type="text"
            placeholder="owner/repo or package name"
            value={addSource}
            onChange={(e) => setAddSource(e.target.value)}
            className="h-[var(--control-height)] flex-1 min-w-[260px] rounded-md border border-border/60 bg-card px-3 text-sm text-foreground placeholder:text-muted-foreground/60 focus:border-ring focus:outline-none focus:ring-1 focus:ring-ring/50"
          />
          <Button
            size="sm"
            onClick={() => void handleAdd()}
            disabled={busyAction !== null || !addSource.trim()}
          >
            {busyAction === "add" ? "Adding…" : "Add skill"}
          </Button>
        </div>

        {skills.length === 0 ? (
          <div className="rounded-md border border-border/70 bg-card p-6 text-sm text-muted-foreground">
            No skills installed in this scope.
          </div>
        ) : (
          <div className="grid gap-3">
            {skills.map((skill) => {
              const isRemoving = pendingRemoval === skill.name;
              return (
                <div
                  key={`${skill.scope}:${skill.name}`}
                  className="flex flex-col gap-3 rounded-lg border border-border/60 bg-card p-4"
                >
                  <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)_auto_auto]">
                    <div className="min-w-0">
                      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground/70">
                        Name
                      </div>
                      <div className="truncate text-[15px] font-semibold text-foreground">
                        {skill.name}
                      </div>
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground/70">
                        Source
                      </div>
                      <div className="truncate text-sm text-foreground">
                        {skill.source ?? (
                          <span className="text-muted-foreground/70">—</span>
                        )}
                      </div>
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground/70">
                        Agents
                      </div>
                      <div className="flex flex-wrap gap-1">
                        {skill.agents.length === 0 ? (
                          <span className="text-sm text-muted-foreground/70">
                            —
                          </span>
                        ) : (
                          skill.agents.map((agent) => (
                            <span
                              key={agent}
                              className="rounded-sm border border-border/70 bg-muted/40 px-1.5 py-0.5 text-[11px]"
                            >
                              {agent}
                            </span>
                          ))
                        )}
                      </div>
                    </div>
                    <div>
                      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground/70">
                        Scope
                      </div>
                      <div className="text-sm text-foreground">
                        {skill.scope}
                      </div>
                    </div>
                    <div>
                      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground/70">
                        Version
                      </div>
                      <div className="text-sm text-foreground">
                        {skill.version ?? (
                          <span className="text-muted-foreground/70">—</span>
                        )}
                      </div>
                    </div>
                  </div>

                  {skill.description ? (
                    <p className="text-sm text-muted-foreground">
                      {skill.description}
                    </p>
                  ) : null}

                  <div className="flex flex-wrap gap-2">
                    <RemovalActions
                      isRemoving={isRemoving}
                      busyAction={busyAction}
                      onToggle={() =>
                        setPendingRemoval((cur) =>
                          cur === skill.name ? null : skill.name,
                        )
                      }
                      onCancel={() => setPendingRemoval(null)}
                      onConfirm={() => void handleRemove(skill.name)}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      <OutputPanel
        lastCommand={lastCommand}
        subtitle="Latest skills CLI command transcript"
        emptyMessage="Run add, remove, or update to capture a transcript here."
      />
    </>
  );
}
