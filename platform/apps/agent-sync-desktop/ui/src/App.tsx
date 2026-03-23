import { useCallback, useEffect, useState, type ReactNode } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { cn, errorMessage } from "./lib/utils";
import {
  getAppContext,
  getRuntimeStatus,
  listMcpServers,
  listSkills,
  openAgentsDir,
  openAgentsToml,
  openUserHome,
  runDotagentsCommand,
  setProjectRoot,
  setScope,
} from "./tauriApi";
import type {
  AppContext,
  DotagentsCommandResult,
  DotagentsMcpListItem,
  DotagentsRuntimeStatus,
  DotagentsScope,
  DotagentsSkillListItem,
  DotagentsSkillStatus,
} from "./types";

const DOCS_URL = "https://dotagents.sentry.dev/cli";
const DOCS_LINK_CLASS =
  "inline-flex h-[var(--control-height)] items-center rounded-sm border border-border/70 px-3 text-sm font-medium text-foreground hover:bg-accent/70";
const EXAMPLE_CONFIG = `version = 1
agents = ["claude", "codex"]
skills = []
mcp = []`;

function isReadyContext(context: AppContext | null): boolean {
  if (!context) {
    return false;
  }

  if (context.activeProjectContext.mode === "user") {
    return context.userInitialized;
  }

  return Boolean(
    context.activeProjectContext.projectRoot && context.projectInitialized,
  );
}

function firstDialogPath(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }

  if (Array.isArray(value) && typeof value[0] === "string") {
    return value[0];
  }

  return null;
}

function commandFailureMessage(result: DotagentsCommandResult): string {
  if (result.stderr.trim()) {
    return result.stderr;
  }
  if (result.stdout.trim()) {
    return result.stdout;
  }
  return "dotagents command failed";
}

function formatDuration(durationMs: number): string {
  if (durationMs < 1000) {
    return `${durationMs} ms`;
  }
  return `${(durationMs / 1000).toFixed(2)} s`;
}

function statusTone(
  value: DotagentsSkillStatus,
): "neutral" | "warning" | "danger" {
  switch (value) {
    case "ok":
      return "neutral";
    case "modified":
    case "unlocked":
      return "warning";
    case "missing":
      return "danger";
  }
}

function toneClass(tone: "neutral" | "warning" | "danger"): string {
  switch (tone) {
    case "neutral":
      return "border-border/70 bg-muted/40 text-foreground";
    case "warning":
      return "border-amber-600/25 bg-amber-500/10 text-amber-800 dark:text-amber-300";
    case "danger":
      return "border-destructive/25 bg-destructive/10 text-destructive";
  }
}

function statusHint(status: DotagentsSkillStatus): string | null {
  switch (status) {
    case "ok":
      return null;
    case "modified":
      return "Local changes — sync will reset to declared state";
    case "missing":
      return "Not installed — sync will install";
    case "unlocked":
      return "Not pinned — sync will lock to a commit";
  }
}

