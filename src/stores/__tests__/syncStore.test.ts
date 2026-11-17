import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DockIpc } from "../../lib/ipc";
import { useDockStore } from "../dockStore";
import { setSearchIpcForTests } from "../searchStore";
import { setSyncIpcForTests, useSyncStore } from "../syncStore";
import { useVaultStore } from "../vaultStore";

function createSyncMockIpc() {
  let connected = false;
  const ipc = {
    rootsList: vi.fn(async () => []),
    rootsCountDocs: vi.fn(async () => 2),
    searchDocs: vi.fn(async () => []),
    syncStatus: vi.fn(async () => ({
      connected,
      phase: "Idle" as const,
      queued: 0,
      synced: 2,
      conflicts: 0,
      last_ok_sec: 1234,
      last_error: null,
    })),
    driveConnect: vi.fn(async () => {
      connected = true;
    }),
    driveDisconnect: vi.fn(async () => {
      connected = false;
    }),
    driveCheckRemoteKeyfile: vi.fn(async () => true),
    driveImportRemoteKeyfile: vi.fn(async () => {}),
    syncNow: vi.fn(async () => ({
      uploaded: 2,
      skipped: 1,
      tombstoned: 0,
      conflicts_detected: 0,
      errors: 0,
    })),
    syncPull: vi.fn(async () => ({
      pulled: 1,
      conflict_copies: 0,
      skipped: 2,
    })),
    conflictsList: vi.fn(async () => []),
    conflictsResolve: vi.fn(async () => {}),
    restoreList: vi.fn(async () => [
      {
        file_id: "ab".repeat(32),
        root_uuid: "r1",
        root_label: "Notes",
        rel_path: "hello.md",
        size_bytes: 42,
        updated_at_sec: 1000,
        deleted: false,
        device_name: "Laptop",
        sha256_hex: "cd".repeat(32),
        blob_drive_id: "drv_1",
        versions: [
          { sha256_hex: "cd".repeat(32), blob_drive_id: "drv_1", at_sec: 1000 },
        ],
      },
    ]),
    restoreRun: vi.fn(async () => ({
      restored: 1,
      skipped_existing: 0,
      failed: 0,
    })),
  } as unknown as DockIpc;
  return ipc;
}

describe("syncStore", () => {
  beforeEach(() => {
    useVaultStore.setState({
      status: {
        state: "Unlocked",
        remember_in_keyring: false,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm",
      },
    });
    useDockStore.setState({ syncStatus: "idle" });
    useSyncStore.setState({
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
    });
  });

  it("connects, runs 1-way backup, pulls changes, and restores", async () => {
    const ipc = createSyncMockIpc();
    setSyncIpcForTests(ipc);
    setSearchIpcForTests(ipc);

    const ok = await useSyncStore.getState().connectDrive();
    expect(ok).toBe(true);
    expect(useSyncStore.getState().status.connected).toBe(true);

    await useSyncStore.getState().syncNow();
    expect(useSyncStore.getState().lastMessage).toContain("Backed up 2 file(s)");

    await useSyncStore.getState().pullChanges();
    expect(useSyncStore.getState().lastMessage).toContain("Pulled 1 file(s)");

    await useSyncStore.getState().loadRestoreCatalog();
    expect(useSyncStore.getState().restoreCatalog).toHaveLength(1);

    await useSyncStore.getState().runRestore(
      [
        {
          file_id: "ab".repeat(32),
          rel_path: "hello.md",
          blob_drive_id: "drv_1",
          sha256_hex: "cd".repeat(32),
        },
      ],
      "D:\\Restored",
      false,
    );
    expect(useSyncStore.getState().lastMessage).toContain("Restored 1 file(s)");
  });
});
