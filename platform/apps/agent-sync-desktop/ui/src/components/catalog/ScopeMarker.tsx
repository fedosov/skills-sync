export function ScopeMarker({ scope }: { scope: string }) {
  const scopeLabel = scope === "global" ? "Global" : "Project";

  return (
    <span className="text-[10px] text-muted-foreground">{scopeLabel}</span>
  );
}
