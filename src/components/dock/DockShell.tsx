import { AnimatePresence, motion } from "motion/react";
import { useAutoCollapse } from "../../hooks/useAutoCollapse";
import { useDragDrop } from "../../hooks/useDragDrop";
import { useReducedMotion } from "../../hooks/useReducedMotion";
import { REDUCED_MOTION_TRANSITION, SPRINGS } from "../../lib/springs";
import { useDockStore } from "../../stores/dockStore";
import { GlassSurface, type AccentTone } from "../ui/GlassSurface";
import { Capsule } from "./Capsule";
import { DropZoneOverlay } from "./DropZoneOverlay";
import { EdgeSliver } from "./EdgeSliver";
import { ExpandedPanel } from "./ExpandedPanel";

const SYNC_TONE: Record<"idle" | "syncing" | "locked" | "error", AccentTone> = {
  idle: "emerald",
  syncing: "indigo",
  locked: "amber",
  error: "rose",
};

export function DockShell() {
  const state = useDockStore((s) => s.state);
  const preCollapseMode = useDockStore((s) => s.preCollapseMode);
  const anchor = useDockStore((s) => s.anchor);
  const visualLogical = useDockStore((s) => s.visualLogical);
  const shadowPad = useDockStore((s) => s.shadowPad);
  const syncStatus = useDockStore((s) => s.syncStatus);
  const expandTo = useDockStore((s) => s.expandTo);
  const finalizeCollapse = useDockStore((s) => s.finalizeCollapse);

  const reducedMotion = useReducedMotion();
  const { onPointerEnter, onPointerLeave } = useAutoCollapse();
  useDragDrop();

  const isCollapsing = state === "Collapsing";
  const displayMode = isCollapsing ? preCollapseMode : state;
  const showSliver = state === "Collapsed" || isCollapsing;

  const enterTransition = reducedMotion ? REDUCED_MOTION_TRANSITION : SPRINGS.morph;
  const exitTransition = reducedMotion ? REDUCED_MOTION_TRANSITION : SPRINGS.exit;
  const contentTransition = reducedMotion ? REDUCED_MOTION_TRANSITION : SPRINGS.content;
  const activeTransition = isCollapsing ? exitTransition : enterTransition;

  const verticalEdgeStyle = {
    top: anchor === "TopRight" ? shadowPad : "auto",
    bottom: anchor === "BottomRight" ? shadowPad : "auto",
  } as const;

  const handleExitComplete = () => {
    if (useDockStore.getState().state === "Collapsing") {
      void finalizeCollapse();
    }
  };

  return (
    <div
      data-testid="dock-shell"
      data-dock-state={state}
      data-dock-anchor={anchor}
      className="pointer-events-none relative h-full w-full"
    >
      {/* Invisible 6px hover hit-padding beside the 7px edge sliver so cursor reaches PEEK effortlessly */}
      {state === "Collapsed" && (
        <div
          aria-hidden="true"
          onPointerEnter={onPointerEnter}
          onPointerLeave={onPointerLeave}
          onClick={() => {
            void expandTo("Expanded", { focusWindow: true });
          }}
          style={{
            position: "absolute",
            right: 7,
            width: 6,
            height: 80,
            ...verticalEdgeStyle,
          }}
          className="pointer-events-auto cursor-pointer bg-white/[0.004]"
        />
      )}

      {/* Stage 1: Minimalist Edge Sliver */}
      <AnimatePresence initial={false}>
        {showSliver && (
          <motion.div
            key="sliver-layer"
            onPointerEnter={onPointerEnter}
            onPointerLeave={onPointerLeave}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0, transition: { duration: 0.08 } }}
            transition={contentTransition}
            style={{
              position: "absolute",
              right: 0,
              width: 7,
              height: 80,
              borderTopLeftRadius: 10,
              borderBottomLeftRadius: 10,
              borderTopRightRadius: 0,
              borderBottomRightRadius: 0,
              ...verticalEdgeStyle,
            }}
            className="pointer-events-auto overflow-hidden"
          >
            <GlassSurface tone={SYNC_TONE[syncStatus]} className="rounded-[inherit]">
              <EdgeSliver
                onClickOpen={() => {
                  void expandTo("Expanded", { focusWindow: true });
                }}
              />
            </GlassSurface>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Stage 2 (Peek Capsule) & Stage 3 (Expanded / DropTarget): stationary hitboxes prevent enter/leave bounce */}
      {displayMode === "Peek" && (
        <div
          key="peek-capsule-hitbox"
          onPointerEnter={onPointerEnter}
          onPointerLeave={onPointerLeave}
          style={{
            position: "absolute",
            right: 0,
            width: 58,
            height: 96,
            ...verticalEdgeStyle,
          }}
          className="pointer-events-auto z-10 overflow-hidden rounded-l-[18px]"
        >
          <motion.div
            initial={{ opacity: 0, x: 16 }}
            animate={isCollapsing ? { opacity: 0, x: 16 } : { opacity: 1, x: 0 }}
            transition={activeTransition}
            onAnimationComplete={handleExitComplete}
            style={{ willChange: "transform, opacity" }}
            className="h-full w-full overflow-hidden rounded-l-[18px]"
          >
            <GlassSurface tone={SYNC_TONE[syncStatus]} className="rounded-[inherit]">
              <Capsule
                onClickOpen={() => {
                  void expandTo("Expanded", { focusWindow: true });
                }}
              />
            </GlassSurface>
          </motion.div>
        </div>
      )}

      {displayMode === "DropTarget" && (
        <div
          key="drop-target-hitbox"
          onPointerEnter={onPointerEnter}
          onPointerLeave={onPointerLeave}
          style={{
            position: "absolute",
            right: shadowPad,
            width: visualLogical.width ?? 240,
            height: visualLogical.height ?? 320,
            ...verticalEdgeStyle,
          }}
          className="pointer-events-auto z-20 overflow-hidden rounded-[22px]"
        >
          <motion.div
            initial={{ opacity: 0, x: 18 }}
            animate={isCollapsing ? { opacity: 0, x: 18 } : { opacity: 1, x: 0 }}
            transition={activeTransition}
            onAnimationComplete={handleExitComplete}
            style={{ willChange: "transform, opacity" }}
            className="h-full w-full overflow-hidden rounded-[22px]"
          >
            <GlassSurface tone={SYNC_TONE[syncStatus]} className="rounded-[inherit]">
              <DropZoneOverlay />
            </GlassSurface>
          </motion.div>
        </div>
      )}

      {(displayMode === "Expanded" ||
        displayMode === "Wide" ||
        displayMode === "Fullscreen") && (
        <div
          key="expanded-panel-hitbox"
          onPointerEnter={onPointerEnter}
          onPointerLeave={onPointerLeave}
          style={{
            position: "absolute",
            right: shadowPad,
            width: visualLogical.width ?? 460,
            height: visualLogical.height ?? 680,
            borderRadius: displayMode === "Fullscreen" ? 16 : 22,
            ...verticalEdgeStyle,
          }}
          className="pointer-events-auto z-20 overflow-hidden"
        >
          <motion.div
            initial={{ opacity: 0, x: 22 }}
            animate={isCollapsing ? { opacity: 0, x: 22 } : { opacity: 1, x: 0 }}
            transition={activeTransition}
            onAnimationComplete={handleExitComplete}
            style={{
              borderRadius: displayMode === "Fullscreen" ? 16 : 22,
              willChange: "transform, opacity",
            }}
            className="h-full w-full overflow-hidden"
          >
            <GlassSurface tone={SYNC_TONE[syncStatus]} className="rounded-[inherit]">
              <ExpandedPanel />
            </GlassSurface>
          </motion.div>
        </div>
      )}
    </div>
  );
}
