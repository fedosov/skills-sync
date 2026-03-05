import { AgentLogoIcon } from "../catalog/AgentLogoIcon";
import { Button } from "../ui/button";
import { StarIcon } from "../ui/StarIcon";
import { CardContent, CardHeader, CardTitle } from "../ui/card";
import { getVisibleMcpAgents } from "../../lib/mcpAgents";
import { mcpStatus, toTitleCase } from "../../lib/catalogUtils";
import { cn } from "../../lib/utils";
import type { McpServerRecord } from "../../types";

type McpDetailsPanelProps = {
  server: McpServerRecord;
  warnings: string[];
  busy: boolean;
  isFavorite: boolean;
  onToggleFavorite: () => void;
  actionsMenuOpen: boolean;
  onToggleActionsMenu: () => void;
  onSetEnabled: (agent: "codex" | "claude", enabled: boolean) => void;
  onArchive: () => void;
  onMakeGlobal: () => void;
  onRestore: () => void;
  onRequestDelete: () => void;
};

export function McpDetailsPanel({
  server,
  warnings,
  busy,
  isFavorite,
  onToggleFavorite,
  actionsMenuOpen,
  onToggleActionsMenu,
  onSetEnabled,
  onArchive,
  onMakeGlobal,
  onRestore,
  onRequestDelete,
}: McpDetailsPanelProps) {
  const status = mcpStatus(server);

  return (
    <>
      <CardHeader className="border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div className="flex items-start gap-1.5">
            <button
              type="button"
              aria-label={isFavorite ? "Unstar MCP server" : "Star MCP server"}
              className={
                isFavorite
                  ? "mt-0.5 text-amber-400 hover:text-amber-500"
                  : "mt-0.5 text-muted-foreground/50 hover:text-amber-400"
              }
              onClick={onToggleFavorite}
            >
              <StarIcon filled={isFavorite} className="h-4 w-4" />
            </button>
            <div>
              <CardTitle className="text-lg leading-tight">
                {server.server_key}
              </CardTitle>
              <p className="mt-1 text-xs text-muted-foreground">
                {`${server.transport.toUpperCase()} · ${toTitleCase(server.scope)}`}
              </p>
            </div>
          </div>
          <div className="relative">
            <Button
              size="sm"
              variant="ghost"
              aria-label="More actions"
              disabled={busy}
              aria-expanded={actionsMenuOpen}
              onClick={onToggleActionsMenu}
            >
              ⋯
            </Button>
            {actionsMenuOpen ? (
              <div
                role="menu"
                className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
              >
                {status === "active" ? (
                  <>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={busy}
                      className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                      onClick={onArchive}
                    >
                      Archive
                    </button>
                    {server.scope === "project" ? (
                      <button
                        type="button"
                        role="menuitem"
                        disabled={busy}
                        className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                        onClick={onMakeGlobal}
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
                    onClick={onRestore}
                  >
                    Restore
                  </button>
                )}
                <button
                  type="button"
                  role="menuitem"
                  disabled={busy}
                  className="block w-full rounded-sm px-2 py-1.5 text-left text-xs text-destructive hover:bg-destructive/10 disabled:cursor-not-allowed disabled:opacity-50"
                  onClick={onRequestDelete}
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
            <dt className="text-muted-foreground">Status</dt>
            <dd className="mt-0.5 capitalize">{status}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Command</dt>
            <dd className="mt-0.5 break-all font-mono">
              {server.command ?? "-"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">URL</dt>
            <dd className="mt-0.5 break-all font-mono">{server.url ?? "-"}</dd>
          </div>
        </dl>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            Enable by agent
          </h3>
          <div className="flex flex-wrap gap-3">
            {getVisibleMcpAgents().map((agent) => {
              const enabled = server.enabled_by_agent[agent];
              return (
                <div
                  key={agent}
                  className="inline-flex items-center gap-2 px-1 py-1"
                >
                  <span className="inline-flex items-center gap-1.5 text-xs font-medium">
                    <span
                      role="img"
                      aria-label={`${agent} agent`}
                      className={cn(
                        "inline-flex items-center transition-colors",
                        enabled
                          ? "text-emerald-500"
                          : "text-muted-foreground/70 opacity-60",
                      )}
                    >
                      <AgentLogoIcon agent={agent} className="h-3.5 w-3.5" />
                    </span>
                    <span>{agent}</span>
                  </span>
                  <button
                    type="button"
                    role="switch"
                    aria-label={`${agent} toggle`}
                    aria-checked={enabled}
                    disabled={busy || status === "archived"}
                    onClick={() => onSetEnabled(agent, !enabled)}
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
          {status === "archived" ? (
            <p className="text-xs text-muted-foreground">
              Restore this MCP server to change per-agent toggles.
            </p>
          ) : null}
        </section>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">Args</h3>
          {server.args.length === 0 ? (
            <p className="text-xs text-muted-foreground">No args.</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {server.args.map((arg) => (
                <li key={arg} className="rounded-md bg-muted/20 p-2 font-mono">
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
          {server.targets.length === 0 ? (
            <p className="text-xs text-muted-foreground">No managed targets.</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {server.targets.map((path) => (
                <li key={path} className="rounded-md bg-muted/20 p-2 font-mono">
                  {path}
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            Warnings
          </h3>
          {warnings.length === 0 ? (
            <p className="text-xs text-muted-foreground">No warnings.</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {warnings.map((warning) => (
                <li key={warning} className="rounded-md bg-muted/20 p-2">
                  {warning}
                </li>
              ))}
            </ul>
          )}
        </section>
      </CardContent>
    </>
  );
}
