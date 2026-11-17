import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DockIpc } from "../../lib/ipc";
import { useDockStore } from "../dockStore";
import { setVaultIpcForTests, useVaultStore } from "../vaultStore";

function createVaultMockIpc() {
  let state: "Uninitialized" | "Locked" | "Unlocked" = "Uninitialized";
  const ipc = {
    vaultStatus: vi.fn(async () => ({
      state,
      remember_in_keyring: false,
      idle_lock_minutes: 15,
      key_epoch: state === "Unlocked" ? 1 : null,
      aead: state === "Unlocked" ? ("Aes256Gcm" as const) : null,
    })),
    vaultCheckPasswordStrength: vi.fn(async (pw: string) => ({
      score: pw.length >= 12 ? 4 : 1,
      warning: pw.length < 12 ? "Too short" : null,
      suggestions: [],
    })),
    vaultBeginSetup: vi.fn(async () => ({
      recovery_key: "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF-GGGG-HHHH-IIII-JJJJ-KKKK-LLLL-MMMM",
      recovery_suffix: "LLMMMM",
    })),
    vaultConfirmSetup: vi.fn(async (suffix: string) => {
      if (suffix !== "LLMMMM") {
        throw { kind: "Crypto", message: "Recovery Key verification characters did not match" };
      }
      state = "Unlocked";
      return {
        state,
        remember_in_keyring: false,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm" as const,
      };
    }),
    vaultCancelSetup: vi.fn(async () => {}),
    vaultUnlockPassword: vi.fn(async (pw: string, remember: boolean) => {
      if (pw !== "valid-passphrase-123") {
        throw { kind: "Crypto", message: "Authentication failed" };
      }
      state = "Unlocked";
      return {
        state,
        remember_in_keyring: remember,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm" as const,
      };
    }),
    vaultUnlockRecovery: vi.fn(),
    vaultChangePassword: vi.fn(async () => {}),
    vaultLock: vi.fn(async () => {
      state = "Locked";
      return {
        state,
        remember_in_keyring: false,
        idle_lock_minutes: 15,
        key_epoch: null,
        aead: null,
      };
    }),
  } as unknown as DockIpc;
  return ipc;
}

describe("vaultStore", () => {
  beforeEach(() => {
    useDockStore.setState({ syncStatus: "locked" });
    useVaultStore.setState({
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
    });
  });

  it("enforces mandatory recovery key verification before unlocking vault", async () => {
    setVaultIpcForTests(createVaultMockIpc());

    const begun = await useVaultStore
      .getState()
      .beginSetup("valid-passphrase-123", "Aes256Gcm", false);
    expect(begun).toBe(true);
    expect(useVaultStore.getState().pendingSetup?.recovery_suffix).toBe("LLMMMM");
    expect(useVaultStore.getState().status.state).toBe("Uninitialized");

    const badConfirm = await useVaultStore.getState().confirmSetup("WRONG1");
    expect(badConfirm).toBe(false);
    expect(useVaultStore.getState().error).toContain("did not match");

    const okConfirm = await useVaultStore.getState().confirmSetup("LLMMMM");
    expect(okConfirm).toBe(true);
    expect(useVaultStore.getState().status.state).toBe("Unlocked");
    expect(useDockStore.getState().syncStatus).toBe("idle");
  });

  it("locks and unlocks with password", async () => {
    setVaultIpcForTests(createVaultMockIpc());
    await useVaultStore.getState().lock(false);
    expect(useVaultStore.getState().status.state).toBe("Locked");
    expect(useDockStore.getState().syncStatus).toBe("locked");

    const bad = await useVaultStore.getState().unlockWithPassword("wrong", false);
    expect(bad).toBe(false);

    const good = await useVaultStore.getState().unlockWithPassword("valid-passphrase-123", true);
    expect(good).toBe(true);
    expect(useVaultStore.getState().status.state).toBe("Unlocked");
    expect(useDockStore.getState().syncStatus).toBe("idle");
  });
});
