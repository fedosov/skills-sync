import { useEffect, useMemo, useState } from "react";
import { Button } from "../ui/button";
import type { RefreshIntervalMinutes } from "../../types";

type AppFooterProps = {
  nextRunAt: number | null;
  onRefreshNow: () => void;
  refreshIntervalMinutes: RefreshIntervalMinutes;
};

function formatRemainingMs(remainingMs: number): string {
  const totalSeconds = Math.max(0, Math.ceil(remainingMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export function AppFooter({
  nextRunAt,
  onRefreshNow,
  refreshIntervalMinutes,
}: AppFooterProps) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!nextRunAt) {
      return;
    }
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [nextRunAt]);

  const countdownLabel = useMemo(() => {
    if (!nextRunAt) {
      return "Manual refresh mode";
    }
    const remaining = Math.max(0, nextRunAt - now);
    return `Next sync in ${formatRemainingMs(remaining)}`;
  }, [nextRunAt, now]);

  return (
    <footer className="flex items-center justify-between border-t border-border/60 px-1 pt-2">
      <span className="text-xs text-muted-foreground tabular-nums">
        {countdownLabel}
      </span>
      <div className="flex items-center gap-2">
        {refreshIntervalMinutes === 0 ? (
          <span className="text-[11px] text-muted-foreground">Manual</span>
        ) : null}
        <Button size="sm" variant="ghost" onClick={onRefreshNow}>
          Refresh now
        </Button>
      </div>
    </footer>
  );
}
