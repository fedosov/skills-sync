import { CardContent } from "../ui/card";
import { compactPath, formatUnixTime } from "../../lib/formatting";
import { subagentStatus } from "../../lib/catalogUtils";
import type { SubagentDetails } from "../../types";
import { EntityActionMenus } from "./EntityActionMenus";
import { EntityDetailHeader } from "./EntityDetailHeader";

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
      <EntityDetailHeader
        name={subagentDetails.subagent.name}
        entityKey={subagentDetails.subagent.subagent_key}
        entityLabel="subagent"
        isFavorite={isFavorite}
        onToggleFavorite={onToggleFavorite}
        actions={
          <EntityActionMenus
            busy={busy}
            openMenuExpanded={openTargetMenu}
            actionsMenuExpanded={actionsMenuOpen}
            onToggleOpenMenu={onToggleOpenTargetMenu}
            onToggleActionsMenu={onToggleActionsMenu}
            openItems={[
              { label: "Open folder", onSelect: () => onOpenPath("folder") },
              {
                label: "Open file",
                onSelect: () => onOpenPath("file"),
                disabled: !subagentDetails.main_file_exists,
              },
            ]}
            actionItems={[
              subagentStatus(subagentDetails.subagent) === "active"
                ? { label: "Archive", onSelect: onArchive, disabled: busy }
                : { label: "Restore", onSelect: onRestore, disabled: busy },
              {
                label: "Delete",
                onSelect: onRequestDelete,
                disabled: busy,
                tone: "destructive",
              },
            ]}
          />
        }
      />

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
