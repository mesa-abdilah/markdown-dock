import { AnimatePresence, motion } from "motion/react";
import { useAutoCollapse } from "../../hooks/useAutoCollapse";
import { useDragDrop } from "../../hooks/useDragDrop";
import { useReducedMotion } from "../../hooks/useReducedMotion";
import { REDUCED_MOTION_TRANSITION, SPRINGS } from "../../lib/springs";
import { useDockStore } from "../../stores/dockStore";
import { GlassSurface, type AccentTone } from "../ui/GlassSurface";
import { Capsule } from "./Capsule";
import { DropZoneOverlay } from "./DropZoneOverlay";
import { ExpandedPanel } from "./ExpandedPanel";
import { PeekCard } from "./PeekCard";

const SYNC_TONE: Record<"idle" | "syncing" | "locked" | "error", AccentTone> = {
  idle: "emerald",
  syncing: "indigo",
  locked: "amber",
  error: "rose",
};

export function DockShell() {
  const state = useDockStore((s) => s.state);
  const anchor = useDockStore((s) => s.anchor);
  const visualLogical = useDockStore((s) => s.visualLogical);
  const shadowPad = useDockStore((s) => s.shadowPad);
  const syncStatus = useDockStore((s) => s.syncStatus);
  const expandTo = useDockStore((s) => s.expandTo);
  const finalizeCollapse = useDockStore((s) => s.finalizeCollapse);

  const reducedMotion = useReducedMotion();
  const { onPointerEnter, onPointerLeave } = useAutoCollapse();
  useDragDrop();

  const isCollapsedVisual = state === "Collapsed" || state === "Collapsing";
  const targetWidth = isCollapsedVisual ? 58 : (visualLogical.width ?? 460);
  const targetHeight = isCollapsedVisual ? 98 : (visualLogical.height ?? 680);
  const targetRadius = isCollapsedVisual
    ? 18
    : state === "Peek"
      ? 24
      : state === "DropTarget"
        ? 22
        : state === "Fullscreen"
          ? 16
          : 22;
  const targetRight = isCollapsedVisual ? 0 : shadowPad;

  const morphTransition = reducedMotion ? REDUCED_MOTION_TRANSITION : SPRINGS.morph;
  const contentTransition = reducedMotion ? REDUCED_MOTION_TRANSITION : SPRINGS.content;

  const activeLayerKey = isCollapsedVisual
    ? "capsule"
    : state === "Peek"
      ? "peek"
      : state === "DropTarget"
        ? "drop-target"
        : "expanded";

  return (
    <div className="pointer-events-none relative h-full w-full">
      <motion.div
        data-testid="dock-shell"
        data-dock-state={state}
        data-dock-anchor={anchor}
        onPointerEnter={onPointerEnter}
        onPointerLeave={onPointerLeave}
        initial={false}
        animate={{
          width: targetWidth,
          height: targetHeight,
          right: targetRight,
          borderTopLeftRadius: targetRadius,
          borderBottomLeftRadius: targetRadius,
          borderTopRightRadius: isCollapsedVisual ? 0 : targetRadius,
          borderBottomRightRadius: isCollapsedVisual ? 0 : targetRadius,
        }}
        transition={morphTransition}
        onAnimationComplete={() => {
          if (useDockStore.getState().state === "Collapsing") {
            void finalizeCollapse();
          }
        }}
        style={{
          position: "absolute",
          top: anchor === "TopRight" ? shadowPad : "auto",
          bottom: anchor === "BottomRight" ? shadowPad : "auto",
          willChange: "width, height, border-radius, right",
        }}
        className="pointer-events-auto overflow-hidden"
      >
        <GlassSurface tone={SYNC_TONE[syncStatus]} className="rounded-[inherit]">
          {/* Simultaneous layered crossfade (no mode="wait" stall) so content flows with the spring */}
          <AnimatePresence initial={false}>
            {activeLayerKey === "capsule" && (
              <motion.div
                key="capsule"
                initial={{ opacity: 0, scale: 0.94 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.94 }}
                transition={contentTransition}
                className="absolute inset-0"
              >
                <Capsule
                  onClickOpen={() => {
                    void expandTo("Expanded", { focusWindow: true });
                  }}
                />
              </motion.div>
            )}

            {activeLayerKey === "peek" && (
              <motion.div
                key="peek"
                initial={{ opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.96 }}
                transition={contentTransition}
                className="absolute inset-0"
              >
                <PeekCard
                  onClickOpen={() => {
                    void expandTo("Expanded", { focusWindow: true });
                  }}
                />
              </motion.div>
            )}

            {activeLayerKey === "drop-target" && (
              <motion.div
                key="drop-target"
                initial={{ opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.96 }}
                transition={contentTransition}
                className="absolute inset-0"
              >
                <DropZoneOverlay />
              </motion.div>
            )}

            {activeLayerKey === "expanded" && (
              <motion.div
                key="expanded"
                initial={{ opacity: 0, scale: 0.97 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.97 }}
                transition={contentTransition}
                className="absolute inset-0"
              >
                <ExpandedPanel />
              </motion.div>
            )}
          </AnimatePresence>
        </GlassSurface>
      </motion.div>
    </div>
  );
}
