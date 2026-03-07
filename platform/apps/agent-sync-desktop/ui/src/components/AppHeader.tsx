import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { cn } from "../lib/utils";
import {
  toTitleCase,
  syncStatusVariant,
  severityDotClass,
} from "../lib/catalogUtils";
import type { DotagentsProofStatus } from "../hooks/useDotagentsVerification";
import type { AgentsContextReport, RuntimeControls, SyncState } from "../types";

type AppHeaderProps = {
  state: SyncState | null;
  runtimeControls: RuntimeControls | null;
  busy: boolean;
  query: string;
  setQuery: (query: string) => void;
  activeSkillCount: number;
  archivedSkillCount: number;
  activeSubagentCount: number;
  archivedSubagentCount: number;
  activeMcpCount: number;
  archivedMcpCount: number;
  agentContextCount: number;
  agentsReport: AgentsContextReport | null;
  dotagentsProofStatus: DotagentsProofStatus;
  dotagentsProofSummary: string;
  dotagentsNeedsMigration: boolean;
  onSync: () => void;
  onVerifyDotagents: () => void;
  onInitializeDotagents: () => void;
  onAuditOpen: () => void;
  onAllowToggle: (allow: boolean) => void;
};

export function AppHeader({
  state,
  runtimeControls,
  busy,
  query,
  setQuery,
  activeSkillCount,
  archivedSkillCount,
  activeSubagentCount,
  archivedSubagentCount,
  activeMcpCount,
  archivedMcpCount,
  agentContextCount,
  agentsReport,
  dotagentsProofStatus,
  dotagentsProofSummary,
  dotagentsNeedsMigration,
  onSync,
  onVerifyDotagents,
  onInitializeDotagents,
  onAuditOpen,
  onAllowToggle,
}: AppHeaderProps) {
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
            Active {activeSkillCount} · Archived {archivedSkillCount} · Skills{" "}
            {state?.skills.length ?? 0} · Subagents A {activeSubagentCount}/R{" "}
            {archivedSubagentCount} · MCP A {activeMcpCount}/R{" "}
            {archivedMcpCount} · Agents {agentContextCount}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            disabled={busy || !runtimeControls?.allow_filesystem_changes}
            aria-label="Sync"
            onClick={onSync}
          >
            Sync
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={busy || !runtimeControls?.allow_filesystem_changes}
            aria-label="Verify dotagents"
            onClick={onVerifyDotagents}
          >
            Verify dotagents
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={onAuditOpen}
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
                onAllowToggle(
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
            onClick={onInitializeDotagents}
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
  );
}
