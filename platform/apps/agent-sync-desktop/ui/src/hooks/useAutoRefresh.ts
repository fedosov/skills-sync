import { useEffect, useMemo, useState } from "react";
import type { RefreshIntervalMinutes } from "../types";

type UseAutoRefreshOptions = {
  enabled: boolean;
  intervalMinutes: RefreshIntervalMinutes;
  onRefresh: () => void | Promise<void>;
  resetSignal?: number;
};

type UseAutoRefreshResult = {
  nextRunAt: number | null;
};

function toMilliseconds(intervalMinutes: RefreshIntervalMinutes): number {
  return intervalMinutes * 60 * 1000;
}

function hasCatch(value: void | Promise<void>): value is Promise<void> {
  return !!value && typeof (value as { catch?: unknown }).catch === "function";
}

export function useAutoRefresh({
  enabled,
  intervalMinutes,
  onRefresh,
  resetSignal = 0,
}: UseAutoRefreshOptions): UseAutoRefreshResult {
  const intervalMs = useMemo(
    () => toMilliseconds(intervalMinutes),
    [intervalMinutes],
  );
  const [nextRunAt, setNextRunAt] = useState<number | null>(() =>
    enabled && intervalMs > 0 ? Date.now() + intervalMs : null,
  );

  useEffect(() => {
    if (!enabled || intervalMs <= 0) {
      queueMicrotask(() => setNextRunAt(null));
      return;
    }

    queueMicrotask(() => setNextRunAt(Date.now() + intervalMs));

    const timer = window.setInterval(() => {
      try {
        const refreshResult = onRefresh();
        if (hasCatch(refreshResult)) {
          void refreshResult.catch(() => {
            // Prevent unhandled promise rejections from periodic callbacks.
          });
        }
      } catch {
        // Ignore sync refresh errors to keep the interval loop active.
      }
      setNextRunAt(Date.now() + intervalMs);
    }, intervalMs);

    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, intervalMs, onRefresh, resetSignal]);

  return { nextRunAt };
}
