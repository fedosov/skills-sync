import type { SubagentRecord } from "../../types";
import { ScopeGroupedCatalog } from "./ScopeGroupedCatalog";
import { CatalogSelectableRow } from "./CatalogSelectableRow";

type SubagentListPanelProps = {
  subagents: SubagentRecord[];
  query: string;
  selectedSubagentId: string | null;
  favorites: Set<string>;
  emptyText: string;
  expandedProjectGroups: Record<string, boolean | undefined>;
  onSelect: (subagentId: string) => void;
  onToggleProjectGroup: (groupKey: string, currentExpanded: boolean) => void;
  onCloseMenus: () => void;
};

export function SubagentListPanel({
  subagents,
  query,
  selectedSubagentId,
  favorites,
  emptyText,
  expandedProjectGroups,
  onSelect,
  onToggleProjectGroup,
  onCloseMenus,
}: SubagentListPanelProps) {
  return (
    <ScopeGroupedCatalog
      items={subagents}
      query={query}
      emptyText={emptyText}
      expandedProjectGroups={expandedProjectGroups}
      getItemKey={(subagent) => subagent.id}
      getScope={(subagent) => subagent.scope}
      getWorkspace={(subagent) => subagent.workspace}
      isItemSelected={(subagent) => subagent.id === selectedSubagentId}
      onToggleProjectGroup={onToggleProjectGroup}
      renderItem={(subagent) => {
        return (
          <CatalogSelectableRow
            name={subagent.name}
            subtitle={subagent.subagent_key}
            scope={subagent.scope}
            selected={subagent.id === selectedSubagentId}
            isFavorite={favorites.has(subagent.id)}
            onClick={() => {
              onSelect(subagent.id);
              onCloseMenus();
            }}
          />
        );
      }}
    />
  );
}
