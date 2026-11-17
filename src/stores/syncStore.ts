import { create } from "zustand";
import type { RestoreCatalogItem, RestoreSelection } from "../bindings/commands";
import {
  defaultDockIpc,
  type ConflictInfo,
  type DockIpc,
  type SyncStatusEvent,
  type SyncStatusSnapshot,
} from "../lib/ipc";
import { useDockStore } from "./dockStore";
import { useSearchStore } from "./searchStore";
import { useVaultStore } from "./vaultStore";

let activeIpc: DockIpc = defaultDockIpc;

export function setSyncIpcForTests(ipc: DockIpc) {
  activeIpc = ipc;
}

export interface SyncStoreState {
  status: SyncStatusSnapshot;
  conflicts: ConflictInfo[];
  restoreCatalog: RestoreCatalogItem[];
  busy: boolean;
  lastMessage: string | null;
  error: string | null;
  modalMode: "none" | "restore";

  loadStatus: () => Promise<void>;
  connectDrive: () => Promise<boolean>;
  disconnectDrive: () => Promise<void>;
  importRemoteKeyfileIfPresent: () => Promise<boolean>;
  syncNow: () => Promise<void>;
  pullChanges: () => Promise<void>;
  loadConflicts: () => Promise<void>;
  resolveConflict: (conflictId: number, resolution: "keep_both" | "keep_local" | "keep_remote") => Promise<void>;
  loadRestoreCatalog: () => Promise<void>;
  runRestore: (selections: RestoreSelection[], targetDir: string, overwrite: boolean) => Promise<void>;
  openModal: (mode: "none" | "restore") => void;
  applyStatusEvent: (ev: SyncStatusEvent) => void;
  applyConnectionEvent: (connected: boolean) => void;
  clearFeedback: () => void;
}

function syncDockDot(status: SyncStatusSnapshot) {
  const vaultState = useVaultStore.getState().status.state;
  if (vaultState !== "Unlocked") {
    useDockStore.setState({ syncStatus: "locked" });
    return;
  }
  if (status.phase === "Syncing" || status.phase === "Pulling") {
    useDockStore.setState({ syncStatus: "syncing" });
  } else if (status.phase === "Error" || status.phase === "Offline") {
    useDockStore.setState({ syncStatus: "error" });
  } else {
    useDockStore.setState({ syncStatus: "idle" });
  }
}

export const useSyncStore = create<SyncStoreState>((set, get) => ({
  status: {
    connected: false,
    phase: "Idle",
    queued: 0,
    synced: 0,
    conflicts: 0,
    last_ok_sec: null,
    last_error: null,
  },
  conflicts: [],
  restoreCatalog: [],
  busy: false,
  lastMessage: null,
  error: null,
  modalMode: "none",

  async loadStatus() {
    try {
      const status = await activeIpc.syncStatus();
      set({ status });
      syncDockDot(status);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async connectDrive() {
    set({ busy: true, error: null, lastMessage: null });
    try {
      await activeIpc.driveConnect();
      await get().loadStatus();
      set({ busy: false, lastMessage: "Connected to Google Drive" });
      return true;
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
      return false;
    }
  },

  async disconnectDrive() {
    set({ busy: true, error: null });
    try {
      await activeIpc.driveDisconnect();
      await get().loadStatus();
      set({ busy: false, lastMessage: "Disconnected from Google Drive" });
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
    }
  },

  async importRemoteKeyfileIfPresent() {
    set({ busy: true, error: null });
    try {
      const exists = await activeIpc.driveCheckRemoteKeyfile();
      if (!exists) {
        set({ busy: false, error: "No existing Vault found on Google Drive" });
        return false;
      }
      await activeIpc.driveImportRemoteKeyfile();
      await useVaultStore.getState().loadStatus();
      set({
        busy: false,
        lastMessage: "Imported Vault keyfile from Drive. Unlock with your Master Password.",
      });
      return true;
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
      return false;
    }
  },

  async syncNow() {
    set({ busy: true, error: null, lastMessage: null });
    try {
      const report = await activeIpc.syncNow();
      await get().loadStatus();
      await get().loadConflicts();
      set({
        busy: false,
        lastMessage: `Backed up ${report.uploaded} file(s), skipped ${report.skipped} unchanged.`,
      });
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
    }
  },

  async pullChanges() {
    set({ busy: true, error: null, lastMessage: null });
    try {
      const report = await activeIpc.syncPull();
      await get().loadStatus();
      await useSearchStore.getState().loadRoots();
      await useSearchStore.getState().runSearch();
      set({
        busy: false,
        lastMessage: `Pulled ${report.pulled} file(s), ${report.conflict_copies} conflict copy(ies).`,
      });
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
    }
  },

  async loadConflicts() {
    try {
      const conflicts = await activeIpc.conflictsList(true);
      set({ conflicts });
    } catch {
      // ignore
    }
  },

  async resolveConflict(conflictId, resolution) {
    try {
      await activeIpc.conflictsResolve(conflictId, resolution);
      await get().loadConflicts();
      await get().loadStatus();
    } catch (err) {
      set({ error: extractErr(err) });
    }
  },

  async loadRestoreCatalog() {
    set({ busy: true, error: null });
    try {
      const restoreCatalog = await activeIpc.restoreList();
      set({ restoreCatalog, busy: false });
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
    }
  },

  async runRestore(selections, targetDir, overwrite) {
    set({ busy: true, error: null, lastMessage: null });
    try {
      const report = await activeIpc.restoreRun(selections, targetDir, overwrite);
      set({
        busy: false,
        modalMode: "none",
        lastMessage: `Restored ${report.restored} file(s) (skipped ${report.skipped_existing} existing).`,
      });
    } catch (err) {
      set({ busy: false, error: extractErr(err) });
    }
  },

  openModal(mode) {
    set({ modalMode: mode, error: null });
    if (mode === "restore") {
      void get().loadRestoreCatalog();
    }
  },

  applyStatusEvent(ev) {
    set((s) => {
      const next: SyncStatusSnapshot = {
        ...s.status,
        phase: ev.phase,
        queued: ev.queued,
        synced: ev.synced,
        conflicts: ev.conflicts,
        last_ok_sec: ev.lastOkSec,
        last_error: ev.lastError,
      };
      syncDockDot(next);
      return { status: next };
    });
  },

  applyConnectionEvent(connected) {
    set((s) => ({ status: { ...s.status, connected } }));
  },

  clearFeedback() {
    set({ error: null, lastMessage: null });
  },
}));

function extractErr(err: unknown): string {
  if (typeof err === "object" && err !== null && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}
