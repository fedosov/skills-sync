import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { cn } from "../../lib/utils";
import type { RefreshIntervalMinutes, SyncHealthStatus } from "../../types";

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

type AppHeaderProps = {
  syncStatus: SyncHealthStatus | undefined;
  activeSkillCount: number;
  archivedSkillCount: number;
  totalSkills: number;
  activeSubagentCount: number;
  mcpCount: number;
  query: string;
  onQueryChange: (value: string) => void;
  busy: boolean;
  allowFilesystemChanges: boolean;
  onAllowFilesystemChangesToggle: () => void;
  onSync: () => void;
  onOpenAuditLog: () => void;
  refreshIntervalMinutes: RefreshIntervalMinutes;
  onRefreshIntervalChange: (value: RefreshIntervalMinutes) => void;
};

export function AppHeader({
  syncStatus,
  activeSkillCount,
  archivedSkillCount,
  totalSkills,
  activeSubagentCount,
  mcpCount,
  query,
  onQueryChange,
  busy,
  allowFilesystemChanges,
  onAllowFilesystemChangesToggle,
  onSync,
  onOpenAuditLog,
  refreshIntervalMinutes,
  onRefreshIntervalChange,
}: AppHeaderProps) {
  return (
    <header className="shrink-0 border-b border-border/60 px-1 pb-3">
      <div className="flex flex-wrap items-start justify-between gap-2.5">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h1 className="text-base font-semibold tracking-tight text-dense">
              Agent Sync
            </h1>
            <Badge variant={syncStatusVariant(syncStatus)}>
              {toTitleCase(syncStatus ?? "unknown")}
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            Active {activeSkillCount} · Archived {archivedSkillCount} · Skills{" "}
            {totalSkills} · Subagents {activeSubagentCount} · MCP {mcpCount}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="inline-flex items-center gap-1 text-[11px] text-muted-foreground">
            <span>Auto</span>
            <select
              aria-label="Auto refresh interval"
              className="rounded-md border border-border/70 bg-background px-1.5 py-1 text-[11px]"
              value={String(refreshIntervalMinutes)}
              onChange={(event) => {
                const value = Number(event.currentTarget.value);
                if (
                  value === 0 ||
                  value === 5 ||
                  value === 15 ||
                  value === 30
                ) {
                  onRefreshIntervalChange(value);
                }
              }}
            >
              <option value="0">Manual</option>
              <option value="5">5m</option>
              <option value="15">15m</option>
              <option value="30">30m</option>
            </select>
          </label>

          <Button
            size="sm"
            variant="outline"
            disabled={busy || !allowFilesystemChanges}
            aria-label="Sync"
            onClick={onSync}
          >
            Sync
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={onOpenAuditLog}
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
              aria-checked={allowFilesystemChanges}
              disabled={busy}
              onClick={onAllowFilesystemChangesToggle}
              className={cn(
                "relative inline-flex h-6 w-11 items-center rounded-full border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60",
                allowFilesystemChanges
                  ? "border-primary/70 bg-primary/80"
                  : "border-border bg-muted-foreground/25",
              )}
            >
              <span
                aria-hidden="true"
                className={cn(
                  "inline-block h-4 w-4 transform rounded-full bg-background shadow-sm transition-transform",
                  allowFilesystemChanges ? "translate-x-5" : "translate-x-1",
                )}
              />
            </button>
          </div>
        </div>
      </div>
      {!allowFilesystemChanges ? (
        <p className="mt-2 text-xs text-muted-foreground">
          Read-only mode: filesystem changes are blocked.
        </p>
      ) : null}
      <div className="mt-2.5">
        <Input
          value={query}
          placeholder="Search by name, key, scope or workspace"
          onChange={(event) => onQueryChange(event.currentTarget.value)}
        />
      </div>
    </header>
  );
}
