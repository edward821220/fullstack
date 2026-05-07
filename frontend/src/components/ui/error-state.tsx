import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ErrorStateProps {
  title?: string;
  description?: string;
  onRetry?: () => void;
  className?: string;
}

export function ErrorState({
  className,
  description = "Something went wrong while loading data.",
  onRetry,
  title = "Error",
}: ErrorStateProps) {
  return (
    <div
      className={cn(
        "rounded-xl border border-destructive/20 bg-destructive/8 px-4 py-6 text-center",
        className,
      )}
    >
      <p className="text-sm font-medium text-destructive">{title}</p>
      <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      {onRetry && (
        <Button className="mt-3" onClick={onRetry} size="sm" variant="outline">
          Retry
        </Button>
      )}
    </div>
  );
}
