import { create } from "zustand";
import type { MonitorInfo } from "../bindings/commands";
import { defaultDockIpc, type AppSettings, type SettingsPatch } from "../lib/ipc";

interface SettingsState {
  settings: AppSettings;
  monitors: MonitorInfo[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  refreshMonitors: () => Promise<void>;
  update: (patch: SettingsPatch) => Promise<void>;
  clearError: () => void;
}

const DEFAULT_SETTINGS: AppSettings = {
  monitor_policy: { kind: "Primary" },
  vertical_ratio: 0.5,
  hotkey: "Alt+Space",
  pinned_by_default: false,
  reduced_motion_override: null,
  autostart: false,
  google_client_id: "",
  google_client_secret: "",
  sync_retention_days: 30,
};

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: DEFAULT_SETTINGS,
  monitors: [],
  loading: false,
  error: null,

  async load() {
    set({ loading: true, error: null });
    try {
      const [settings, monitors] = await Promise.all([
        defaultDockIpc.settingsGet(),
        defaultDockIpc.listMonitors(),
      ]);
      set({ settings, monitors, loading: false });
    } catch (err) {
      set({ loading: false, error: String(err) });
    }
  },

  async refreshMonitors() {
    try {
      const monitors = await defaultDockIpc.listMonitors();
      set({ monitors });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async update(patch) {
    set({ error: null });
    try {
      const updated = await defaultDockIpc.settingsUpdate(patch);
      set({ settings: updated });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  clearError() {
    set({ error: null });
  },
}));
