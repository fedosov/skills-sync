import { CardContent } from "../ui/card";
import { Input } from "../ui/input";
import { compactPath, formatUnixTime } from "../../lib/formatting";
import type { SkillDetails } from "../../types";
import { EntityDetailHeader } from "./EntityDetailHeader";
import { EntityActionMenus } from "./EntityActionMenus";
import { Button } from "../ui/button";

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

      <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
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
            <dd className="mt-0.5 flex items-center gap-2 font-mono">
              <span title={details.main_file_path}>
                {compactPath(details.main_file_path)}
              </span>
              <Button
                size="sm"
                variant="ghost"
                aria-label="Copy main path"
                onClick={() =>
                  onCopyPath(details.main_file_path, "Copy main path failed.")
                }
              >
                Copy
              </Button>
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Canonical path</dt>
            <dd className="mt-0.5 flex items-center gap-2 font-mono">
              <span title={details.skill.canonical_source_path}>
                {compactPath(details.skill.canonical_source_path)}
              </span>
              <Button
                size="sm"
                variant="ghost"
                aria-label="Copy canonical path"
                onClick={() =>
                  onCopyPath(
                    details.skill.canonical_source_path,
                    "Copy canonical path failed.",
                  )
                }
              >
                Copy
              </Button>
            </dd>
          </div>
        </dl>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            SKILL.md preview
          </h3>
          {details.main_file_body_preview ? (
            <pre className="max-h-64 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
              {details.main_file_body_preview}
            </pre>
          ) : (
            <p className="text-xs text-muted-foreground">
              No readable preview available.
            </p>
          )}
        </section>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            SKILL dir tree
          </h3>
          {details.skill_dir_tree_preview ? (
            <pre className="max-h-48 overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed">
              {details.skill_dir_tree_preview}
            </pre>
          ) : (
            <p className="text-xs text-muted-foreground">
              No readable directory tree available.
            </p>
          )}
        </section>

        <section className="space-y-1.5 border-t border-border/50 pt-3">
          <h3 className="text-xs font-semibold text-muted-foreground">
            Targets
          </h3>
          {details.skill.target_paths.length === 0 ? (
            <p className="text-xs text-muted-foreground">No target paths.</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {details.skill.target_paths.map((path) => (
                <li key={path} className="rounded-md bg-muted/20 p-2 font-mono">
                  {path}
                </li>
              ))}
            </ul>
          )}
        </section>

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
      </CardContent>
    </>
  );
}
