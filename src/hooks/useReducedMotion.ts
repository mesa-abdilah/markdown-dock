import { useEffect, useState } from "react";
import { useSettingsStore } from "../stores/settingsStore";

export function useReducedMotion(): boolean {
  const override = useSettingsStore((s) => s.settings.reduced_motion_override);
  const [systemPrefers, setSystemPrefers] = useState<boolean>(() => {
    if (typeof window === "undefined" || !window.matchMedia) return false;
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  });

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = (e: MediaQueryListEvent) => setSystemPrefers(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  return override ?? systemPrefers;
}
