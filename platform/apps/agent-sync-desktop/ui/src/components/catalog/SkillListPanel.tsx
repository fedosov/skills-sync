import type { SkillRecord } from "../../types";
import { ScopeGroupedCatalog } from "./ScopeGroupedCatalog";
import { CatalogSelectableRow } from "./CatalogSelectableRow";

type SkillListPanelProps = {
  skills: SkillRecord[];
  query: string;
  selectedSkillKey: string | null;
  favorites: Set<string>;
  emptyText: string;
  expandedProjectGroups: Record<string, boolean | undefined>;
  onSelect: (skillKey: string) => void;
  onToggleProjectGroup: (groupKey: string, currentExpanded: boolean) => void;
  onCloseMenus: () => void;
};

export function SkillListPanel({
  skills,
  query,
  selectedSkillKey,
  favorites,
  emptyText,
  expandedProjectGroups,
  onSelect,
  onToggleProjectGroup,
  onCloseMenus,
}: SkillListPanelProps) {
  return (
    <ScopeGroupedCatalog
      items={skills}
      query={query}
      emptyText={emptyText}
      expandedProjectGroups={expandedProjectGroups}
      getItemKey={(skill) => skill.id}
      getScope={(skill) => skill.scope}
      getWorkspace={(skill) => skill.workspace}
      isItemSelected={(skill) => skill.skill_key === selectedSkillKey}
      onToggleProjectGroup={onToggleProjectGroup}
      renderItem={(skill) => {
        return (
          <CatalogSelectableRow
            name={skill.name}
            subtitle={skill.skill_key}
            scope={skill.scope}
            selected={skill.skill_key === selectedSkillKey}
            isFavorite={favorites.has(skill.id)}
            onClick={() => {
              onSelect(skill.skill_key);
              onCloseMenus();
            }}
          />
        );
      }}
    />
  );
}
