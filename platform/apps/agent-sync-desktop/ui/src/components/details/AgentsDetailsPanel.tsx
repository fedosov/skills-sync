import { Badge } from "../ui/badge";
import { StarIcon } from "../ui/StarIcon";
import { CardHeader, CardTitle } from "../ui/card";
import type { AgentContextEntry, AgentContextSegment } from "../../types";
import {
  DetailContent,
  DetailSection,
  DetailStringList,
} from "./DetailPrimitives";

type AgentsDetailsPanelProps = {
  entry: AgentContextEntry;
  topSegments: AgentContextSegment[];
  isFavorite: boolean;
  onToggleFavorite: () => void;
};

export function AgentsDetailsPanel({
  entry,
  topSegments,
  isFavorite,
  onToggleFavorite,
}: AgentsDetailsPanelProps) {
  return (
    <>
      <CardHeader className="border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div className="flex items-start gap-1.5">
            <button
              type="button"
              aria-label={
                isFavorite ? "Unstar agent entry" : "Star agent entry"
              }
              className={
                isFavorite
                  ? "mt-0.5 text-amber-400 hover:text-amber-500"
                  : "mt-0.5 text-muted-foreground/50 hover:text-amber-400"
              }
              onClick={onToggleFavorite}
            >
              <StarIcon filled={isFavorite} className="h-4 w-4" />
            </button>
            <div className="min-w-0">
              <CardTitle className="text-lg leading-tight">
                {entry.scope === "global"
                  ? "Global AGENTS.md"
                  : "Project AGENTS.md"}
              </CardTitle>
              <p className="mt-1 truncate text-xs text-muted-foreground">
                {entry.root_path}
              </p>
            </div>
          </div>
          <Badge
            variant={
              entry.severity === "critical"
                ? "error"
                : entry.severity === "warning"
                  ? "warning"
                  : "success"
            }
          >
            {entry.severity}
          </Badge>
        </div>
      </CardHeader>

      <DetailContent>
        <dl className="grid gap-x-4 gap-y-2 text-xs sm:grid-cols-2">
          <div>
            <dt className="text-muted-foreground">Scope</dt>
            <dd className="mt-0.5">{entry.scope}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Workspace</dt>
            <dd className="mt-0.5 break-all font-mono">
              {entry.workspace ?? "-"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Root path</dt>
            <dd className="mt-0.5 break-all font-mono">{entry.root_path}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Exists</dt>
            <dd className="mt-0.5">{entry.exists ? "yes" : "no"}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Raw</dt>
            <dd className="mt-0.5">
              {entry.raw_chars} chars · {entry.raw_lines} lines
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Rendered</dt>
            <dd className="mt-0.5">
              {entry.rendered_chars} chars · {entry.rendered_lines} lines ·{" "}
              {entry.tokens_estimate} est tokens
            </dd>
          </div>
        </dl>

        <DetailSection title="Include stats">
          <ul className="space-y-1 text-xs text-muted-foreground">
            <li>{`Includes: ${entry.include_count}`}</li>
            <li>{`Missing includes: ${entry.missing_includes.length}`}</li>
            <li>{`Cycles detected: ${entry.cycles_detected.length}`}</li>
            <li>{`Depth cap reached: ${entry.max_depth_reached ? "yes" : "no"}`}</li>
          </ul>
        </DetailSection>

        <DetailSection title="Top segments">
          {topSegments.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              No rendered segments.
            </p>
          ) : (
            <ul className="space-y-1 text-xs">
              {topSegments.map((segment) => (
                <li
                  key={`${segment.path}:${segment.depth}`}
                  className="rounded-md bg-muted/20 p-2"
                >
                  <p className="truncate font-mono">{segment.path}</p>
                  <p className="mt-0.5 text-[11px] text-muted-foreground">
                    {segment.tokens_estimate} est tokens · {segment.chars} chars
                    · depth {segment.depth}
                  </p>
                </li>
              ))}
            </ul>
          )}
        </DetailSection>

        <DetailSection title="Diagnostics">
          <DetailStringList
            items={entry.diagnostics}
            emptyText="No diagnostics."
          />
        </DetailSection>
      </DetailContent>
    </>
  );
}
