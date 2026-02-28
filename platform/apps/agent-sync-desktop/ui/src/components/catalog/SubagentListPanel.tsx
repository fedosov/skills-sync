import { cn } from "../../lib/utils";
import type { SubagentRecord } from "../../types";
import { ScopeMarker } from "./ScopeMarker";

type SubagentListPanelProps = {
  subagents: SubagentRecord[];
  selectedSubagentId: string | null;
  emptyText: string;
  onSelect: (subagentId: string) => void;
  onCloseMenus: () => void;
};

export function SubagentListPanel({
  subagents,
  selectedSubagentId,
  emptyText,
  onSelect,
  onCloseMenus,
}: SubagentListPanelProps) {
  if (subagents.length === 0) {
    return (
      <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
        {emptyText}
      </p>
    );
  }

  return (
    <ul className="space-y-0.5">
      {subagents.map((subagent) => {
        const selected = subagent.id === selectedSubagentId;
        return (
          <li key={subagent.id}>
            <button
              type="button"
              className={cn(
                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                selected
                  ? "bg-accent/85 text-foreground"
                  : "hover:bg-accent/55",
              )}
              onClick={() => {
                onSelect(subagent.id);
                onCloseMenus();
              }}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">
                  {subagent.name}
                </span>
                <ScopeMarker scope={subagent.scope} />
              </div>
              <p
                aria-hidden="true"
                className="mt-0.5 truncate text-[11px] text-muted-foreground"
              >
                {subagent.subagent_key}
              </p>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
