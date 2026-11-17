import { AlertTriangle, Lock, LockOpen, RefreshCw } from "lucide-react";
import { cn } from "../../lib/cn";
import type { SyncIndicatorStatus } from "../../stores/dockStore";

const STATUS_STYLE: Record<
  SyncIndicatorStatus,
  { dot: string; badge: string; text: string; label: string }
> = {
  idle: {
    dot: "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.75)]",
    badge:
      "bg-gradient-to-b from-emerald-400/25 to-emerald-500/15 text-emerald-300 ring-1 ring-emerald-400/45 shadow-[0_0_10px_rgba(52,211,153,0.35)]",
    text: "text-emerald-200/90",
    label: "Vault unlocked & sync ready",
  },
  syncing: {
    dot: "bg-indigo-400 animate-pulse shadow-[0_0_8px_rgba(129,140,248,0.8)]",
    badge:
      "bg-gradient-to-b from-indigo-400/30 to-indigo-500/15 text-indigo-200 ring-1 ring-indigo-400/50 shadow-[0_0_10px_rgba(129,140,248,0.45)]",
    text: "text-indigo-200",
    label: "Syncing encrypted backup...",
  },
  locked: {
    dot: "bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.65)]",
    badge:
      "bg-gradient-to-b from-amber-400/25 to-amber-500/15 text-amber-300 ring-1 ring-amber-400/50 shadow-[0_0_10px_rgba(251,191,36,0.35)]",
    text: "text-amber-200/95",
    label: "Vault locked — unlock to backup or restore",
  },
  error: {
    dot: "bg-rose-400 shadow-[0_0_8px_rgba(251,113,133,0.75)]",
    badge:
      "bg-gradient-to-b from-rose-400/25 to-rose-500/15 text-rose-300 ring-1 ring-rose-400/50 shadow-[0_0_10px_rgba(251,113,133,0.4)]",
    text: "text-rose-200",
    label: "Sync error",
  },
};

export function StatusIndicator({
  status,
  showLabel = false,
  variant = "badge",
  className,
}: {
  status: SyncIndicatorStatus;
  showLabel?: boolean;
  variant?: "badge" | "dot";
  className?: string;
}) {
  const info = STATUS_STYLE[status];

  const renderIcon = () => {
    if (status === "idle") return <LockOpen className="h-2.5 w-2.5" strokeWidth={2.3} />;
    if (status === "locked") return <Lock className="h-2.5 w-2.5" strokeWidth={2.3} />;
    if (status === "syncing")
      return <RefreshCw className="h-2.5 w-2.5 animate-spin" strokeWidth={2.3} />;
    return <AlertTriangle className="h-2.5 w-2.5" strokeWidth={2.3} />;
  };

  return (
    <span
      className={cn("inline-flex items-center gap-1.5", className)}
      title={info.label}
      aria-label={info.label}
    >
      {variant === "dot" ? (
        <span className={cn("h-2 w-2 rounded-full", info.dot)} />
      ) : (
        <span
          className={cn(
            "relative inline-flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full",
            info.badge,
          )}
        >
          {renderIcon()}
          {status === "locked" && (
            <span
              className="absolute -top-0.5 -right-0.5 h-1.5 w-1.5 rounded-full bg-amber-400 ring-1 ring-zinc-950"
              aria-hidden="true"
            />
          )}
        </span>
      )}
      {showLabel && (
        <span className={cn("text-[11px] font-medium", info.text)}>{info.label}</span>
      )}
    </span>
  );
}
