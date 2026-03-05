import { Button } from "../ui/button";
import { StarIcon } from "../ui/StarIcon";
import { CardContent, CardHeader, CardTitle } from "../ui/card";
import { compactPath } from "../../lib/formatting";
import { formatUnixTime } from "../../skillUtils";
import { subagentStatus } from "../../lib/catalogUtils";
import type { SubagentDetails } from "../../types";

type SubagentDetailsPanelProps = {
  subagentDetails: SubagentDetails;
  busy: boolean;
  isFavorite: boolean;
  onToggleFavorite: () => void;
  openTargetMenu: boolean;
  actionsMenuOpen: boolean;
  onToggleOpenTargetMenu: () => void;
  onToggleActionsMenu: () => void;
  onOpenPath: (target: "folder" | "file") => void;
  onArchive: () => void;
  onRestore: () => void;
  onRequestDelete: () => void;
};

export function SubagentDetailsPanel({
  subagentDetails,
  busy,
  isFavorite,
  onToggleFavorite,
  openTargetMenu,
  actionsMenuOpen,
  onToggleOpenTargetMenu,
  onToggleActionsMenu,
  onOpenPath,
  onArchive,
  onRestore,
  onRequestDelete,
}: SubagentDetailsPanelProps) {
  return (
    <>
      <CardHeader className="border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div className="flex items-start gap-1.5">
            <button
              type="button"
              aria-label={isFavorite ? "Unstar subagent" : "Star subagent"}
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
                {subagentDetails.subagent.name}
              </CardTitle>
              <p className="mt-1 text-xs text-muted-foreground">
                {subagentDetails.subagent.subagent_key}
              </p>
            </div>
          </div>
          <div className="relative flex items-center gap-1.5">
            <Button
              size="sm"
              variant="outline"
              aria-expanded={openTargetMenu}
              onClick={onToggleOpenTargetMenu}
            >
              Open…
            </Button>
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

            {openTargetMenu ? (
              <div
                role="menu"
                className="absolute right-14 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
              >
                <button
                  type="button"
                  role="menuitem"
                  className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent"
                  onClick={() => onOpenPath("folder")}
                >
                  Open folder
                </button>
                <button
                  type="button"
                  role="menuitem"
                  disabled={!subagentDetails.main_file_exists}
                  className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:opacity-50"
                  onClick={() => onOpenPath("file")}
                >
                  Open file
                </button>
              </div>
            ) : null}

            {actionsMenuOpen ? (
              <div
                role="menu"
                className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
              >
                {subagentStatus(subagentDetails.subagent) === "active" ? (
                  <button
                    type="button"
                    role="menuitem"
                    disabled={busy}
                    className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                    onClick={onArchive}
                  >
                    Archive
                  </button>
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
            <dd className="mt-0.5 capitalize">
              {subagentStatus(subagentDetails.subagent)}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Workspace</dt>
            <dd className="mt-0.5 break-all font-mono">
              {subagentDetails.subagent.workspace ?? "-"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Updated</dt>
            <dd className="mt-0.5">
              {formatUnixTime(subagentDetails.last_modified_unix_seconds)}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Main file</dt>
            <dd className="mt-0.5 break-all font-mono">
              {compactPath(subagentDetails.main_file_path)}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Canonical path</dt>
            <dd
              className="mt-0.5 break-all font-mono"
              title={subagentDetails.subagent.canonical_source_path}
            >
              {compactPath(subagentDetails.subagent.canonical_source_path)}
            </dd>
          </div>
        </dl>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            Targets
          </h3>
          {subagentDetails.subagent.target_paths.length === 0 ? (
            <p className="text-xs text-muted-foreground">No target paths.</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {subagentDetails.subagent.target_paths.map((path) => (
                <li key={path} className="rounded-md bg-muted/20 p-2 font-mono">
                  {path}
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            Subagent prompt preview
          </h3>
          {subagentDetails.main_file_body_preview ? (
            <pre className="max-h-64 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
              {subagentDetails.main_file_body_preview}
            </pre>
          ) : (
            <p className="text-xs text-muted-foreground">
              No readable preview available.
            </p>
          )}
        </section>
      </CardContent>
    </>
  );
}
