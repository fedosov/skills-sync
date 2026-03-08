import type { ReactNode } from "react";
import { compactPath } from "../../lib/formatting";
import { Button } from "../ui/button";
import { CardContent } from "../ui/card";

export function DetailContent({ children }: { children: ReactNode }) {
  return (
    <CardContent className="space-y-3 p-3 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
      {children}
    </CardContent>
  );
}

export function DetailSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="space-y-1.5 border-t border-border/50 pt-3">
      <h3 className="text-xs font-semibold text-muted-foreground">{title}</h3>
      {children}
    </section>
  );
}

export function DetailStringList({
  items,
  emptyText,
  renderItem,
}: {
  items: string[];
  emptyText: string;
  renderItem?: (item: string) => ReactNode;
}) {
  if (items.length === 0) {
    return <p className="text-xs text-muted-foreground">{emptyText}</p>;
  }

  return (
    <ul className="space-y-1 text-xs">
      {items.map((item) => (
        <li key={item} className="rounded-md bg-muted/20 p-2 font-mono">
          {renderItem ? renderItem(item) : item}
        </li>
      ))}
    </ul>
  );
}

export function DetailPreviewSection({
  title,
  preview,
  emptyText,
  maxHeightClass = "max-h-64",
}: {
  title: string;
  preview: string | null;
  emptyText: string;
  maxHeightClass?: string;
}) {
  return (
    <DetailSection title={title}>
      {preview ? (
        <pre
          className={`${maxHeightClass} overflow-auto rounded-md bg-muted/30 p-2 font-mono text-[11px] leading-relaxed`}
        >
          {preview}
        </pre>
      ) : (
        <p className="text-xs text-muted-foreground">{emptyText}</p>
      )}
    </DetailSection>
  );
}

export function DetailPathValue({
  path,
  copyAriaLabel,
  onCopy,
}: {
  path: string;
  copyAriaLabel?: string;
  onCopy?: () => void;
}) {
  return (
    <dd className="mt-0.5 flex items-center gap-2 font-mono">
      <span title={path}>{compactPath(path)}</span>
      {onCopy ? (
        <Button
          size="sm"
          variant="ghost"
          aria-label={copyAriaLabel}
          onClick={onCopy}
        >
          Copy
        </Button>
      ) : null}
    </dd>
  );
}
