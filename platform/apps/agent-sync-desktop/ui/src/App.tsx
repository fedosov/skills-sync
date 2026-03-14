import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { Input } from "./components/ui/input";
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
  DotagentsCommandRequest,
  DotagentsCommandResult,
  DotagentsMcpListItem,
  DotagentsRuntimeStatus,
  DotagentsScope,
  DotagentsSkillListItem,
} from "./types";

const DOCS_URL = "https://www.npmjs.com/package/@sentry/dotagents/v/0.10.0";
const EXAMPLE_CONFIG = `version = 1
agents = ["claude", "codex"]
skills = []
mcp = []`;

type AppTab = "skills" | "mcp" | "output";
type SkillAddMode = "named" | "wildcard";
type McpMode = "stdio" | "http";

type SkillFormState = {
  source: string;
  mode: SkillAddMode;
  name: string;
};

type McpFormState = {
  mode: McpMode;
  name: string;
  command: string;
  args: string;
  url: string;
  headers: string;
  env: string;
};

const INITIAL_SKILL_FORM: SkillFormState = {
  source: "",
  mode: "named",
  name: "",
};

const INITIAL_MCP_FORM: McpFormState = {
  mode: "stdio",
  name: "",
  command: "",
  args: "",
  url: "",
  headers: "",
  env: "",
};

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

function splitLines(value: string): string[] {
  return value
    .split("\n")
    .map((entry) => entry.trim())
    .filter(Boolean);
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

function tabButtonClass(active: boolean): string {
  return cn(
    "border-b-2 px-1 pb-3 text-sm font-medium transition-colors",
    active
      ? "border-primary text-foreground"
      : "border-transparent text-muted-foreground hover:text-foreground",
  );
}

function statusTone(
  value: DotagentsSkillListItem["status"],
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

function RuntimeBanner({
  runtimeStatus,
}: {
  runtimeStatus: DotagentsRuntimeStatus | null;
}) {
  if (!runtimeStatus) {
    return null;
  }

  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 rounded-md border px-3 py-2 text-sm",
        runtimeStatus.available
          ? "border-border/70 bg-card"
          : "border-destructive/30 bg-destructive/10 text-destructive",
      )}
    >
      <span className="font-medium">
        {runtimeStatus.available
          ? "Bundled runtime ready"
          : "Bundled runtime unavailable"}
      </span>
      <span className="text-muted-foreground">
        expected {runtimeStatus.expectedVersion}
      </span>
      {runtimeStatus.actualVersion ? (
        <span className="text-muted-foreground">
          running {runtimeStatus.actualVersion}
        </span>
      ) : null}
      {runtimeStatus.binaryPath ? (
        <code className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">
          {runtimeStatus.binaryPath}
        </code>
      ) : null}
      {!runtimeStatus.available && runtimeStatus.error ? (
        <span>{runtimeStatus.error}</span>
      ) : null}
    </div>
  );
}

function FieldLabel({ label, hint }: { label: string; hint?: string }) {
  return (
    <div className="mb-1 flex items-center justify-between gap-3 text-xs">
      <label className="font-medium text-foreground">{label}</label>
      {hint ? <span className="text-muted-foreground">{hint}</span> : null}
    </div>
  );
}

function Textarea({
  value,
  onChange,
  placeholder,
  rows = 3,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  rows?: number;
}) {
  return (
    <textarea
      className="min-h-[96px] w-full rounded-sm border border-input bg-card px-2.5 py-2 text-[13px] text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      value={value}
      rows={rows}
      placeholder={placeholder}
      onChange={(event) => onChange(event.target.value)}
    />
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
  if (!lastCommand) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Last command transcript</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          Run any install, sync, add, update, or remove action to capture the
          latest transcript here.
        </CardContent>
      </Card>
    );
  }

  return (
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
  );
}

