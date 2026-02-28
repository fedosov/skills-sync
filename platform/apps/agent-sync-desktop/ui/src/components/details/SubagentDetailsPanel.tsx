import { Button } from "../ui/button";
import { CardContent, CardHeader, CardTitle } from "../ui/card";
import { formatUnixTime } from "../../skillUtils";
import type { SubagentDetails } from "../../types";

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

type SubagentDetailsPanelProps = {
  subagentDetails: SubagentDetails;
  openTargetMenu: boolean;
  onToggleOpenTargetMenu: () => void;
  onOpenPath: (target: "folder" | "file") => void;
};

export function SubagentDetailsPanel({
  subagentDetails,
  openTargetMenu,
  onToggleOpenTargetMenu,
  onOpenPath,
}: SubagentDetailsPanelProps) {
  return (
    <>
      <CardHeader className="border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle className="text-lg leading-tight">
              {subagentDetails.subagent.name}
            </CardTitle>
            <p className="mt-1 text-xs text-muted-foreground">
              {subagentDetails.subagent.subagent_key}
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

            {openTargetMenu ? (
              <div
                role="menu"
                className="absolute right-0 top-8 z-20 min-w-36 rounded-md border border-border/70 bg-card p-1 shadow-sm"
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
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
        <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
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
