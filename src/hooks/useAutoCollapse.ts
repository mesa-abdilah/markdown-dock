import { useCallback, useEffect, useRef } from "react";
import { useDockStore } from "../stores/dockStore";
import { useSearchStore } from "../stores/searchStore";

const PEEK_DWELL_MS = 45;
const PEEK_LEAVE_GRACE_MS = 350;
const EXPANDED_LEAVE_GRACE_MS = 600;
const TYPING_GUARD_MS = 2000;

export function useAutoCollapse() {
  const hoverDwellTimer = useRef<number | null>(null);
  const leaveGraceTimer = useRef<number | null>(null);
  const isPointerInside = useRef(false);

  const clearTimers = useCallback(() => {
    if (hoverDwellTimer.current !== null) {
      window.clearTimeout(hoverDwellTimer.current);
      hoverDwellTimer.current = null;
    }
    if (leaveGraceTimer.current !== null) {
      window.clearTimeout(leaveGraceTimer.current);
      leaveGraceTimer.current = null;
    }
  }, []);

  const scheduleLeaveCollapse = useCallback((delayMs: number) => {
    const schedule = (ms: number) => {
      if (leaveGraceTimer.current !== null) {
        window.clearTimeout(leaveGraceTimer.current);
      }
      leaveGraceTimer.current = window.setTimeout(() => {
        leaveGraceTimer.current = null;
        if (isPointerInside.current) return;

        const { state, pinned, lastTypingAt, collapse } = useDockStore.getState();
        if (
          pinned ||
          state === "Collapsed" ||
          state === "Collapsing" ||
          useSearchStore.getState().dirty
        ) {
          return;
        }

        const sinceTyping = Date.now() - lastTypingAt;
        if (sinceTyping < TYPING_GUARD_MS) {
          schedule(TYPING_GUARD_MS - sinceTyping + 100);
          return;
        }
        void collapse();
      }, ms);
    };
    schedule(delayMs);
  }, []);

  const onPointerEnter = useCallback(() => {
    isPointerInside.current = true;
    clearTimers();

    const { state, preCollapseMode, expandTo } = useDockStore.getState();
    if (state === "Collapsing") {
      // Restore whichever mode was collapsing so hover never opens the full list unexpectedly.
      const restoreMode = preCollapseMode === "Peek" ? "Peek" : preCollapseMode;
      void expandTo(restoreMode, { focusWindow: restoreMode !== "Peek" });
      return;
    }
    if (state === "Collapsed") {
      hoverDwellTimer.current = window.setTimeout(() => {
        hoverDwellTimer.current = null;
        if (isPointerInside.current && useDockStore.getState().state === "Collapsed") {
          void expandTo("Peek", { focusWindow: false });
        }
      }, PEEK_DWELL_MS);
    }
  }, [clearTimers]);

  const onPointerLeave = useCallback(() => {
    isPointerInside.current = false;
    if (hoverDwellTimer.current !== null) {
      window.clearTimeout(hoverDwellTimer.current);
      hoverDwellTimer.current = null;
    }

    const { state, pinned } = useDockStore.getState();
    if (pinned || state === "Collapsed" || state === "Collapsing" || state === "Fullscreen") {
      return;
    }

    const grace = state === "Peek" ? PEEK_LEAVE_GRACE_MS : EXPANDED_LEAVE_GRACE_MS;
    scheduleLeaveCollapse(grace);
  }, [scheduleLeaveCollapse]);

  useEffect(() => {
    const onWindowBlur = () => {
      const { state, pinned, lastExpandAt, collapse } = useDockStore.getState();
      if (
        pinned ||
        state === "Fullscreen" ||
        useSearchStore.getState().dirty ||
        Date.now() - lastExpandAt < 450
      ) {
        return;
      }
      if (state === "Expanded" || state === "Wide") {
        clearTimers();
        void collapse();
      }
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("dmd-ctrl-f"));
        return;
      }

      if (e.key === "Escape") {
        const { state, collapse, expandTo, settingsOpen, toggleSettings } =
          useDockStore.getState();
        if (settingsOpen) {
          toggleSettings(false);
          return;
        }
        if (state === "Fullscreen") {
          clearTimers();
          void expandTo("Wide", { focusWindow: true });
          return;
        }
        if (state !== "Collapsed" && state !== "Collapsing") {
          clearTimers();
          void collapse({ force: true });
        }
      }
    };

    window.addEventListener("blur", onWindowBlur);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      clearTimers();
      window.removeEventListener("blur", onWindowBlur);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [clearTimers]);

  return { onPointerEnter, onPointerLeave };
}
