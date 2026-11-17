import { create } from "zustand";
import type {
  AeadAlg,
  VaultLockState,
  VaultSetupPending,
} from "../bindings/commands";
import {
  defaultDockIpc,
  type DockIpc,
  type PasswordStrength,
  type VaultStatus,
} from "../lib/ipc";
import { useDockStore } from "./dockStore";

let activeIpc: DockIpc = defaultDockIpc;

export function setVaultIpcForTests(ipc: DockIpc) {
  activeIpc = ipc;
}

export interface VaultStoreState {
  status: VaultStatus;
  pendingSetup: VaultSetupPending | null;
  strength: PasswordStrength | null;
  busy: boolean;
  error: string | null;
  modalMode: "none" | "setup" | "unlock" | "change-password";

  loadStatus: () => Promise<void>;
  checkStrength: (password: string) => Promise<void>;
  beginSetup: (password: string, aead: AeadAlg, remember: boolean) => Promise<boolean>;
  confirmSetup: (suffixInput: string) => Promise<boolean>;
  cancelSetup: () => Promise<void>;
  unlockWithPassword: (password: string, remember: boolean) => Promise<boolean>;
  unlockWithRecovery: (
    recoveryKey: string,
    newPassword: string | null,
    remember: boolean,
  ) => Promise<boolean>;
  changePassword: (currentPassword: string, newPassword: string) => Promise<boolean>;
  lock: (forgetKeyring?: boolean) => Promise<void>;
  openModal: (mode: "none" | "setup" | "unlock" | "change-password") => void;
  applyExternalStatus: (state: VaultLockState, remember: boolean) => void;
  clearError: () => void;
}

function syncDockIndicator(lockState: VaultLockState) {
  useDockStore.setState({
    syncStatus: lockState === "Unlocked" ? "idle" : "locked",
  });
}

export const useVaultStore = create<VaultStoreState>((set, get) => ({
  status: {
    state: "Uninitialized",
    remember_in_keyring: false,
    idle_lock_minutes: 15,
    key_epoch: null,
    aead: null,
  },
  pendingSetup: null,
  strength: null,
  busy: false,
  error: null,
  modalMode: "none",

  async loadStatus() {
    try {
      const status = await activeIpc.vaultStatus();
      set({ status });
      syncDockIndicator(status.state);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async checkStrength(password) {
    try {
      const strength = await activeIpc.vaultCheckPasswordStrength(password);
      set({ strength });
    } catch {
      // ignore strength check error
    }
  },

  async beginSetup(password, aead, remember) {
    set({ busy: true, error: null });
    try {
      const pendingSetup = await activeIpc.vaultBeginSetup(password, aead, remember);
      set({ pendingSetup, busy: false });
      return true;
    } catch (err) {
      set({ busy: false, error: extractErrorMessage(err) });
      return false;
    }
  },

  async confirmSetup(suffixInput) {
    set({ busy: true, error: null });
    try {
      const status = await activeIpc.vaultConfirmSetup(suffixInput);
      set({ status, pendingSetup: null, busy: false, modalMode: "none", strength: null });
      syncDockIndicator(status.state);
      return true;
    } catch (err) {
      set({ busy: false, error: extractErrorMessage(err) });
      return false;
    }
  },

  async cancelSetup() {
    await activeIpc.vaultCancelSetup();
    set({ pendingSetup: null, error: null, modalMode: "none", strength: null });
  },

  async unlockWithPassword(password, remember) {
    set({ busy: true, error: null });
    try {
      const status = await activeIpc.vaultUnlockPassword(password, remember);
      set({ status, busy: false, modalMode: "none" });
      syncDockIndicator(status.state);
      return true;
    } catch (err) {
      set({ busy: false, error: extractErrorMessage(err) });
      return false;
    }
  },

  async unlockWithRecovery(recoveryKey, newPassword, remember) {
    set({ busy: true, error: null });
    try {
      const status = await activeIpc.vaultUnlockRecovery(
        recoveryKey,
        newPassword?.trim() ? newPassword : null,
        remember,
      );
      set({ status, busy: false, modalMode: "none" });
      syncDockIndicator(status.state);
      return true;
    } catch (err) {
      set({ busy: false, error: extractErrorMessage(err) });
      return false;
    }
  },

  async changePassword(currentPassword, newPassword) {
    set({ busy: true, error: null });
    try {
      await activeIpc.vaultChangePassword(currentPassword, newPassword);
      set({ busy: false, modalMode: "none", strength: null });
      return true;
    } catch (err) {
      set({ busy: false, error: extractErrorMessage(err) });
      return false;
    }
  },

  async lock(forgetKeyring = false) {
    const status = await activeIpc.vaultLock(forgetKeyring);
    set({ status });
    syncDockIndicator(status.state);
  },

  openModal(mode) {
    set({ modalMode: mode, error: null });
  },

  applyExternalStatus(lockState, remember) {
    set((s) => ({
      status: { ...s.status, state: lockState, remember_in_keyring: remember },
    }));
    syncDockIndicator(lockState);
    if (lockState !== get().status.state) {
      void get().loadStatus();
    }
  },

  clearError() {
    set({ error: null });
  },
}));

function extractErrorMessage(err: unknown): string {
  if (typeof err === "object" && err !== null && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}