function RemovalActions({
  isRemoving,
  busyAction,
  extraDisabled = false,
  onToggle,
  onCancel,
  onConfirm,
}: {
  isRemoving: boolean;
  busyAction: string | null;
  extraDisabled?: boolean;
  onToggle: () => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  if (isRemoving) {
    return (
      <>
        <Button
          size="sm"
          variant="outline"
          onClick={onCancel}
          disabled={busyAction !== null}
        >
          Cancel
        </Button>
        <Button
          size="sm"
          variant="destructive"
          onClick={onConfirm}
          disabled={busyAction !== null}
        >
          Confirm remove
        </Button>
      </>
    );
  }
  return (
    <Button
      size="sm"
      variant="ghost"
      className="text-muted-foreground/60 hover:text-destructive"
      onClick={onToggle}
      disabled={busyAction !== null || extraDisabled}
    >
      Remove
    </Button>
  );
}

function EmptyState({
  title,
  message,
  actions,
}: {
  title: string;
  message: string;
  actions: ReactNode;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4 text-sm">
        <p className="max-w-[70ch] text-muted-foreground">{message}</p>
        <div className="flex flex-wrap gap-2">{actions}</div>
        <div className="rounded-md border border-border/70 bg-muted/30 p-3">
          <div className="mb-2 text-xs font-medium text-foreground">
            Minimal vendor config
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-[12px] text-muted-foreground">
            {EXAMPLE_CONFIG}
          </pre>
        </div>
      </CardContent>
    </Card>
  );
}

function OutputPanel({
  lastCommand,
}: {
  lastCommand: DotagentsCommandResult | null;
}) {
  return (
    <section>
      <div className="mb-4">
        <h2 className="text-xl font-semibold text-foreground">Output</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Latest vendor command transcript
        </p>
      </div>

      {!lastCommand ? (
        <div className="rounded-md border border-border/70 bg-card p-6 text-sm text-muted-foreground">
          Run sync or remove to capture a transcript here.
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[minmax(0,320px)_minmax(0,1fr)]">
          <Card>
            <CardHeader>
              <CardTitle>Transcript</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm">
              <div>
                <div className="mb-1 text-xs font-medium text-muted-foreground">
                  Command
                </div>
                <code className="block rounded-md border border-border/70 bg-muted/30 px-2.5 py-2 font-mono text-[12px]">
                  {lastCommand.command}
                </code>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Scope
                  </div>
                  <div className="text-foreground">{lastCommand.scope}</div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Exit code
                  </div>
                  <div className="text-foreground">
                    {lastCommand.exitCode ?? "not available"}
                  </div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Duration
                  </div>
                  <div className="text-foreground">
                    {formatDuration(lastCommand.durationMs)}
                  </div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Status
                  </div>
                  <div
                    className={cn(
                      "inline-flex rounded-sm border px-2 py-1 text-xs font-medium",
                      lastCommand.success
                        ? toneClass("neutral")
                        : toneClass("danger"),
                    )}
                  >
                    {lastCommand.success ? "success" : "failed"}
                  </div>
                </div>
              </div>
              <div>
                <div className="mb-1 text-xs font-medium text-muted-foreground">
                  Working directory
                </div>
                <code className="block rounded-md border border-border/70 bg-muted/30 px-2.5 py-2 font-mono text-[12px]">
                  {lastCommand.cwd}
                </code>
              </div>
            </CardContent>
          </Card>

          <div className="grid gap-4">
            <Card>
              <CardHeader>
                <CardTitle>stdout</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="min-h-[180px] overflow-x-auto whitespace-pre-wrap rounded-md border border-border/70 bg-muted/30 p-3 font-mono text-[12px] text-foreground">
                  {lastCommand.stdout || "No stdout output."}
                </pre>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>stderr</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="min-h-[180px] overflow-x-auto whitespace-pre-wrap rounded-md border border-border/70 bg-muted/30 p-3 font-mono text-[12px] text-foreground">
                  {lastCommand.stderr || "No stderr output."}
                </pre>
              </CardContent>
            </Card>
          </div>
        </div>
      )}
    </section>
  );
}

function SectionActions({
  syncNeeded,
  busyAction,
  onSync,
  onOpenAgentsToml,
}: {
  syncNeeded: boolean;
  busyAction: string | null;
  onSync: () => Promise<void>;
  onOpenAgentsToml: () => Promise<void>;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {!syncNeeded && (
        <span className="inline-flex items-center gap-1.5 text-xs font-medium text-emerald-600 dark:text-emerald-400">
          <svg className="size-3.5" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-1.06-1.06L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.06 1.06l2 2a.75.75 0 0 0 1.06 0l4.5-4.5Z" />
          </svg>
          All synced
        </span>
      )}
      <Button
        size="sm"
        onClick={() => void onSync()}
        disabled={!syncNeeded || busyAction !== null}
      >
        {busyAction === "sync" ? "Syncing…" : "Sync"}
      </Button>
      <Button
        size="sm"
        variant="outline"
        onClick={() => void onOpenAgentsToml()}
      >
        Open agents.toml
      </Button>
    </div>
  );
}

export function App() {
  const [runtimeStatus, setRuntimeStatus] =
    useState<DotagentsRuntimeStatus | null>(null);
  const [appContext, setAppContext] = useState<AppContext | null>(null);
  const [skills, setSkills] = useState<DotagentsSkillListItem[]>([]);
  const [mcpServers, setMcpServers] = useState<DotagentsMcpListItem[]>([]);
  const [lastCommand, setLastCommand] = useState<DotagentsCommandResult | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<{
    kind: "skill" | "mcp";
    name: string;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [skillFilter, setSkillFilter] = useState("");
  const currentScope = appContext?.activeProjectContext.mode ?? "user";
  const currentProjectRoot =
    appContext?.activeProjectContext.projectRoot ?? null;
  const ready = isReadyContext(appContext);
  const needsSync = skills.some((s) => s.status !== "ok");
  const filteredSkills = skillFilter
    ? skills.filter(
        (s) =>
          s.name.toLowerCase().includes(skillFilter.toLowerCase()) ||
          s.description?.toLowerCase().includes(skillFilter.toLowerCase()),
      )
    : skills;

  const refreshApp = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const [nextRuntimeStatus, nextContext] = await Promise.all([
        getRuntimeStatus(),
        getAppContext(),
      ]);
      setRuntimeStatus(nextRuntimeStatus);
      setAppContext(nextContext);

      if (!nextRuntimeStatus.available || !isReadyContext(nextContext)) {
        setSkills([]);
        setMcpServers([]);
        return;
      }

      const [nextSkills, nextMcp] = await Promise.all([
        listSkills(),
        listMcpServers(),
      ]);

      setSkills(nextSkills);
      setMcpServers(nextMcp);
      setPendingRemoval(null);
    } catch (refreshError) {
      setError(errorMessage(refreshError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshApp();
  }, [refreshApp]);

  async function handleScopeChange(scope: DotagentsScope) {
    setBusyAction(`scope:${scope}`);
    setError(null);
    try {
      const nextContext = await setScope(scope);
      setAppContext(nextContext);
      await refreshApp();
    } catch (scopeError) {
      setError(errorMessage(scopeError));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleChooseProjectRoot() {
    setBusyAction("projectRoot");
    setError(null);
    try {
      const selected: unknown = await openDirectoryDialog({
        directory: true,
        multiple: false,
      });
      const pickedPath = firstDialogPath(selected);
      if (!pickedPath) {
        return;
      }
      await setProjectRoot(pickedPath);
      await refreshApp();
    } catch (pickerError) {
      setError(errorMessage(pickerError));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleClearProjectRoot() {
    setBusyAction("clearProjectRoot");
    setError(null);
    try {
      await setProjectRoot(null);
      await refreshApp();
    } catch (clearError) {
      setError(errorMessage(clearError));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleSync() {
    setBusyAction("sync");
    setError(null);
    try {
      const result = await runDotagentsCommand({ kind: "sync" });
      setLastCommand(result);
      if (!result.success) {
        setError(commandFailureMessage(result));
        return;
      }
      await refreshApp();
    } catch (commandError) {
      setError(errorMessage(commandError));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleOpen(fn: () => Promise<void>) {
    try {
      await fn();
    } catch (openError) {
      setError(errorMessage(openError));
    }
  }

  async function handleRemove(kind: "skill" | "mcp", name: string) {
    setBusyAction(`${kind}:remove`);
    setError(null);
    try {
      const result = await runDotagentsCommand(
        kind === "skill"
          ? { kind: "skillRemove", name }
          : { kind: "mcpRemove", name },
      );
      setLastCommand(result);
      if (!result.success) {
        setError(commandFailureMessage(result));
        return;
      }
      await refreshApp();
    } catch (commandError) {
      setError(errorMessage(commandError));
    } finally {
      setBusyAction(null);
      setPendingRemoval(null);
    }
  }

  function toggleRemoval(kind: "skill" | "mcp", name: string) {
    setPendingRemoval((current) =>
      current?.kind === kind && current.name === name ? null : { kind, name },
    );
  }

  const contextMeta = !appContext
    ? {
        scopeSummary: "Loading context",
        pathSummary: "",
      }
    : currentScope === "user"
      ? {
          scopeSummary: "User scope",
          pathSummary: appContext.userAgentsTomlPath,
        }
      : {
          scopeSummary: "Project scope",
          pathSummary: currentProjectRoot ?? "No project folder selected",
        };

  function renderEmptyState() {
    if (!runtimeStatus?.available) {
      return (
        <EmptyState
          title="Runtime unavailable"
          message="Dotagents Desktop requires npx to run @sentry/dotagents. Install Node.js and npm, then reload the app."
          actions={
            <a
              href={DOCS_URL}
              target="_blank"
              rel="noreferrer"
              className={DOCS_LINK_CLASS}
            >
              View pinned docs
            </a>
          }
        />
      );
    }

    if (currentScope === "project" && !currentProjectRoot) {
      return (
        <EmptyState
          title="Choose a project folder"
          message="Project scope is explicit in Dotagents Desktop. Pick the folder that owns the agents.toml you want to operate on."
          actions={
            <>
              <Button onClick={() => void handleChooseProjectRoot()}>
                Choose project folder
              </Button>
              <a
                href={DOCS_URL}
                target="_blank"
                rel="noreferrer"
                className={DOCS_LINK_CLASS}
              >
                Open docs and examples
              </a>
            </>
          }
        />
      );
    }

    if (
      currentScope === "project" &&
      appContext?.projectInitialized === false
    ) {
      return (
        <EmptyState
          title="Selected folder is not initialized"
          message={`No agents.toml was found in ${currentProjectRoot}. Dotagents Desktop will not guess vendor defaults for init in v1, so initialize it manually or switch back to user scope.`}
          actions={
            <>
              <Button
                variant="outline"
                onClick={() => void handleOpen(openAgentsDir)}
              >
                Open folder
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleScopeChange("user")}
              >
                Switch to user scope
              </Button>
            </>
          }
        />
      );
    }

    if (currentScope === "user" && appContext && !appContext.userInitialized) {
      return (
        <EmptyState
          title="User scope is not initialized"
          message={`No user agents.toml was found at ${appContext.userAgentsTomlPath}. Initialize dotagents manually, then return here to manage skills and MCP entries.`}
          actions={
            <>
              <Button
                variant="outline"
                onClick={() => void handleOpen(openAgentsDir)}
              >
                Open ~/.agents
              </Button>
              <a
                href={DOCS_URL}
                target="_blank"
                rel="noreferrer"
                className={DOCS_LINK_CLASS}
              >
                Open docs and examples
              </a>
            </>
          }
        />
      );
    }

    return null;
  }

  function renderContent() {
    if (!ready) {
      return renderEmptyState();
    }

    return (
      <div className="space-y-8">
        {/* Skills section */}
        <section>
          <div className="mb-4 flex flex-col gap-3">
            <div className="flex flex-wrap items-end justify-between gap-4">
              <div>
                <h2 className="text-xl font-semibold text-foreground">
                  Skills
                </h2>
                <p className="mt-0.5 text-sm text-muted-foreground">
                  {skillFilter
                    ? `${filteredSkills.length} of ${skills.length} skills`
                    : `${skills.length} ${skills.length === 1 ? "skill" : "skills"} installed`}
                </p>
              </div>
              <SectionActions
                syncNeeded={needsSync}
                busyAction={busyAction}
                onSync={handleSync}
                onOpenAgentsToml={openAgentsToml}
              />
            </div>

            {skills.length > 5 && (
              <div className="relative">
                <svg
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground/50"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M10.68 11.74a6 6 0 0 1-7.922-8.982 6 6 0 0 1 8.982 7.922l3.04 3.04a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215ZM11.5 7a4.499 4.499 0 1 0-8.997 0A4.499 4.499 0 0 0 11.5 7Z" />
                </svg>
                <input
                  type="text"
                  placeholder="Filter by name or description…"
                  value={skillFilter}
                  onChange={(e) => setSkillFilter(e.target.value)}
                  className="h-[var(--control-height)] w-full rounded-lg border border-border/50 bg-card pl-9 pr-3.5 text-sm text-foreground placeholder:text-muted-foreground/50 focus:border-ring focus:outline-none focus:ring-1 focus:ring-ring/50 transition-colors duration-150"
                />
                {skillFilter && (
                  <button
                    type="button"
                    onClick={() => setSkillFilter("")}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground/50 hover:text-foreground transition-colors"
                  >
                    <svg
                      className="size-4"
                      viewBox="0 0 16 16"
                      fill="currentColor"
                    >
                      <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
                    </svg>
                  </button>
                )}
              </div>
            )}
          </div>

          {skills.length === 0 ? (
            <div className="rounded-md border border-border/70 bg-card p-6 text-sm text-muted-foreground">
              No skills declared in this scope.
            </div>
          ) : (
            <>
              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
                {filteredSkills.map((skill, index) => {
                  const hint = statusHint(skill.status);
                  const tone = statusTone(skill.status);
                  const isRemoving =
                    pendingRemoval?.kind === "skill" &&
                    pendingRemoval.name === skill.name;
                  return (
                    <div
                      key={`${skill.name}:${skill.source}`}
                      style={{
                        animationDelay: `${Math.min(index * 30, 300)}ms`,
                      }}
                      className={cn(
                        "item-card group flex flex-col gap-2 rounded-lg border bg-card p-4",
                        tone === "neutral"
                          ? "border-border/70"
                          : tone === "warning"
                            ? "border-amber-600/40"
                            : "border-destructive/40",
                      )}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <h3 className="min-w-0 text-[15px] font-semibold tracking-tight text-foreground">
                          {skill.name}
                        </h3>
                        {tone !== "neutral" && (
                          <span
                            className={cn(
                              "mt-0.5 shrink-0 rounded-sm border px-2 py-0.5 text-xs font-medium",
                              toneClass(tone),
                            )}
                          >
                            {skill.status}
                          </span>
                        )}
                      </div>

                      {skill.description ? (
                        <p className="line-clamp-2 text-[13px] leading-relaxed text-muted-foreground">
                          {skill.description}
                        </p>
                      ) : null}

                      {hint ? (
                        <div
                          className={cn(
                            "rounded-sm border px-2 py-1.5 text-xs",
                            toneClass(tone),
                          )}
                        >
                          {hint}
                        </div>
                      ) : null}

                      {skill.wildcard ? (
                        <div className="text-xs text-muted-foreground">
                          wildcard <code>{skill.wildcard}</code>
                        </div>
                      ) : null}

                      <div
                        className={cn(
                          "mt-auto flex flex-wrap gap-2 pt-0.5",
                          !isRemoving && "item-remove",
                        )}
                      >
                        <RemovalActions
                          isRemoving={isRemoving}
                          busyAction={busyAction}
                          extraDisabled={Boolean(skill.wildcard)}
                          onToggle={() => toggleRemoval("skill", skill.name)}
                          onCancel={() => setPendingRemoval(null)}
                          onConfirm={() =>
                            void handleRemove("skill", skill.name)
                          }
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
              {skillFilter && filteredSkills.length === 0 && (
                <div className="rounded-md border border-border/70 bg-card p-6 text-center text-sm text-muted-foreground">
                  No skills match &ldquo;{skillFilter}&rdquo;
                </div>
              )}
            </>
          )}
        </section>

        {/* MCP Servers section */}
        <section>
          <div className="mb-4 flex flex-wrap items-end justify-between gap-4">
            <div>
              <h2 className="text-xl font-semibold text-foreground">
                MCP Servers
              </h2>
              <p className="mt-0.5 text-sm text-muted-foreground">
                {mcpServers.length}{" "}
                {mcpServers.length === 1 ? "server" : "servers"} declared
              </p>
            </div>
            <SectionActions
              syncNeeded={true}
              busyAction={busyAction}
              onSync={handleSync}
              onOpenAgentsToml={openAgentsToml}
            />
          </div>

          {mcpServers.length === 0 ? (
            <div className="rounded-md border border-border/70 bg-card p-6 text-sm text-muted-foreground">
              No MCP servers declared in this scope.
            </div>
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
              {mcpServers.map((server) => {
                const isRemoving =
                  pendingRemoval?.kind === "mcp" &&
                  pendingRemoval.name === server.name;
                return (
                  <div
                    key={`${server.name}:${server.target}`}
                    className="item-card group flex flex-col gap-2 rounded-lg border border-border/60 bg-card p-4"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <h3 className="min-w-0 text-[15px] font-semibold tracking-tight text-foreground">
                        {server.name}
                      </h3>
                      <span className="mt-0.5 shrink-0 rounded-sm border border-border/70 bg-muted/40 px-2 py-0.5 text-xs font-medium">
                        {server.transport}
                      </span>
                    </div>

                    {server.description ? (
                      <p className="line-clamp-2 text-sm text-muted-foreground">
                        {server.description}
                      </p>
                    ) : null}

                    <code className="truncate rounded bg-muted/40 px-2 py-1 font-mono text-[13px] text-foreground">
                      {server.target}
                    </code>

                    {server.env.length > 0 ? (
                      <div className="flex flex-wrap gap-1.5">
                        {server.env.map((envVar) => (
                          <code
                            key={envVar}
                            className="rounded bg-muted/50 px-1.5 py-0.5 text-xs text-muted-foreground"
                          >
                            {envVar}
                          </code>
                        ))}
                      </div>
                    ) : null}

                    <div
                      className={cn(
                        "mt-auto flex flex-wrap gap-2 pt-0.5",
                        !isRemoving && "item-remove",
                      )}
                    >
                      <RemovalActions
                        isRemoving={isRemoving}
                        busyAction={busyAction}
                        onToggle={() => toggleRemoval("mcp", server.name)}
                        onCancel={() => setPendingRemoval(null)}
                        onConfirm={() => void handleRemove("mcp", server.name)}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <OutputPanel lastCommand={lastCommand} />
      </div>
    );
  }

  return (
    <div className="min-h-full bg-background text-foreground">
      <div className="mx-auto flex min-h-full w-full max-w-[1380px] flex-col gap-4 px-4 py-5 md:px-6 md:py-6">
        <header className="sticky top-0 z-10 space-y-3 border-b border-border/50 bg-background/80 pb-4 backdrop-blur-xl">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold tracking-tight">
                Dotagents Desktop
              </h1>
              {runtimeStatus && (
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
                      <span className="size-1.5 rounded-full bg-emerald-500" />v
                      {runtimeStatus.expectedVersion}
                    </>
                  ) : (
                    "Runtime unavailable"
                  )}
                </span>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <div className="inline-flex overflow-hidden rounded-lg border border-border/50 bg-muted/30 p-0.5">
                <button
                  type="button"
                  className={cn(
                    "rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
                    currentScope === "project"
                      ? "bg-primary text-primary-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  onClick={() => void handleScopeChange("project")}
                  disabled={busyAction !== null}
                >
                  Project
                </button>
                <button
                  type="button"
                  className={cn(
                    "rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
                    currentScope === "user"
                      ? "bg-primary text-primary-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  onClick={() => void handleScopeChange("user")}
                  disabled={busyAction !== null}
                >
                  User
                </button>
              </div>
            </div>
          </div>

          <div className="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="font-medium text-foreground">
                {contextMeta.scopeSummary}
              </span>
              <span className="hidden text-border lg:inline">/</span>
              <span className="truncate">{contextMeta.pathSummary}</span>
            </div>

            <div className="flex flex-wrap gap-2">
              {currentScope === "project" ? (
                <>
                  <Button
                    size="sm"
                    onClick={() => void handleChooseProjectRoot()}
                    disabled={busyAction !== null}
                  >
                    Choose project folder
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleOpen(openAgentsDir)}
                    disabled={!currentProjectRoot}
                  >
                    Open folder
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleClearProjectRoot()}
                    disabled={!currentProjectRoot || busyAction !== null}
                  >
                    Clear
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleOpen(openAgentsDir)}
                  >
                    Open ~/.agents
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleOpen(openUserHome)}
                  >
                    Open home
                  </Button>
                </>
              )}
            </div>
          </div>
        </header>

        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        ) : null}

        {isLoading ? (
          <Card>
            <CardContent className="p-4 text-sm text-muted-foreground">
              Loading dotagents runtime and active context…
            </CardContent>
          </Card>
        ) : (
          renderContent()
        )}
      </div>
    </div>
  );
}
