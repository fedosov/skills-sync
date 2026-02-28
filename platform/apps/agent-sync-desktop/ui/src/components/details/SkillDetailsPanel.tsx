import { Button } from "../ui/button";
import { CardContent, CardHeader, CardTitle } from "../ui/card";
import { Input } from "../ui/input";
import { formatUnixTime } from "../../skillUtils";
import type { SkillDetails } from "../../types";

function compactPath(path: string | null | undefined): string {
  if (!path) {
    return "-";
  }
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= 3) {
    return path;
  }
  return `/${segments[0]}/.../${segments[segments.length - 1]}`;
}

type SkillDetailsPanelProps = {
  details: SkillDetails;
  busy: boolean;
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
      <CardHeader className="border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle className="text-lg leading-tight">
              {details.skill.name}
            </CardTitle>
            <p className="mt-1 text-xs text-muted-foreground">
              {details.skill.skill_key}
            </p>
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
                  disabled={!details.main_file_exists}
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
                {details.skill.status === "active" ? (
                  <>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={busy}
                      className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                      onClick={onArchive}
                    >
                      Archive
                    </button>
                    {details.skill.scope === "project" ? (
                      <button
                        type="button"
                        role="menuitem"
                        disabled={busy}
                        className="block w-full rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                        onClick={onMakeGlobal}
                      >
                        Make global
                      </button>
                    ) : null}
                  </>
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
