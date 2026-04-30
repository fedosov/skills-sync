import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { cn } from "../../lib/utils";
import { toneClass } from "./tone";

function formatDuration(durationMs: number): string {
  if (durationMs < 1000) {
    return `${durationMs} ms`;
  }
  return `${(durationMs / 1000).toFixed(2)} s`;
}

export type OutputCommand = {
  command: string;
  cwd: string;
  scope: string;
  exitCode: number | null;
  durationMs: number;
  stdout: string;
  stderr: string;
  success: boolean;
};

export function OutputPanel({
  lastCommand,
  emptyMessage = "Run a command to capture a transcript here.",
  subtitle = "Latest command transcript",
}: {
  lastCommand: OutputCommand | null;
  emptyMessage?: string;
  subtitle?: string;
}) {
  return (
    <section>
      <div className="mb-4">
        <h2 className="text-xl font-semibold text-foreground">Output</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">{subtitle}</p>
      </div>

      {!lastCommand ? (
        <div className="rounded-md border border-border/70 bg-card p-6 text-sm text-muted-foreground">
          {emptyMessage}
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[minmax(0,320px)_minmax(0,1fr)]">
          <Card>
            <CardHeader>
              <CardTitle>Transcript</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm">
              <div>
                <div className="mb-1 text-xs font-medium text-muted-foreground">
                  Command
                </div>
                <code className="block rounded-md border border-border/70 bg-muted/30 px-2.5 py-2 font-mono text-[12px]">
                  {lastCommand.command}
                </code>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Scope
                  </div>
                  <div className="text-foreground">{lastCommand.scope}</div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Exit code
                  </div>
                  <div className="text-foreground">
                    {lastCommand.exitCode ?? "not available"}
                  </div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Duration
                  </div>
                  <div className="text-foreground">
                    {formatDuration(lastCommand.durationMs)}
                  </div>
                </div>
                <div>
                  <div className="mb-1 text-xs font-medium text-muted-foreground">
                    Status
                  </div>
                  <div
                    className={cn(
                      "inline-flex rounded-sm border px-2 py-1 text-xs font-medium",
                      lastCommand.success
                        ? toneClass("neutral")
                        : toneClass("danger"),
                    )}
                  >
                    {lastCommand.success ? "success" : "failed"}
                  </div>
                </div>
              </div>
              <div>
                <div className="mb-1 text-xs font-medium text-muted-foreground">
                  Working directory
                </div>
                <code className="block rounded-md border border-border/70 bg-muted/30 px-2.5 py-2 font-mono text-[12px]">
                  {lastCommand.cwd}
                </code>
              </div>
            </CardContent>
          </Card>

          <div className="grid gap-4">
            <Card>
              <CardHeader>
                <CardTitle>stdout</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="min-h-[180px] overflow-x-auto whitespace-pre-wrap rounded-md border border-border/70 bg-muted/30 p-3 font-mono text-[12px] text-foreground">
                  {lastCommand.stdout || "No stdout output."}
                </pre>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>stderr</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="min-h-[180px] overflow-x-auto whitespace-pre-wrap rounded-md border border-border/70 bg-muted/30 p-3 font-mono text-[12px] text-foreground">
                  {lastCommand.stderr || "No stderr output."}
                </pre>
              </CardContent>
            </Card>
          </div>
        </div>
      )}
    </section>
  );
}
