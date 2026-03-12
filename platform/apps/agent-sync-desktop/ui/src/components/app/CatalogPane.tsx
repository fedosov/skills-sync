import { AgentsListPanel } from "../catalog/AgentsListPanel";
import { McpListPanel } from "../catalog/McpListPanel";
import { SkillListPanel } from "../catalog/SkillListPanel";
import { SubagentListPanel } from "../catalog/SubagentListPanel";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { cn } from "../../lib/utils";
import type { CatalogProjectGroupState } from "../../lib/uiStateTypes";
import type {
  AgentContextEntry,
  FocusKind,
  McpServerRecord,
  SkillRecord,
  SubagentRecord,
} from "../../types";

const CATALOG_TABS: ReadonlyArray<readonly [FocusKind, string]> = [
  ["skills", "Skills"],
  ["subagents", "Subagents"],
  ["mcp", "MCP"],
  ["agents", "Agents.md"],
];

type CatalogPaneProps = {
  focusKind: FocusKind;
  catalogTabCounts: Record<FocusKind, number>;
  activeCatalogTitle: string;
  activeCatalogCount: number;
  activeCatalogTotal: number;
  activeCatalogEmptyText: string;
  query: string;
  filteredSkills: SkillRecord[];
  filteredSubagents: SubagentRecord[];
  filteredMcpServers: McpServerRecord[];
  filteredAgentEntries: AgentContextEntry[];
  selectedSkillKey: string | null;
  selectedSubagentId: string | null;
  selectedMcpKey: string | null;
  selectedAgentEntryId: string | null;
  starredSkillSet: Set<string>;
  subagentFavorites: Set<string>;
  mcpFavorites: Set<string>;
  agentFavorites: Set<string>;
  expandedProjectGroups: CatalogProjectGroupState;
  onTabChange: (next: FocusKind) => void;
  onToggleProjectGroup: (
    kind: FocusKind,
    groupKey: string,
    currentExpanded: boolean,
  ) => void;
  onSelectSkill: (skillKey: string) => void;
  onSelectSubagent: (subagentId: string) => void;
  onSelectMcp: (selectionKey: string) => void;
  onSelectAgent: (entryId: string) => void;
  onCloseMenus: () => void;
};

export function CatalogPane({
  focusKind,
  catalogTabCounts,
  activeCatalogTitle,
  activeCatalogCount,
  activeCatalogTotal,
  activeCatalogEmptyText,
  query,
  filteredSkills,
  filteredSubagents,
  filteredMcpServers,
  filteredAgentEntries,
  selectedSkillKey,
  selectedSubagentId,
  selectedMcpKey,
  selectedAgentEntryId,
  starredSkillSet,
  subagentFavorites,
  mcpFavorites,
  agentFavorites,
  expandedProjectGroups,
  onTabChange,
  onToggleProjectGroup,
  onSelectSkill,
  onSelectSubagent,
  onSelectMcp,
  onSelectAgent,
  onCloseMenus,
}: CatalogPaneProps) {
  return (
    <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
      <CardHeader className="pb-2">
        <CardTitle>Catalog</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 p-2 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
        <div className="flex flex-wrap items-center gap-1.5">
          {CATALOG_TABS.map(([kind, label]) => {
            const isActive = focusKind === kind;
            return (
              <Button
                key={kind}
                type="button"
                size="sm"
                variant={isActive ? "outline" : "ghost"}
                aria-label={`Switch catalog to ${label}`}
                aria-pressed={isActive}
                className={cn(
                  "h-6 px-2 text-[11px]",
                  isActive ? "bg-accent/70" : "text-muted-foreground",
                )}
                onClick={() => onTabChange(kind)}
              >
                {`${label} (${catalogTabCounts[kind]})`}
              </Button>
            );
          })}
        </div>

        <section
          className="space-y-1.5 border-t border-border/50 pt-3"
          data-testid="active-catalog-panel"
        >
          <div className="flex items-center justify-between">
            <p className="text-xs font-semibold text-muted-foreground">
              {activeCatalogTitle}
            </p>
            <span className="text-[11px] text-muted-foreground">
              {activeCatalogCount}/{activeCatalogTotal}
            </span>
          </div>

          {focusKind === "skills" ? (
            <SkillListPanel
              skills={filteredSkills}
              query={query}
              selectedSkillKey={selectedSkillKey}
              favorites={starredSkillSet}
              emptyText={activeCatalogEmptyText}
              expandedProjectGroups={expandedProjectGroups.skills}
              onSelect={onSelectSkill}
              onToggleProjectGroup={(groupKey, currentExpanded) =>
                onToggleProjectGroup("skills", groupKey, currentExpanded)
              }
              onCloseMenus={onCloseMenus}
            />
          ) : null}

          {focusKind === "subagents" ? (
            <SubagentListPanel
              subagents={filteredSubagents}
              query={query}
              selectedSubagentId={selectedSubagentId}
              favorites={subagentFavorites}
              emptyText={activeCatalogEmptyText}
              expandedProjectGroups={expandedProjectGroups.subagents}
              onSelect={onSelectSubagent}
              onToggleProjectGroup={(groupKey, currentExpanded) =>
                onToggleProjectGroup("subagents", groupKey, currentExpanded)
              }
              onCloseMenus={onCloseMenus}
            />
          ) : null}

          {focusKind === "mcp" ? (
            <McpListPanel
              servers={filteredMcpServers}
              query={query}
              selectedMcpKey={selectedMcpKey}
              favorites={mcpFavorites}
              emptyText={activeCatalogEmptyText}
              expandedProjectGroups={expandedProjectGroups.mcp}
              onSelect={onSelectMcp}
              onToggleProjectGroup={(groupKey, currentExpanded) =>
                onToggleProjectGroup("mcp", groupKey, currentExpanded)
              }
              onCloseMenus={onCloseMenus}
            />
          ) : null}

          {focusKind === "agents" ? (
            <AgentsListPanel
              entries={filteredAgentEntries}
              query={query}
              selectedAgentEntryId={selectedAgentEntryId}
              favorites={agentFavorites}
              emptyText={activeCatalogEmptyText}
              expandedProjectGroups={expandedProjectGroups.agents}
              onSelect={onSelectAgent}
              onToggleProjectGroup={(groupKey, currentExpanded) =>
                onToggleProjectGroup("agents", groupKey, currentExpanded)
              }
              onCloseMenus={onCloseMenus}
            />
          ) : null}
        </section>
      </CardContent>
    </Card>
  );
}
