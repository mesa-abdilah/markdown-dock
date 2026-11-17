import { create } from "zustand";
import type { AnchorEdge, DockMode, LogicalSizeF } from "../bindings/commands";
import { defaultDockIpc, type DockIpc } from "../lib/ipc";

export type DockMachineState = DockMode | "Collapsing";
export type SyncIndicatorStatus = "idle" | "syncing" | "locked" | "error";

export interface DockNotice {
  id: number;
  text: string;
}

export interface DockStoreState {
  state: DockMachineState;
  preCollapseMode: Exclude<DockMode, "Collapsed">;
  anchor: AnchorEdge;
  pinned: boolean;
  seq: number;
  lastTypingAt: number;
  focusSearchSeq: number;
  settingsOpen: boolean;
  visualLogical: LogicalSizeF;
  shadowPad: number;
  verticalRatio: number;
  indexedCount: number;
  syncStatus: SyncIndicatorStatus;
  droppedPaths: string[];
  notice: DockNotice | null;

  hydrate: () => Promise<void>;
  expandTo: (target: Exclude<DockMode, "Collapsed">, opts?: { focusWindow?: boolean }) => Promise<void>;
  collapse: (opts?: { force?: boolean }) => Promise<void>;
  finalizeCollapse: () => Promise<void>;
  setPinned: (pinned: boolean) => Promise<void>;
  toggleSettings: (open?: boolean) => void;
  markTyping: () => void;
  requestSearchFocus: () => void;
  setDroppedPaths: (paths: string[]) => void;
  pushNotice: (text: string) => void;
  clearNotice: () => void;
  applyExternalModeEvent: (payload: {
    mode: DockMode;
    pinned: boolean;
    anchor: AnchorEdge;
    seq: number;
  }) => void;
}

let activeIpc: DockIpc = defaultDockIpc;

export function setDockIpcForTests(ipc: DockIpc) {
  activeIpc = ipc;
}

let noticeCounter = 0;

export const useDockStore = create<DockStoreState>((set, get) => ({
  state: "Collapsed",
  preCollapseMode: "Peek",
  anchor: "TopRight",
  pinned: false,
  seq: 0,
  lastTypingAt: 0,
  focusSearchSeq: 0,
  settingsOpen: false,
  visualLogical: { width: 58, height: 98 },
  shadowPad: 8,
  verticalRatio: 0.5,
  indexedCount: 0,
  syncStatus: "locked",
  droppedPaths: [],
  notice: null,

  async hydrate() {
    try {
      const snap = await activeIpc.dockGetState();
      set({
        state: snap.mode,
        anchor: snap.anchor,
        pinned: snap.pinned,
        seq: snap.seq,
        verticalRatio: snap.vertical_ratio ?? 0.5,
        visualLogical: {
          width: snap.plan.visual_logical.width ?? 58,
          height: snap.plan.visual_logical.height ?? 98,
        },
        shadowPad: snap.plan.shadow_pad ?? 6,
      });
    } catch (err) {
      get().pushNotice(`Dock init warning: ${String(err)}`);
    }
  },

  async expandTo(target, opts) {
    const current = get().state;
    const focusWindow =
      opts?.focusWindow ??
      (target === "Expanded" || target === "Wide" || target === "Fullscreen");

    // If interrupting a running collapse animation, the OS window is still expanded!
    // Reverse the spring immediately without shrinking/re-growing the OS window.
    if (current === "Collapsing") {
      const plan = await activeIpc.dockPlanTransition(target);
      set({
        anchor: plan.anchor,
        seq: plan.seq,
        visualLogical: {
          width: plan.visual_logical.width ?? 400,
          height: plan.visual_logical.height ?? 640,
        },
        shadowPad: plan.shadow_pad ?? 6,
      });
      await activeIpc.dockApply(plan.seq, focusWindow);
      set((s) => ({
        state: target,
        focusSearchSeq: focusWindow ? s.focusSearchSeq + 1 : s.focusSearchSeq,
      }));
      return;
    }

    // Same mode & just promoting focus (e.g. clicking inside Peek -> Expanded)
    if (current === target) {
      if (focusWindow) {
        set((s) => ({ focusSearchSeq: s.focusSearchSeq + 1 }));
      }
      return;
    }

    // Expand protocol:
    // 1. Plan transition (compute anchor + physical rect).
    // 2. Update CSS anchor while still visually collapsed (visual no-op).
    // 3. Apply OS window resize FIRST so content is never clipped.
    // 4. Switch state to target so Motion springs capsule -> panel.
    const plan = await activeIpc.dockPlanTransition(target);
    set({
      anchor: plan.anchor,
      seq: plan.seq,
      visualLogical: {
        width: plan.visual_logical.width ?? 400,
        height: plan.visual_logical.height ?? 640,
      },
      shadowPad: plan.shadow_pad ?? 6,
    });

    await activeIpc.dockApply(plan.seq, focusWindow);

    // Guard against newer transition overtaking this async call.
    if (get().seq !== plan.seq) return;

    set((s) => ({
      state: target,
      focusSearchSeq: focusWindow ? s.focusSearchSeq + 1 : s.focusSearchSeq,
    }));
  },

  async collapse(opts) {
    const { state, pinned } = get();
    if (state === "Collapsed" || state === "Collapsing") return;
    if (pinned && !opts?.force) return;

    if (opts?.force && pinned) {
      await get().setPinned(false);
    }

    // Collapse protocol:
    // 1. Remember pre-collapse mode so hover-interruption restores Peek vs Expanded accurately.
    // 2. Start spring animation back to capsule while window remains large.
    // 3. Wait for Motion onAnimationComplete -> finalizeCollapse() shrinks OS window.
    set({
      state: "Collapsing",
      preCollapseMode: state,
      settingsOpen: false,
      visualLogical: { width: 58, height: 98 },
    });
  },

  async finalizeCollapse() {
    // Only shrink OS window if collapse was not interrupted.
    if (get().state !== "Collapsing") return;

    try {
      const plan = await activeIpc.dockPlanTransition("Collapsed");
      if (get().state !== "Collapsing") return;

      set({
        seq: plan.seq,
        anchor: plan.anchor,
        visualLogical: {
          width: plan.visual_logical.width ?? 58,
          height: plan.visual_logical.height ?? 98,
        },
        shadowPad: plan.shadow_pad ?? 6,
      });

      await activeIpc.dockApply(plan.seq, false);
      if (get().state !== "Collapsing") return;

      set({ state: "Collapsed", droppedPaths: [] });
    } catch {
      // Stale seq or interrupted; safe to ignore.
    }
  },

  async setPinned(pinned) {
    const snap = await activeIpc.dockSetPinned(pinned);
    set({ pinned: snap.pinned });
  },

  toggleSettings(open) {
    set((s) => ({ settingsOpen: open ?? !s.settingsOpen }));
  },

  markTyping() {
    set({ lastTypingAt: Date.now() });
  },

  requestSearchFocus() {
    set((s) => ({ focusSearchSeq: s.focusSearchSeq + 1 }));
  },

  setDroppedPaths(paths) {
    set({ droppedPaths: paths });
  },

  pushNotice(text) {
    noticeCounter += 1;
    set({ notice: { id: noticeCounter, text } });
  },

  clearNotice() {
    set({ notice: null });
  },

  applyExternalModeEvent(payload) {
    const { state } = get();
    // Do not stomp an active Collapsing animation with intermediate events.
    if (state === "Collapsing" && payload.mode !== "Collapsed") {
      return;
    }
    set({
      pinned: payload.pinned,
      anchor: payload.anchor,
      seq: payload.seq,
    });
  },
}));
