import { getVisibleMcpAgents } from "../../lib/mcpAgents";
import { cn } from "../../lib/utils";
import type { McpServerRecord } from "../../types";
import { AgentLogoIcon } from "./AgentLogoIcon";
import { ScopeGroupedCatalog } from "./ScopeGroupedCatalog";
import { ScopeMarker } from "./ScopeMarker";
import { StarIcon } from "../ui/StarIcon";
import { mcpSelectionKey, mcpStatus } from "../../lib/catalogUtils";

type McpListPanelProps = {
  servers: McpServerRecord[];
  query: string;
  selectedMcpKey: string | null;
  favorites: Set<string>;
  emptyText: string;
  expandedProjectGroups: Record<string, boolean | undefined>;
  onSelect: (mcpKey: string) => void;
  onToggleProjectGroup: (groupKey: string, currentExpanded: boolean) => void;
  onCloseMenus: () => void;
};

export function McpListPanel({
  servers,
  query,
  selectedMcpKey,
  favorites,
  emptyText,
  expandedProjectGroups,
  onSelect,
  onToggleProjectGroup,
  onCloseMenus,
}: McpListPanelProps) {
  return (
    <ScopeGroupedCatalog
      items={servers}
      query={query}
      emptyText={emptyText}
      expandedProjectGroups={expandedProjectGroups}
      getItemKey={(server) => mcpSelectionKey(server)}
      getScope={(server) => server.scope}
      getWorkspace={(server) => server.workspace}
      isItemSelected={(server) => mcpSelectionKey(server) === selectedMcpKey}
      onToggleProjectGroup={onToggleProjectGroup}
      renderItem={(server) => {
        const key = mcpSelectionKey(server);
        const selected = key === selectedMcpKey;
        const rowAgents = getVisibleMcpAgents().map((agent) => ({
          agent,
          enabled: server.enabled_by_agent[agent],
        }));

        return (
          <button
            type="button"
            className={cn(
              "w-full rounded-md px-2.5 py-2 text-left transition-colors",
              selected ? "bg-accent/85 text-foreground" : "hover:bg-accent/55",
            )}
            onClick={() => {
              onSelect(key);
              onCloseMenus();
            }}
          >
            <div className="flex items-start justify-between gap-2">
              <span className="flex min-w-0 items-center gap-1">
                {favorites.has(key) ? (
                  <StarIcon
                    filled
                    className="h-3 w-3 shrink-0 text-amber-400"
                  />
                ) : null}
                <span className="truncate text-sm font-medium">
                  {server.server_key}
                </span>
              </span>
              <span className="inline-flex items-center gap-1.5">
                <ScopeMarker scope={server.scope} />
                {mcpStatus(server) === "archived" ? (
                  <span className="text-[10px] text-muted-foreground">
                    Archived
                  </span>
                ) : mcpStatus(server) === "unmanaged" ? (
                  <span className="font-medium text-[10px] text-amber-500">
                    Unmanaged
                  </span>
                ) : null}
              </span>
            </div>
            <div className="mt-0.5 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
              <span className="flex min-w-0 items-center gap-1.5">
                <span className="shrink-0 font-medium uppercase tracking-wide">
                  {server.transport.toUpperCase()}
                </span>
              </span>
              <ul className="flex shrink-0 items-center gap-1.5">
                {rowAgents.map(({ agent, enabled }) => (
                  <li key={agent}>
                    <span
                      role="img"
                      aria-label={`${agent} ${enabled ? "connected" : "disabled"}`}
                      className={cn(
                        "inline-flex items-center",
                        enabled
                          ? "text-emerald-500"
                          : "text-muted-foreground/50 opacity-30",
                      )}
                    >
                      <AgentLogoIcon agent={agent} className="h-3.5 w-3.5" />
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          </button>
        );
      }}
    />
  );
}
