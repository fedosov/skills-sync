import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center whitespace-nowrap rounded-sm border text-[12px] font-medium leading-none transition-colors duration-150 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default:
          "border-primary/55 bg-primary text-primary-foreground hover:bg-primary/88",
        ghost:
          "border-transparent text-foreground hover:bg-accent/80 hover:text-accent-foreground",
        outline:
          "border-border/70 bg-transparent text-foreground hover:bg-accent/80 hover:text-accent-foreground",
        destructive:
          "border-destructive/55 bg-destructive text-destructive-foreground hover:bg-destructive/88",
      },
      size: {
        default: "h-[var(--control-height)] px-3",
        sm: "h-[var(--control-height-sm)] px-2.5 text-[11px]",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button };
