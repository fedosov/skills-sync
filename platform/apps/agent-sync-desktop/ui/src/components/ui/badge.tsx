import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-sm border px-1.5 py-0.5 text-[10px] font-medium leading-none",
  {
    variants: {
      variant: {
        default: "border-border/55 bg-muted/55 text-foreground",
        outline: "border-border/60 bg-transparent text-muted-foreground",
        success:
          "border-emerald-600/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
        warning:
          "border-amber-600/35 bg-amber-500/10 text-amber-700 dark:text-amber-300",
        error:
          "border-rose-600/35 bg-rose-500/10 text-rose-700 dark:text-rose-300",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge };
