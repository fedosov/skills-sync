import { Button } from "./ui/button";
import { Card, CardContent } from "./ui/card";
import {
  isFixableSyncWarning,
  syncWarningFixSummary,
} from "../lib/catalogUtils";
import type { RuntimeControls } from "../types";

function renderSyncWarningText(warning: string) {
  const term = "central catalog";
  const replacement = "Central Catalog (~/.config/ai-agents/config.toml)";
  const index = warning.indexOf(term);
  if (index === -1) {
    return warning;
  }

  const before = warning.slice(0, index);
  const after = warning.slice(index + term.length);
  return (
    <>
      {before}
      <code className="font-mono text-[11px]">{replacement}</code>
      {after}
    </>
  );
}

type SyncWarningsBannerProps = {
  syncWarnings: string[];
  syncWarningsExpanded: boolean;
  onToggleExpanded: () => void;
  fixingSyncWarning: string | null;
  busy: boolean;
  runtimeControls: RuntimeControls | null;
  onFixWarning: (warning: string) => void;
};

export function SyncWarningsBanner({
  syncWarnings,
  syncWarningsExpanded,
  onToggleExpanded,
  fixingSyncWarning,
  busy,
  runtimeControls,
  onFixWarning,
}: SyncWarningsBannerProps) {
  if (syncWarnings.length === 0) {
    return null;
  }

  return (
    <Card
      className="shrink-0 border-amber-500/40 bg-amber-500/10"
      data-testid="sync-warning-banner"
    >
      <CardContent className="space-y-2 p-2 text-xs text-foreground">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span className="font-medium">
            {`Sync warnings (${syncWarnings.length})`}
          </span>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="h-6 px-2 text-[11px]"
            onClick={onToggleExpanded}
          >
            {syncWarningsExpanded ? "Hide warnings" : "Show warnings"}
          </Button>
        </div>
        {syncWarningsExpanded ? (
          <ul className="space-y-1">
            {syncWarnings.map((warning) => (
              <li
                key={warning}
                className="rounded-md border border-amber-600/35 bg-amber-500/15 p-2 text-foreground"
              >
                <div className="flex items-start justify-between gap-2">
                  <span className="min-w-0 flex-1 break-words">
                    {renderSyncWarningText(warning)}
                  </span>
                  {isFixableSyncWarning(warning) ? (
                    <div className="flex shrink-0 items-center gap-2 pl-2">
                      <span className="max-w-[220px] text-right text-[11px] font-medium leading-tight text-foreground/90">
                        {syncWarningFixSummary(warning)}
                      </span>
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        className="h-6 shrink-0 border-amber-600/45 bg-card/70 px-2 text-[11px] text-foreground hover:bg-amber-500/20"
                        disabled={
                          busy ||
                          !runtimeControls?.allow_filesystem_changes ||
                          fixingSyncWarning !== null
                        }
                        onClick={() => onFixWarning(warning)}
                      >
                        {fixingSyncWarning === warning ? "Fixing..." : "Fix"}
                      </Button>
                    </div>
                  ) : null}
                </div>
              </li>
            ))}
          </ul>
        ) : null}
      </CardContent>
    </Card>
  );
}
