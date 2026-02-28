import { cn } from "../../lib/utils";
import type { SkillRecord } from "../../types";
import { ScopeMarker } from "./ScopeMarker";

type SkillListPanelProps = {
  skills: SkillRecord[];
  selectedSkillKey: string | null;
  emptyText: string;
  onSelect: (skillKey: string) => void;
  onCloseMenus: () => void;
};

export function SkillListPanel({
  skills,
  selectedSkillKey,
  emptyText,
  onSelect,
  onCloseMenus,
}: SkillListPanelProps) {
  if (skills.length === 0) {
    return (
      <p className="rounded-md bg-muted/20 px-2 py-2 text-xs text-muted-foreground">
        {emptyText}
      </p>
    );
  }

  return (
    <ul className="space-y-0.5">
      {skills.map((skill) => {
        const selected = skill.skill_key === selectedSkillKey;
        return (
          <li key={skill.id}>
            <button
              type="button"
              className={cn(
                "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                selected
                  ? "bg-accent/85 text-foreground"
                  : "hover:bg-accent/55",
              )}
              onClick={() => {
                onSelect(skill.skill_key);
                onCloseMenus();
              }}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">
                  {skill.name}
                </span>
                <ScopeMarker scope={skill.scope} />
              </div>
              <p
                aria-hidden="true"
                className="mt-0.5 truncate text-[11px] text-muted-foreground"
              >
                {skill.skill_key}
              </p>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
