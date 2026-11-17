import { useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isTauriRuntime } from "../lib/ipc";

export function useTauriEvent<T>(
  subscribe: (handler: (event: { payload: T }) => void) => Promise<UnlistenFn>,
  handler: (payload: T) => void,
) {
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let active = true;
    let unlisten: UnlistenFn | undefined;

    subscribe((event) => {
      if (active) handler(event.payload);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [subscribe, handler]);
}
