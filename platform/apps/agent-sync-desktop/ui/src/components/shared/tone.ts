export type OutputTone = "neutral" | "warning" | "danger";

export function toneClass(tone: OutputTone): string {
  switch (tone) {
    case "neutral":
      return "border-border/70 bg-muted/40 text-foreground";
    case "warning":
      return "border-amber-600/25 bg-amber-500/10 text-amber-800 dark:text-amber-300";
    case "danger":
      return "border-destructive/25 bg-destructive/10 text-destructive";
  }
}
