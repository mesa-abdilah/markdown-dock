import { useEffect, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauriRuntime } from "../lib/ipc";
import { useDockStore } from "../stores/dockStore";
import { useSearchStore } from "../stores/searchStore";

const DRAG_LEAVE_GRACE_MS = 300;

function isMarkdownPath(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith(".md") || lower.endsWith(".markdown");
}

export function useDragDrop() {
  const leaveTimer = useRef<number | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let active = true;
    let unlisten: (() => void) | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!active) return;
        const payload = event.payload;

        if (payload.type === "enter" || payload.type === "over") {
          if (leaveTimer.current !== null) {
            window.clearTimeout(leaveTimer.current);
            leaveTimer.current = null;
          }
          const current = useDockStore.getState().state;
          if (current === "Collapsed" || current === "Peek" || current === "Collapsing") {
            void useDockStore.getState().expandTo("DropTarget", { focusWindow: false });
          }
        } else if (payload.type === "leave") {
          if (leaveTimer.current !== null) {
            window.clearTimeout(leaveTimer.current);
          }
          leaveTimer.current = window.setTimeout(() => {
            leaveTimer.current = null;
            if (useDockStore.getState().state === "DropTarget") {
              void useDockStore.getState().collapse();
            }
          }, DRAG_LEAVE_GRACE_MS);
        } else if (payload.type === "drop") {
          if (leaveTimer.current !== null) {
            window.clearTimeout(leaveTimer.current);
            leaveTimer.current = null;
          }
          const paths = payload.paths ?? [];
          const mdPaths = paths.filter(isMarkdownPath);
          useDockStore.getState().setDroppedPaths(mdPaths);

          if (paths.length > 0) {
            void useSearchStore.getState().uploadFiles(paths);
          } else {
            void useDockStore.getState().collapse();
          }
        }
      })
      .then((fn) => {
        if (active) {
          unlisten = fn;
        } else {
          fn();
        }
      });

    return () => {
      active = false;
      if (leaveTimer.current !== null) {
        window.clearTimeout(leaveTimer.current);
      }
      unlisten?.();
    };
  }, []);
}
