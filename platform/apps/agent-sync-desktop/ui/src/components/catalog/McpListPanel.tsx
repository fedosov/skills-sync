import { getVisibleMcpAgents } from "../../lib/mcpAgents";
import { cn } from "../../lib/utils";
import type { McpServerRecord } from "../../types";
import { AgentLogoIcon } from "./AgentLogoIcon";
import { CatalogSelectableRow } from "./CatalogSelectableRow";
import { ScopeGroupedCatalog } from "./ScopeGroupedCatalog";
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
        const rowAgents = getVisibleMcpAgents().map((agent) => ({
          agent,
          enabled: server.enabled_by_agent[agent],
        }));

        return (
          <CatalogSelectableRow
            name={server.server_key}
            subtitle={server.transport.toUpperCase()}
            scope={server.scope}
            selected={key === selectedMcpKey}
            isFavorite={favorites.has(key)}
            headerAlign="start"
            meta={
              mcpStatus(server) === "archived" ? (
                <span className="text-[10px] text-muted-foreground">
                  Archived
                </span>
              ) : mcpStatus(server) === "unmanaged" ? (
                <span className="font-medium text-[10px] text-amber-500">
                  Unmanaged
                </span>
              ) : null
            }
            footer={
              <div className="flex items-center justify-between gap-2">
                <span className="flex min-w-0 items-center gap-1.5" />
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
            }
            onClick={() => {
              onSelect(key);
              onCloseMenus();
            }}
          />
        );
      }}
    />
  );
}
