import { formatUnixTime } from "../../lib/formatting";
import { subagentStatus } from "../../lib/catalogUtils";
import type { SubagentDetails } from "../../types";
import { EntityActionMenus } from "./EntityActionMenus";
import { EntityDetailHeader } from "./EntityDetailHeader";
import {
  DetailContent,
  DetailPathValue,
  DetailPreviewSection,
  DetailSection,
  DetailStringList,
} from "./DetailPrimitives";

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

      <DetailContent>
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
            <DetailPathValue path={subagentDetails.main_file_path} />
          </div>
          <div>
            <dt className="text-muted-foreground">Canonical path</dt>
            <DetailPathValue
              path={subagentDetails.subagent.canonical_source_path}
            />
          </div>
        </dl>

        <DetailSection title="Targets">
          <DetailStringList
            items={subagentDetails.subagent.target_paths}
            emptyText="No target paths."
          />
        </DetailSection>

        <DetailPreviewSection
          title="Subagent prompt preview"
          preview={subagentDetails.main_file_body_preview}
          emptyText="No readable preview available."
        />
      </DetailContent>
    </>
  );
}
