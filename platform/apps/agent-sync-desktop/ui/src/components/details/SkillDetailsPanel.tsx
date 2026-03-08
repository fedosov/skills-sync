import { Input } from "../ui/input";
import { formatUnixTime } from "../../lib/formatting";
import type { SkillDetails } from "../../types";
import { EntityDetailHeader } from "./EntityDetailHeader";
import { EntityActionMenus } from "./EntityActionMenus";
import { Button } from "../ui/button";
import {
  DetailContent,
  DetailPathValue,
  DetailPreviewSection,
  DetailSection,
  DetailStringList,
} from "./DetailPrimitives";

type SkillDetailsPanelProps = {
  details: SkillDetails;
  busy: boolean;
  isFavorite: boolean;
  onToggleFavorite: () => void;
  renameDraft: string;
  openTargetMenu: boolean;
  actionsMenuOpen: boolean;
  onRenameDraftChange: (value: string) => void;
  onRenameSubmit: () => void;
  onToggleOpenTargetMenu: () => void;
  onToggleActionsMenu: () => void;
  onOpenPath: (target: "folder" | "file") => void;
  onArchive: () => void;
  onMakeGlobal: () => void;
  onRestore: () => void;
  onRequestDelete: () => void;
  onCopyPath: (path: string, errorLabel: string) => void;
};

export function SkillDetailsPanel({
  details,
  busy,
  isFavorite,
  onToggleFavorite,
  renameDraft,
  openTargetMenu,
  actionsMenuOpen,
  onRenameDraftChange,
  onRenameSubmit,
  onToggleOpenTargetMenu,
  onToggleActionsMenu,
  onOpenPath,
  onArchive,
  onMakeGlobal,
  onRestore,
  onRequestDelete,
  onCopyPath,
}: SkillDetailsPanelProps) {
  return (
    <>
      <EntityDetailHeader
        name={details.skill.name}
        entityKey={details.skill.skill_key}
        entityLabel="skill"
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
                disabled: !details.main_file_exists,
              },
            ]}
            actionItems={
              details.skill.status === "active"
                ? [
                    { label: "Archive", onSelect: onArchive, disabled: busy },
                    ...(details.skill.scope === "project"
                      ? [
                          {
                            label: "Make global",
                            onSelect: onMakeGlobal,
                            disabled: busy,
                          },
                        ]
                      : []),
                    {
                      label: "Delete",
                      onSelect: onRequestDelete,
                      disabled: busy,
                      tone: "destructive" as const,
                    },
                  ]
                : [
                    { label: "Restore", onSelect: onRestore, disabled: busy },
                    {
                      label: "Delete",
                      onSelect: onRequestDelete,
                      disabled: busy,
                      tone: "destructive" as const,
                    },
                  ]
            }
          />
        }
      />

      <DetailContent>
        <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
          <div>
            <dt className="text-muted-foreground">Workspace</dt>
            <dd className="mt-0.5 break-all font-mono">
              {details.skill.workspace ?? "-"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Updated</dt>
            <dd className="mt-0.5">
              {formatUnixTime(details.last_modified_unix_seconds)}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Install status</dt>
            <dd className="mt-0.5">{details.skill.install_status ?? "n/a"}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Source</dt>
            <dd className="mt-0.5 break-all font-mono">
              {details.skill.source ?? "-"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Main file</dt>
            <DetailPathValue
              path={details.main_file_path}
              copyAriaLabel="Copy main path"
              onCopy={() =>
                onCopyPath(details.main_file_path, "Copy main path failed.")
              }
            />
          </div>
          <div>
            <dt className="text-muted-foreground">Canonical path</dt>
            <DetailPathValue
              path={details.skill.canonical_source_path}
              copyAriaLabel="Copy canonical path"
              onCopy={() =>
                onCopyPath(
                  details.skill.canonical_source_path,
                  "Copy canonical path failed.",
                )
              }
            />
          </div>
        </dl>

        <DetailPreviewSection
          title="SKILL.md preview"
          preview={details.main_file_body_preview}
          emptyText="No readable preview available."
        />

        <DetailPreviewSection
          title="SKILL dir tree"
          preview={details.skill_dir_tree_preview}
          emptyText="No readable directory tree available."
          maxHeightClass="max-h-48"
        />

        <DetailSection title="Targets">
          <DetailStringList
            items={details.skill.target_paths}
            emptyText="No target paths."
          />
        </DetailSection>

        {details.skill.status === "active" ? (
          <form
            className="flex flex-wrap items-center gap-2 border-t border-border/50 pt-3"
            onSubmit={(event) => {
              event.preventDefault();
              onRenameSubmit();
            }}
          >
            <Input
              value={renameDraft}
              onChange={(event) =>
                onRenameDraftChange(event.currentTarget.value)
              }
              placeholder="New skill title"
              className="min-w-[220px] flex-1"
            />
            <Button
              type="submit"
              size="sm"
              disabled={
                busy ||
                renameDraft.trim().length === 0 ||
                renameDraft.trim() === details.skill.name
              }
            >
              Save name
            </Button>
          </form>
        ) : null}
      </DetailContent>
    </>
  );
}
