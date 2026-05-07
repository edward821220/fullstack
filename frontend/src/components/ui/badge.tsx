import type { HTMLAttributes, PropsWithChildren } from "react";
import { cn } from "@/lib/utils";

type BadgeVariant = "default" | "secondary" | "success" | "warning" | "destructive";

const variantClasses: Record<BadgeVariant, string> = {
  default: "bg-primary/10 text-primary",
  secondary: "bg-secondary text-secondary-foreground",
  success: "bg-emerald-500/12 text-emerald-700",
  warning: "bg-amber-500/12 text-amber-700",
  destructive: "bg-destructive/12 text-destructive",
};

export function Badge({
  children,
  className,
  variant = "default",
  ...props
}: PropsWithChildren<HTMLAttributes<HTMLSpanElement>> & { variant?: BadgeVariant }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium",
        variantClasses[variant],
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
}
