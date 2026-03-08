import { cn } from "../../lib/utils";
import { Button } from "../ui/button";

export type EntityActionMenuItem = {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  tone?: "default" | "destructive";
};

type EntityActionMenusProps = {
  busy?: boolean;
  openMenuExpanded?: boolean;
  actionsMenuExpanded: boolean;
  onToggleOpenMenu?: () => void;
  onToggleActionsMenu: () => void;
  openItems?: EntityActionMenuItem[];
  actionItems: EntityActionMenuItem[];
};

function menuItemClassName(
  tone: EntityActionMenuItem["tone"] = "default",
): string {
  return cn(
    "block w-full rounded-sm px-2 py-1.5 text-left text-xs disabled:cursor-not-allowed disabled:opacity-50",
    tone === "destructive"
      ? "text-destructive hover:bg-destructive/10"
      : "hover:bg-accent",
  );
}

export function EntityActionMenus({
  busy = false,
  openMenuExpanded = false,
  actionsMenuExpanded,
  onToggleOpenMenu,
  onToggleActionsMenu,
  openItems = [],
  actionItems,
}: EntityActionMenusProps) {
  const hasOpenMenu = openItems.length > 0 && onToggleOpenMenu !== undefined;

  return (
    <div className="relative flex items-center gap-1.5">
      {hasOpenMenu ? (
        <>
          <Button
            size="sm"
            variant="outline"
            aria-expanded={openMenuExpanded}
            onClick={onToggleOpenMenu}
          >
            Open…
          </Button>
          {openMenuExpanded ? (
            <div
              role="menu"
              className="absolute right-14 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
            >
              {openItems.map((item) => (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  className={menuItemClassName(item.tone)}
                  onClick={item.onSelect}
                >
                  {item.label}
                </button>
              ))}
            </div>
          ) : null}
        </>
      ) : null}

      <Button
        size="sm"
        variant="ghost"
        aria-label="More actions"
        disabled={busy}
        aria-expanded={actionsMenuExpanded}
        onClick={onToggleActionsMenu}
      >
        ⋯
      </Button>

      {actionsMenuExpanded ? (
        <div
          role="menu"
          className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
        >
          {actionItems.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              className={menuItemClassName(item.tone)}
              onClick={item.onSelect}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
