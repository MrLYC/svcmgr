import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

type StatusType = "running" | "stopped" | "failed" | "connected" | "disconnected" | "degraded" | "active" | "inactive" | "error" | "modified" | "added" | "deleted" | "clean" | "dirty";

const statusStyles: Record<string, string> = {
  running: "bg-success text-success-foreground",
  connected: "bg-success text-success-foreground",
  active: "bg-success text-success-foreground",
  clean: "bg-success text-success-foreground",
  added: "bg-success text-success-foreground",
  stopped: "bg-muted text-muted-foreground",
  disconnected: "bg-muted text-muted-foreground",
  inactive: "bg-muted text-muted-foreground",
  failed: "bg-destructive text-destructive-foreground",
  error: "bg-destructive text-destructive-foreground",
  deleted: "bg-destructive text-destructive-foreground",
  degraded: "bg-warning text-warning-foreground",
  dirty: "bg-warning text-warning-foreground",
  modified: "bg-info text-info-foreground",
};

export function StatusBadge({ status, className }: { status: StatusType; className?: string }) {
  return (
    <Badge className={cn("border-0 capitalize", statusStyles[status] || "", className)}>
      {status}
    </Badge>
  );
}

export function StatusDot({ status }: { status: "connected" | "disconnected" | "degraded" }) {
  const colors: Record<string, string> = {
    connected: "bg-success",
    disconnected: "bg-destructive",
    degraded: "bg-warning",
  };
  return <span className={cn("inline-block h-2 w-2 rounded-full", colors[status])} />;
}
