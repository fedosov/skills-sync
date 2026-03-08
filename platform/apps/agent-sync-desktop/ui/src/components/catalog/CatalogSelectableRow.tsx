import type { ReactNode } from "react";
import { cn } from "../../lib/utils";
import { ScopeMarker } from "./ScopeMarker";
import { StarIcon } from "../ui/StarIcon";

type CatalogSelectableRowProps = {
  name: string;
  subtitle: string;
  scope: string;
  selected: boolean;
  isFavorite: boolean;
  onClick: () => void;
  meta?: ReactNode;
  footer?: ReactNode;
  headerAlign?: "center" | "start";
};

export function CatalogSelectableRow({
  name,
  subtitle,
  scope,
  selected,
  isFavorite,
  onClick,
  meta,
  footer,
  headerAlign = "center",
}: CatalogSelectableRowProps) {
  return (
    <button
      type="button"
      className={cn(
        "w-full rounded-md px-2.5 py-2 text-left transition-colors",
        selected ? "bg-accent/85 text-foreground" : "hover:bg-accent/55",
      )}
      onClick={onClick}
    >
      <div
        className={cn(
          "flex justify-between gap-2",
          headerAlign === "start" ? "items-start" : "items-center",
        )}
      >
        <span className="flex min-w-0 items-center gap-1">
          {isFavorite ? (
            <StarIcon filled className="h-3 w-3 shrink-0 text-amber-400" />
          ) : null}
          <span className="truncate text-sm font-medium">{name}</span>
        </span>
        <span className="inline-flex items-center gap-1.5">
          <ScopeMarker scope={scope} />
          {meta}
        </span>
      </div>

      <p
        aria-hidden="true"
        className="mt-0.5 truncate text-[11px] text-muted-foreground"
      >
        {subtitle}
      </p>

      {footer ? (
        <div className="mt-0.5 text-[11px] text-muted-foreground">{footer}</div>
      ) : null}
    </button>
  );
}
