import { useCallback, useEffect, useState } from "react";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { formatIsoTime } from "../lib/formatting";
import {
  parseAuditStatusFilter,
  type AuditStatusFilter,
} from "../lib/catalogUtils";
import { clearAuditEvents, listAuditEvents } from "../tauriApi";
import type { AuditEvent } from "../types";
import { errorMessage } from "../lib/utils";

type AuditLogDialogProps = {
  onClose: () => void;
  onError: (message: string) => void;
};

export function AuditLogDialog({ onClose, onError }: AuditLogDialogProps) {
  const [auditEvents, setAuditEvents] = useState<AuditEvent[]>([]);
  const [auditStatusFilter, setAuditStatusFilter] =
    useState<AuditStatusFilter>("all");
  const [auditActionFilter, setAuditActionFilter] = useState("");
  const [auditBusy, setAuditBusy] = useState(false);
  const [clearDialogOpen, setClearDialogOpen] = useState(false);
  const [clearBusy, setClearBusy] = useState(false);

  const loadAudit = useCallback(async () => {
    setAuditBusy(true);
    try {
      const next = await listAuditEvents({
        limit: 200,
        status: auditStatusFilter === "all" ? undefined : auditStatusFilter,
        action: auditActionFilter,
      });
      setAuditEvents(next);
    } catch (invokeError) {
      onError(errorMessage(invokeError));
    } finally {
      setAuditBusy(false);
    }
  }, [auditActionFilter, auditStatusFilter, onError]);

  useEffect(() => {
    void loadAudit();
  }, [loadAudit]);

  async function handleConfirmClear() {
    setClearBusy(true);
    try {
      await clearAuditEvents();
      await loadAudit();
      setClearDialogOpen(false);
    } catch (invokeError) {
      onError(errorMessage(invokeError));
    } finally {
      setClearBusy(false);
    }
  }

  return (
    <>
      <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4">
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Audit log"
          className="flex h-[80vh] w-full max-w-4xl flex-col rounded-md border border-border/70 bg-card p-4"
        >
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-sm font-semibold">Audit log</h2>
            <Button size="sm" variant="ghost" onClick={onClose}>
              Close
            </Button>
          </div>
          <div className="mt-3 flex flex-wrap items-end gap-2">
            <label
              className="text-xs text-muted-foreground"
              htmlFor="audit-status-filter"
            >
              Status
              <select
                id="audit-status-filter"
                aria-label="Audit status filter"
                className="mt-1 block rounded-md border border-border/70 bg-background px-2 py-1 text-xs"
                value={auditStatusFilter}
                onChange={(event) =>
                  setAuditStatusFilter(
                    parseAuditStatusFilter(event.currentTarget.value),
                  )
                }
              >
                <option value="all">all</option>
                <option value="success">success</option>
                <option value="failed">failed</option>
                <option value="blocked">blocked</option>
              </select>
            </label>
            <label
              className="text-xs text-muted-foreground"
              htmlFor="audit-action-filter"
            >
              Action
              <Input
                id="audit-action-filter"
                aria-label="Audit action filter"
                value={auditActionFilter}
                placeholder="run_sync"
                onChange={(event) =>
                  setAuditActionFilter(event.currentTarget.value)
                }
                className="mt-1 min-w-[220px]"
              />
            </label>
            <Button
              size="sm"
              variant="outline"
              disabled={auditBusy}
              onClick={() => void loadAudit()}
            >
              Apply
            </Button>
            <Button
              size="sm"
              variant="destructive"
              disabled={auditBusy || clearBusy}
              onClick={() => setClearDialogOpen(true)}
            >
              Clear logs
            </Button>
          </div>
          <div className="mt-3 min-h-0 flex-1 overflow-auto rounded-md border border-border/50">
            {auditEvents.length === 0 ? (
              <p className="p-3 text-xs text-muted-foreground">
                No audit events.
              </p>
            ) : (
              <ul className="space-y-1 p-2">
                {auditEvents.map((event) => (
                  <li
                    key={event.id}
                    className="rounded-md border border-border/40 bg-muted/20 p-2"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span className="font-mono text-[11px]">
                        {formatIsoTime(event.occurred_at)}
                      </span>
                      <Badge
                        variant={
                          event.status === "success"
                            ? "success"
                            : event.status === "blocked"
                              ? "warning"
                              : "error"
                        }
                      >
                        {event.status}
                      </Badge>
                    </div>
                    <p className="mt-1 text-xs font-medium">
                      {event.action}
                      {event.trigger ? ` (${event.trigger})` : ""}
                    </p>
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      {event.summary}
                    </p>
                    {event.paths.length > 0 ? (
                      <p className="mt-1 truncate font-mono text-[11px]">
                        {event.paths.join(" · ")}
                      </p>
                    ) : null}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </div>

      {clearDialogOpen ? (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Clear audit logs"
            className="w-full max-w-sm rounded-md border border-border/70 bg-card p-4"
          >
            <h2 className="text-sm font-semibold">Clear audit logs</h2>
            <p className="mt-2 text-xs text-muted-foreground">
              Remove all audit events from the log?
            </p>
            <div className="mt-3 flex items-center justify-end gap-2">
              <Button
                size="sm"
                variant="ghost"
                disabled={clearBusy}
                onClick={() => setClearDialogOpen(false)}
              >
                Cancel
              </Button>
              <Button
                size="sm"
                variant="destructive"
                disabled={clearBusy}
                onClick={() => void handleConfirmClear()}
              >
                Confirm
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