export function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("skills");
  const [runtimeStatus, setRuntimeStatus] =
    useState<DotagentsRuntimeStatus | null>(null);
  const [appContext, setAppContext] = useState<AppContext | null>(null);
  const [skills, setSkills] = useState<DotagentsSkillListItem[]>([]);
  const [mcpServers, setMcpServers] = useState<DotagentsMcpListItem[]>([]);
  const [lastCommand, setLastCommand] = useState<DotagentsCommandResult | null>(
    null,
  );
  const [skillForm, setSkillForm] =
    useState<SkillFormState>(INITIAL_SKILL_FORM);
  const [mcpForm, setMcpForm] = useState<McpFormState>(INITIAL_MCP_FORM);
  const [isLoading, setIsLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const currentScope = appContext?.activeProjectContext.mode ?? "user";
  const currentProjectRoot =
    appContext?.activeProjectContext.projectRoot ?? null;
  const ready = isReadyContext(appContext);
  const skillCount = skills.length;
  const mcpCount = mcpServers.length;

  const skillFormValid =
    skillForm.source.trim().length > 0 &&
    (skillForm.mode === "wildcard" || skillForm.name.trim().length > 0);
  const mcpFormValid =
    mcpForm.name.trim().length > 0 &&
    (mcpForm.mode === "stdio"
      ? mcpForm.command.trim().length > 0
      : mcpForm.url.trim().length > 0);

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
      setActiveTab(scope === "user" ? activeTab : "skills");
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

  async function handleCommand(
    request: DotagentsCommandRequest,
  ): Promise<boolean> {
    setBusyAction(request.kind);
    setError(null);
    try {
      const result = await runDotagentsCommand(request);
      setLastCommand(result);
      if (!result.success) {
        setActiveTab("output");
        setError(commandFailureMessage(result));
        return false;
      }
      await refreshApp();
      return true;
    } catch (commandError) {
      setError(errorMessage(commandError));
      return false;
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

  async function handleSkillSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!skillFormValid) {
      setError(
        "Provide a source and either an explicit --name or wildcard mode.",
      );
      return;
    }

    const succeeded = await handleCommand({
      kind: "skillAdd",
      source: skillForm.source.trim(),
      name: skillForm.mode === "named" ? skillForm.name.trim() : null,
      all: skillForm.mode === "wildcard",
    });

    if (succeeded) {
      setSkillForm(INITIAL_SKILL_FORM);
    }
  }

  async function handleMcpSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!mcpFormValid) {
      setError("Complete the MCP form before submitting.");
      return;
    }

    const env = splitLines(mcpForm.env);

    const request: DotagentsCommandRequest =
      mcpForm.mode === "stdio"
        ? {
            kind: "mcpAddStdio",
            name: mcpForm.name.trim(),
            command: mcpForm.command.trim(),
            args: splitLines(mcpForm.args),
            env,
          }
        : {
            kind: "mcpAddHttp",
            name: mcpForm.name.trim(),
            url: mcpForm.url.trim(),
            headers: splitLines(mcpForm.headers),
            env,
          };

    const succeeded = await handleCommand(request);

    if (succeeded) {
      setMcpForm(INITIAL_MCP_FORM);
    }
  }

  const contextMeta = useMemo(() => {
    if (!appContext) {
      return {
        scopeSummary: "Loading context",
        pathSummary: "",
      };
    }

    if (currentScope === "user") {
      return {
        scopeSummary: "User scope",
        pathSummary: appContext.userAgentsTomlPath,
      };
    }

    return {
      scopeSummary: "Project scope",
      pathSummary: currentProjectRoot ?? "No project folder selected",
    };
  }, [appContext, currentProjectRoot, currentScope]);

  function renderEmptyState() {
    if (!runtimeStatus?.available) {
      return (
        <EmptyState
          title="Bundled runtime required"
          message="Dotagents Desktop only runs against the bundled dotagents binary. Fix the packaged runtime first, then reload the app."
          actions={
            <a
              href={DOCS_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex h-[var(--control-height)] items-center rounded-sm border border-border/70 px-3 text-sm font-medium text-foreground hover:bg-accent/70"
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
                className="inline-flex h-[var(--control-height)] items-center rounded-sm border border-border/70 px-3 text-sm font-medium text-foreground hover:bg-accent/70"
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
                className="inline-flex h-[var(--control-height)] items-center rounded-sm border border-border/70 px-3 text-sm font-medium text-foreground hover:bg-accent/70"
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

  function renderSkillsTab() {
    if (!ready) {
      return renderEmptyState();
    }

    return (
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
        <Card>
          <CardHeader className="flex flex-row items-start justify-between gap-4">
            <div>
              <CardTitle>Skills</CardTitle>
              <p className="mt-1 text-sm text-muted-foreground">
                Vendor list output from <code>dotagents list --json</code>.
              </p>
            </div>
            <div className="text-sm text-muted-foreground">
              {skillCount} items
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                onClick={() =>
                  void handleCommand({ kind: "install", frozen: false })
                }
                disabled={busyAction !== null}
              >
                Install
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  void handleCommand({ kind: "install", frozen: true })
                }
                disabled={busyAction !== null}
              >
                Install --frozen
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void handleCommand({ kind: "sync" })}
                disabled={busyAction !== null}
              >
                Sync
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  void handleCommand({ kind: "skillUpdate", name: null })
                }
                disabled={busyAction !== null}
              >
                Update all
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void handleOpen(openAgentsToml)}
              >
                Open agents.toml
              </Button>
            </div>

            <div className="divide-y divide-border/60 rounded-md border border-border/70">
              {skills.length === 0 ? (
                <div className="p-4 text-sm text-muted-foreground">
                  No skills declared in this scope.
                </div>
              ) : (
                skills.map((skill) => (
                  <div
                    key={`${skill.name}:${skill.source}`}
                    className="flex flex-col gap-3 p-4 lg:flex-row lg:items-start lg:justify-between"
                  >
                    <div className="min-w-0 space-y-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <div className="font-medium text-foreground">
                          {skill.name}
                        </div>
                        <span
                          className={cn(
                            "inline-flex rounded-sm border px-2 py-1 text-[11px] font-medium",
                            toneClass(statusTone(skill.status)),
                          )}
                        >
                          {skill.status}
                        </span>
                        {skill.commit ? (
                          <code className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">
                            {skill.commit}
                          </code>
                        ) : null}
                      </div>
                      <div className="text-sm text-muted-foreground">
                        {skill.source}
                      </div>
                      {skill.wildcard ? (
                        <div className="text-xs text-muted-foreground">
                          Managed by wildcard source{" "}
                          <code>{skill.wildcard}</code>. Remove it by editing
                          agents.toml instead.
                        </div>
                      ) : null}
                    </div>

                    <div className="flex flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() =>
                          void handleCommand({
                            kind: "skillUpdate",
                            name: skill.name,
                          })
                        }
                        disabled={busyAction !== null}
                      >
                        Update
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() =>
                          void handleCommand({
                            kind: "skillRemove",
                            name: skill.name,
                          })
                        }
                        disabled={
                          busyAction !== null || Boolean(skill.wildcard)
                        }
                      >
                        Remove
                      </Button>
                    </div>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Add skill</CardTitle>
          </CardHeader>
          <CardContent>
            <form
              className="space-y-4"
              onSubmit={(event) => void handleSkillSubmit(event)}
            >
              <div>
                <FieldLabel label="Source" hint="required" />
                <Input
                  value={skillForm.source}
                  placeholder="owner/repo or git+https://example.com/repo.git"
                  onChange={(event) =>
                    setSkillForm((current) => ({
                      ...current,
                      source: event.target.value,
                    }))
                  }
                />
              </div>

              <div className="space-y-2">
                <FieldLabel label="Selection mode" />
                <div className="flex gap-2">
                  <button
                    type="button"
                    className={cn(
                      "rounded-sm border px-3 py-2 text-sm font-medium",
                      skillForm.mode === "named"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border/70 bg-card text-foreground hover:bg-accent/70",
                    )}
                    onClick={() =>
                      setSkillForm((current) => ({ ...current, mode: "named" }))
                    }
                  >
                    Explicit --name
                  </button>
                  <button
                    type="button"
                    className={cn(
                      "rounded-sm border px-3 py-2 text-sm font-medium",
                      skillForm.mode === "wildcard"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border/70 bg-card text-foreground hover:bg-accent/70",
                    )}
                    onClick={() =>
                      setSkillForm((current) => ({
                        ...current,
                        mode: "wildcard",
                      }))
                    }
                  >
                    Wildcard --all
                  </button>
                </div>
              </div>

              {skillForm.mode === "named" ? (
                <div>
                  <FieldLabel label="--name" hint="required in explicit mode" />
                  <Input
                    value={skillForm.name}
                    placeholder="skill-name"
                    onChange={(event) =>
                      setSkillForm((current) => ({
                        ...current,
                        name: event.target.value,
                      }))
                    }
                  />
                </div>
              ) : (
                <div className="rounded-md border border-border/70 bg-muted/30 p-3 text-sm text-muted-foreground">
                  Wildcard mode writes{" "}
                  <code>dotagents add &lt;source&gt; --all</code>.
                </div>
              )}

              <Button
                type="submit"
                disabled={!skillFormValid || busyAction !== null}
              >
                Add skill
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  }

  function renderMcpTab() {
    if (!ready) {
      return renderEmptyState();
    }

    return (
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
        <Card>
          <CardHeader className="flex flex-row items-start justify-between gap-4">
            <div>
              <CardTitle>MCP servers</CardTitle>
              <p className="mt-1 text-sm text-muted-foreground">
                Vendor list output from <code>dotagents mcp list --json</code>.
              </p>
            </div>
            <div className="text-sm text-muted-foreground">
              {mcpCount} items
            </div>
          </CardHeader>
          <CardContent>
            <div className="divide-y divide-border/60 rounded-md border border-border/70">
              {mcpServers.length === 0 ? (
                <div className="p-4 text-sm text-muted-foreground">
                  No MCP servers declared in this scope.
                </div>
              ) : (
                mcpServers.map((server) => (
                  <div
                    key={`${server.name}:${server.target}`}
                    className="flex flex-col gap-3 p-4 lg:flex-row lg:items-start lg:justify-between"
                  >
                    <div className="space-y-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <div className="font-medium text-foreground">
                          {server.name}
                        </div>
                        <span className="inline-flex rounded-sm border border-border/70 bg-muted/40 px-2 py-1 text-[11px] font-medium">
                          {server.transport}
                        </span>
                      </div>
                      <code className="block rounded bg-muted/60 px-2 py-1 font-mono text-[12px] text-foreground">
                        {server.target}
                      </code>
                      {server.env.length > 0 ? (
                        <div className="text-xs text-muted-foreground">
                          env: {server.env.join(", ")}
                        </div>
                      ) : null}
                    </div>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() =>
                          void handleCommand({
                            kind: "mcpRemove",
                            name: server.name,
                          })
                        }
                        disabled={busyAction !== null}
                      >
                        Remove
                      </Button>
                    </div>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Add MCP server</CardTitle>
          </CardHeader>
          <CardContent>
            <form
              className="space-y-4"
              onSubmit={(event) => void handleMcpSubmit(event)}
            >
              <div className="space-y-2">
                <FieldLabel label="Transport" />
                <div className="flex gap-2">
                  <button
                    type="button"
                    className={cn(
                      "rounded-sm border px-3 py-2 text-sm font-medium",
                      mcpForm.mode === "stdio"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border/70 bg-card text-foreground hover:bg-accent/70",
                    )}
                    onClick={() =>
                      setMcpForm((current) => ({ ...current, mode: "stdio" }))
                    }
                  >
                    --command
                  </button>
                  <button
                    type="button"
                    className={cn(
                      "rounded-sm border px-3 py-2 text-sm font-medium",
                      mcpForm.mode === "http"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border/70 bg-card text-foreground hover:bg-accent/70",
                    )}
                    onClick={() =>
                      setMcpForm((current) => ({ ...current, mode: "http" }))
                    }
                  >
                    --url
                  </button>
                </div>
              </div>

              <div>
                <FieldLabel label="Name" hint="required" />
                <Input
                  value={mcpForm.name}
                  placeholder="github"
                  onChange={(event) =>
                    setMcpForm((current) => ({
                      ...current,
                      name: event.target.value,
                    }))
                  }
                />
              </div>

              {mcpForm.mode === "stdio" ? (
                <>
                  <div>
                    <FieldLabel label="--command" hint="required" />
                    <Input
                      value={mcpForm.command}
                      placeholder="npx"
                      onChange={(event) =>
                        setMcpForm((current) => ({
                          ...current,
                          command: event.target.value,
                        }))
                      }
                    />
                  </div>
                  <div>
                    <FieldLabel label="--args" hint="one per line" />
                    <Textarea
                      value={mcpForm.args}
                      placeholder="-y&#10;@modelcontextprotocol/server-github"
                      onChange={(value) =>
                        setMcpForm((current) => ({ ...current, args: value }))
                      }
                    />
                  </div>
                </>
              ) : (
                <>
                  <div>
                    <FieldLabel label="--url" hint="required" />
                    <Input
                      value={mcpForm.url}
                      placeholder="https://mcp.example.com/sse"
                      onChange={(event) =>
                        setMcpForm((current) => ({
                          ...current,
                          url: event.target.value,
                        }))
                      }
                    />
                  </div>
                  <div>
                    <FieldLabel label="--header" hint="Key:Value per line" />
                    <Textarea
                      value={mcpForm.headers}
                      placeholder="Authorization:Bearer token"
                      onChange={(value) =>
                        setMcpForm((current) => ({
                          ...current,
                          headers: value,
                        }))
                      }
                    />
                  </div>
                </>
              )}

              <div>
                <FieldLabel label="--env" hint="variable names, one per line" />
                <Textarea
                  value={mcpForm.env}
                  placeholder="GITHUB_TOKEN"
                  onChange={(value) =>
                    setMcpForm((current) => ({ ...current, env: value }))
                  }
                />
              </div>

              <Button
                type="submit"
                disabled={!mcpFormValid || busyAction !== null}
              >
                Add MCP server
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-background text-foreground">
      <div className="mx-auto flex min-h-full w-full max-w-[1380px] flex-col gap-4 px-4 py-5 md:px-6 md:py-6">
        <header className="flex flex-col gap-4 border-b border-border/70 pb-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="space-y-1">
            <h1 className="text-[24px] font-semibold tracking-tight">
              Dotagents Desktop
            </h1>
            <p className="max-w-[72ch] text-sm text-muted-foreground">
              Desktop control plane for the bundled dotagents 0.10.0 runtime. No
              synthetic catalog, no custom sync engine, just the vendor CLI
              surfaced with explicit project and user contexts.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={cn(
                "rounded-sm border px-3 py-2 text-sm font-medium",
                currentScope === "project"
                  ? "border-primary bg-primary text-primary-foreground"
                  : "border-border/70 bg-card text-foreground hover:bg-accent/70",
              )}
              onClick={() => void handleScopeChange("project")}
              disabled={busyAction !== null}
            >
              Project
            </button>
            <button
              type="button"
              className={cn(
                "rounded-sm border px-3 py-2 text-sm font-medium",
                currentScope === "user"
                  ? "border-primary bg-primary text-primary-foreground"
                  : "border-border/70 bg-card text-foreground hover:bg-accent/70",
              )}
              onClick={() => void handleScopeChange("user")}
              disabled={busyAction !== null}
            >
              User
            </button>
          </div>
        </header>

        <RuntimeBanner runtimeStatus={runtimeStatus} />

        <div className="flex flex-col gap-3 rounded-md border border-border/70 bg-card px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="space-y-1">
            <div className="text-sm font-medium text-foreground">
              {contextMeta.scopeSummary}
            </div>
            <div className="text-sm text-muted-foreground">
              {contextMeta.pathSummary}
            </div>
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

        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        ) : null}

        <nav className="border-b border-border/70">
          <div className="flex gap-6">
            <button
              type="button"
              className={tabButtonClass(activeTab === "skills")}
              onClick={() => setActiveTab("skills")}
            >
              Skills
            </button>
            <button
              type="button"
              className={tabButtonClass(activeTab === "mcp")}
              onClick={() => setActiveTab("mcp")}
            >
              MCP
            </button>
            <button
              type="button"
              className={tabButtonClass(activeTab === "output")}
              onClick={() => setActiveTab("output")}
            >
              Output
            </button>
          </div>
        </nav>

        {isLoading ? (
          <Card>
            <CardContent className="p-4 text-sm text-muted-foreground">
              Loading dotagents runtime and active context…
            </CardContent>
          </Card>
        ) : activeTab === "skills" ? (
          renderSkillsTab()
        ) : activeTab === "mcp" ? (
          renderMcpTab()
        ) : (
          <OutputPanel lastCommand={lastCommand} />
        )}
      </div>
    </div>
  );
}
