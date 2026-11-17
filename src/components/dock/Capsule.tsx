import { useRef } from "react";
import { defaultDockIpc } from "../../lib/ipc";
import { useTheme } from "../../lib/themes";
import { useDockStore } from "../../stores/dockStore";
import { CountBadge } from "./CountBadge";
import { DockFolderIcon } from "./DockFolderIcon";
import { StatusIndicator } from "./StatusIndicator";

interface CapsuleProps {
  onClickOpen: () => void;
}

export function Capsule({ onClickOpen }: CapsuleProps) {
  const indexedCount = useDockStore((s) => s.indexedCount);
  const syncStatus = useDockStore((s) => s.syncStatus);
  const { peekColor } = useTheme();

  const dragRef = useRef<{
    active: boolean;
    startScreenY: number;
    startRatio: number;
    latestRatio: number;
    rafId: number | null;
  } | null>(null);

  const onGripPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    const startRatio = useDockStore.getState().verticalRatio;
    dragRef.current = {
      active: true,
      startScreenY: e.screenY,
      startRatio,
      latestRatio: startRatio,
      rafId: null,
    };
  };

  const onGripPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag?.active) return;

    const availH = Math.max(window.screen.availHeight || 900, 400);
    const deltaRatio = (e.screenY - drag.startScreenY) / availH;
    drag.latestRatio = Math.min(1, Math.max(0, drag.startRatio + deltaRatio));

    if (drag.rafId === null) {
      drag.rafId = window.requestAnimationFrame(() => {
        if (dragRef.current) {
          dragRef.current.rafId = null;
          useDockStore.setState({ verticalRatio: dragRef.current.latestRatio });
          void defaultDockIpc.dockSetVerticalRatio(dragRef.current.latestRatio, false);
        }
      });
    }
  };

  const onGripPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag?.active) return;
    drag.active = false;
    if (drag.rafId !== null) {
      window.cancelAnimationFrame(drag.rafId);
    }
    e.currentTarget.releasePointerCapture(e.pointerId);
    useDockStore.setState({ verticalRatio: drag.latestRatio });
    void defaultDockIpc.dockSetVerticalRatio(drag.latestRatio, true);
    dragRef.current = null;
  };

  return (
    <div
      role="button"
      tabIndex={0}
      aria-label="Open Dynamic Markdown Dock"
      onClick={onClickOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClickOpen();
        }
      }}
      className="group flex h-full w-full cursor-pointer flex-col items-center justify-between py-2 pr-1 pl-1.5 outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70"
    >
      {/* Subtle vertical drag grip */}
      <div
        role="slider"
        aria-label="Drag to reposition dock vertically"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(useDockStore.getState().verticalRatio * 100)}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onPointerDown={onGripPointerDown}
        onPointerMove={onGripPointerMove}
        onPointerUp={onGripPointerUp}
        onPointerCancel={onGripPointerUp}
        title="Drag to move along right screen edge"
        className="flex h-3 w-8 cursor-ns-resize items-center justify-center"
      >
        <span
          style={{
            backgroundColor: peekColor ?? "var(--text-muted)",
            boxShadow: peekColor ? `0 0 8px ${peekColor}` : undefined,
          }}
          className="h-[3px] w-4 rounded-full opacity-75 transition-all group-hover:w-5 group-hover:opacity-100"
        />
      </div>

      {/* Protruding Folder-with-Documents Icon */}
      <div className="flex items-center justify-center transition-transform duration-150 group-hover:-translate-x-0.5 group-hover:scale-[1.04]">
        <DockFolderIcon className="h-10 w-10 drop-shadow-[0_4px_10px_rgba(0,0,0,0.28)]" />
      </div>

      {/* Unified lock + count pill adapted to active theme & custom peek color */}
      <div
        style={{
          backgroundColor: "var(--bg-surface)",
          borderColor: peekColor ?? "var(--border-color)",
          borderWidth: "var(--border-width)",
          borderStyle: "solid",
          boxShadow: peekColor ? `0 0 10px ${peekColor}40` : "var(--shadow-button)",
        }}
        className="inline-flex items-center gap-0.5 rounded-full px-1 py-[3px] transition-all"
      >
        <StatusIndicator status={syncStatus} variant="badge" />
        <span
          style={{ backgroundColor: "var(--border-color)" }}
          className="h-2.5 w-px"
          aria-hidden="true"
        />
        <CountBadge count={indexedCount} variant="embedded" />
      </div>
    </div>
  );
}
