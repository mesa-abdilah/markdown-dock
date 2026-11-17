import { cn } from "../../lib/cn";
import { useTheme } from "../../lib/themes";
import { useDockStore, type SyncIndicatorStatus } from "../../stores/dockStore";

interface EdgeSliverProps {
  onClickOpen: () => void;
}

const SLIVER_STYLE: Record<SyncIndicatorStatus, { bar: string; label: string }> = {
  idle: {
    bar: "bg-zinc-200/65 shadow-[0_0_8px_rgba(255,255,255,0.3)] group-hover:bg-white/95",
    label: "Vault unlocked & sync ready",
  },
  syncing: {
    bar: "animate-pulse bg-indigo-400/95 shadow-[0_0_12px_rgba(129,140,248,0.85)]",
    label: "Syncing encrypted backup...",
  },
  locked: {
    bar: "bg-amber-400/85 shadow-[0_0_10px_rgba(251,191,36,0.65)]",
    label: "Vault locked",
  },
  error: {
    bar: "bg-rose-400/90 shadow-[0_0_10px_rgba(251,113,133,0.75)]",
    label: "Sync error",
  },
};

/**
 * Stage 1 (Idle / Edge Peek Sliver): compact 7px glowing vertical edge-tab inside continuous GlassSurface.
 */
export function EdgeSliver({ onClickOpen }: EdgeSliverProps) {
  const syncStatus = useDockStore((s) => s.syncStatus);
  const indexedCount = useDockStore((s) => s.indexedCount);
  const { peekColor } = useTheme();
  const tone = SLIVER_STYLE[syncStatus];

  return (
    <div
      role="button"
      tabIndex={0}
      data-testid="dock-edge-sliver"
      aria-label={`Dynamic Markdown Dock (${indexedCount} files, ${tone.label})`}
      title={`Markdown Dock — Hover to peek (${indexedCount} files • ${tone.label})`}
      onClick={onClickOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClickOpen();
        }
      }}
      className="group flex h-full w-full cursor-pointer items-center justify-center py-2.5 outline-none"
    >
      <span
        data-testid="dock-edge-sliver-bar"
        aria-hidden="true"
        style={
          peekColor
            ? {
                backgroundColor: peekColor,
                boxShadow: `0 0 10px ${peekColor}`,
              }
            : undefined
        }
        className={cn(
          "h-full w-[3px] rounded-full transition-all duration-300",
          !peekColor && tone.bar,
          peekColor && syncStatus === "syncing" && "animate-pulse",
        )}
      />
    </div>
  );
}
