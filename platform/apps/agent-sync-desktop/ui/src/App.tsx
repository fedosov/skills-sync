import { useCallback, useEffect, useState } from "react";
import { cn } from "./lib/utils";
import { DotagentsWorkspace } from "./components/workspaces/DotagentsWorkspace";
import { SkillsWorkspace } from "./components/workspaces/SkillsWorkspace";
import type { WorkspaceKey } from "./types";

const WORKSPACE_STORAGE_KEY = "dotagents-desktop:workspace";

function readStoredWorkspace(): WorkspaceKey {
  try {
    const raw = window.localStorage.getItem(WORKSPACE_STORAGE_KEY);
    if (raw === "skills" || raw === "dotagents") {
      return raw;
    }
  } catch {
    // localStorage may be unavailable (SSR, sandbox); fall through.
  }
  return "dotagents";
}

function writeStoredWorkspace(value: WorkspaceKey) {
  try {
    window.localStorage.setItem(WORKSPACE_STORAGE_KEY, value);
  } catch {
    // ignore write failures — switcher state still lives in React.
  }
}

export function App() {
  const [workspace, setWorkspace] = useState<WorkspaceKey>(() =>
    readStoredWorkspace(),
  );
  const [pending, setPending] = useState<WorkspaceKey | null>(null);

  useEffect(() => {
    writeStoredWorkspace(workspace);
  }, [workspace]);

  const handleSwitch = useCallback(
    (next: WorkspaceKey) => {
      if (pending !== null || next === workspace) {
        return;
      }
      setPending(next);
    },
    [pending, workspace],
  );

  const handleReady = useCallback(() => {
    if (pending === null) {
      return;
    }
    setWorkspace(pending);
    setPending(null);
  }, [pending]);

  const active = pending ?? workspace;
  const isLoading = pending !== null;

  return (
    <div className="min-h-full bg-background text-foreground">
      <div className="mx-auto flex min-h-full w-full max-w-[1380px] flex-col gap-4 px-4 py-5 md:px-6 md:py-6">
        <header className="sticky top-0 z-10 space-y-3 border-b border-border/50 bg-background/80 pb-4 backdrop-blur-xl">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <h1 className="text-2xl font-bold tracking-tight">
              Dotagents Desktop
            </h1>
            <nav
              aria-label="Workspace switcher"
              aria-busy={isLoading}
              className="inline-flex overflow-hidden rounded-lg border border-border/50 bg-muted/30 p-0.5"
            >
              <WorkspaceTab
                label="Dotagents"
                target="dotagents"
                active={active}
                pending={pending}
                disabled={isLoading}
                onSelect={handleSwitch}
              />
              <WorkspaceTab
                label="Skills"
                target="skills"
                active={active}
                pending={pending}
                disabled={isLoading}
                onSelect={handleSwitch}
              />
            </nav>
          </div>
        </header>

        <main
          className={cn(
            "flex flex-col gap-4 transition-opacity duration-150",
            isLoading && "pointer-events-none opacity-60",
          )}
          aria-busy={isLoading}
        >
          {active === "dotagents" ? (
            <DotagentsWorkspace key="dotagents" onReady={handleReady} />
          ) : (
            <SkillsWorkspace key="skills" onReady={handleReady} />
          )}
        </main>
      </div>
    </div>
  );
}

function WorkspaceTab({
  label,
  target,
  active,
  pending,
  disabled,
  onSelect,
}: {
  label: string;
  target: WorkspaceKey;
  active: WorkspaceKey;
  pending: WorkspaceKey | null;
  disabled: boolean;
  onSelect: (key: WorkspaceKey) => void;
}) {
  const isActive = active === target;
  const isPending = pending === target;
  return (
    <button
      type="button"
      onClick={() => onSelect(target)}
      disabled={disabled}
      aria-pressed={isActive}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
        isActive
          ? "bg-primary text-primary-foreground shadow-sm"
          : "text-muted-foreground hover:text-foreground",
        disabled && !isPending && "cursor-not-allowed opacity-60",
      )}
    >
      {isPending ? (
        <span
          aria-hidden
          className="size-3 animate-spin rounded-full border border-current border-t-transparent"
        />
      ) : null}
      {label}
    </button>
  );
}
