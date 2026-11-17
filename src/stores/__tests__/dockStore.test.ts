import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DockMode, DockPlan } from "../../bindings/commands";
import type { DockIpc } from "../../lib/ipc";
import { setDockIpcForTests, useDockStore } from "../dockStore";

function createMockIpc() {
  let seq = 0;
  const callLog: string[] = [];

  const makePlan = (target: DockMode, nextSeq: number): DockPlan => ({
    seq: nextSeq,
    target_mode: target,
    anchor: "TopRight",
    monitor_name: "Primary",
    scale_factor: 1,
    rect_phys:
      target === "Collapsed"
        ? { x: 1860, y: 400, width: 52, height: 160 }
        : { x: 1500, y: 400, width: 412, height: 652 },
    window_logical:
      target === "Collapsed" ? { width: 52, height: 160 } : { width: 412, height: 652 },
    visual_logical:
      target === "Collapsed" ? { width: 40, height: 148 } : { width: 400, height: 640 },
    shadow_pad: 6,
  });

  const plans = new Map<number, DockPlan>();

  const ipc: DockIpc = {
    dockGetState: vi.fn(async () => {
      seq += 1;
      const plan = makePlan("Collapsed", seq);
      plans.set(seq, plan);
      return {
        mode: "Collapsed" as const,
        pinned: false,
        anchor: "TopRight" as const,
        seq,
        vertical_ratio: 0.5,
        plan,
      };
    }),
    dockPlanTransition: vi.fn(async (target) => {
      seq += 1;
      callLog.push(`plan:${target}:${seq}`);
      const plan = makePlan(target, seq);
      plans.set(seq, plan);
      return plan;
    }),
    dockApply: vi.fn(async (planSeq, focusWindow) => {
      const plan = plans.get(planSeq)!;
      callLog.push(`apply:${plan.target_mode}:${planSeq}:focus=${focusWindow}`);
      return {
        mode: plan.target_mode,
        pinned: useDockStore.getState().pinned,
        anchor: plan.anchor,
        seq: planSeq,
        vertical_ratio: 0.5,
        plan,
      };
    }),
    dockSetPinned: vi.fn(async (pinned) => {
      callLog.push(`pin:${pinned}`);
      const snap = makePlan(
        useDockStore.getState().state === "Collapsing"
          ? "Expanded"
          : (useDockStore.getState().state as DockMode),
        seq,
      );
      return {
        mode: snap.target_mode,
        pinned,
        anchor: "TopRight" as const,
        seq,
        vertical_ratio: 0.5,
        plan: snap,
      };
    }),
    dockSetVerticalRatio: vi.fn(async () => makePlan("Collapsed", seq)),
    dockNudgeVerticalByScreenY: vi.fn(async () => makePlan("Collapsed", seq)),
    dockRedock: vi.fn(async () => makePlan("Collapsed", seq)),
    settingsGet: vi.fn(async () => ({
      monitor_policy: { kind: "Primary" as const },
      vertical_ratio: 0.5,
      hotkey: "Alt+Space",
      pinned_by_default: false,
      reduced_motion_override: null,
      autostart: false,
      google_client_id: "",
      google_client_secret: "",
      sync_retention_days: 30,
    })),
    settingsUpdate: vi.fn(async () => ({
      monitor_policy: { kind: "Primary" as const },
      vertical_ratio: 0.5,
      hotkey: "Alt+Space",
      pinned_by_default: false,
      reduced_motion_override: null,
      autostart: false,
      google_client_id: "",
      google_client_secret: "",
      sync_retention_days: 30,
    })),
    listMonitors: vi.fn(async () => []),
    rootsList: vi.fn(async () => []),
    rootsCountDocs: vi.fn(async () => 0),
    rootsCreateFolder: vi.fn(async (name) => ({
      id: 1,
      root_uuid: "r1",
      path: name,
      label: name,
      enabled: true,
      backup: true,
      exclude_globs: [],
      doc_count: 0,
    })),
    rootsRenameFolder: vi.fn(async (rootId, newName) => ({
      id: rootId,
      root_uuid: "r1",
      path: newName,
      label: newName,
      enabled: true,
      backup: true,
      exclude_globs: [],
      doc_count: 0,
    })),
    rootsAdd: vi.fn(async (path, label) => ({
      id: 1,
      root_uuid: "uuid",
      path,
      label: label ?? path,
      enabled: true,
      backup: false,
      exclude_globs: [],
      doc_count: 0,
    })),
    rootsRemove: vi.fn(async () => {}),
    rootsSetEnabled: vi.fn(async () => {}),
    rootsSetBackup: vi.fn(async () => {}),
    rootsRescan: vi.fn(async () => 0),
    searchDocs: vi.fn(async () => []),
    getSnippets: vi.fn(async () => []),
    getPreview: vi.fn(async (docId) => ({
      doc_id: docId,
      title: "Doc",
      file_name: "doc.md",
      rel_path: "doc.md",
      root_label: "Notes",
      tags: [],
      word_count: 1,
      mtime_ms: 0,
      html: "<p>Doc</p>",
      raw: "Doc",
    })),
    copyDocument: vi.fn(async () => {}),
    openInEditor: vi.fn(async () => {}),
    revealInFolder: vi.fn(async () => {}),
    openExternalUrl: vi.fn(async () => {}),
    previewDroppedFile: vi.fn(async (path) => ({
      file_name: path,
      path,
      parent_dir: "",
      title: path,
      tags: [],
      word_count: 0,
      html: "",
      raw: "",
    })),
    uploadMarkdownFiles: vi.fn(async () => ({
      target_folder_id: 1,
      target_folder_label: "General",
      uploaded_doc_ids: [],
      uploaded_count: 0,
      skipped_count: 0,
      rejected_dirs: 0,
      first_doc_id: null,
    })),
    moveDocumentToFolder: vi.fn(async (docId) => docId),
    deleteDocument: vi.fn(async () => {}),
    saveDocument: vi.fn(async (docId, content, mode) => ({
      doc_id: docId,
      title: "Doc",
      file_name: "doc.md",
      rel_path: "doc.md",
      root_label: "Notes",
      tags: [],
      word_count: content.split(/\s+/).filter(Boolean).length,
      mtime_ms: 0,
      html: mode === "Rendered" ? "<p>Doc</p>" : null,
      raw: content,
    })),
    vaultStatus: vi.fn(async () => ({
      state: "Uninitialized" as const,
      remember_in_keyring: false,
      idle_lock_minutes: 15,
      key_epoch: null,
      aead: null,
    })),
    vaultCheckPasswordStrength: vi.fn(async () => ({ score: 3, warning: null, suggestions: [] })),
    vaultBeginSetup: vi.fn(),
    vaultConfirmSetup: vi.fn(),
    vaultCancelSetup: vi.fn(),
    vaultUnlockPassword: vi.fn(),
    vaultUnlockRecovery: vi.fn(),
    vaultChangePassword: vi.fn(),
    vaultLock: vi.fn(),
    syncStatus: vi.fn(),
    driveConnect: vi.fn(),
    driveDisconnect: vi.fn(),
    driveCheckRemoteKeyfile: vi.fn(),
    driveImportRemoteKeyfile: vi.fn(),
    syncNow: vi.fn(),
    syncPull: vi.fn(),
    conflictsList: vi.fn(),
    conflictsResolve: vi.fn(),
    restoreList: vi.fn(),
    restoreRun: vi.fn(),
  };

  return { ipc, callLog };
}

describe("dockStore state machine", () => {
  beforeEach(() => {
    useDockStore.setState({
      state: "Collapsed",
      preCollapseMode: "Peek",
      anchor: "TopRight",
      pinned: false,
      seq: 0,
      lastTypingAt: 0,
      focusSearchSeq: 0,
      settingsOpen: false,
      visualLogical: { width: 40, height: 148 },
      shadowPad: 10,
      verticalRatio: 0.5,
      indexedCount: 0,
      syncStatus: "locked",
      droppedPaths: [],
      notice: null,
    });
  });

  it("resizes OS window before animating expand", async () => {
    const { ipc, callLog } = createMockIpc();
    setDockIpcForTests(ipc);

    await useDockStore.getState().expandTo("Expanded");

    expect(callLog).toEqual(["plan:Expanded:1", "apply:Expanded:1:focus=true"]);
    expect(useDockStore.getState().state).toBe("Expanded");
    expect(useDockStore.getState().visualLogical).toEqual({ width: 400, height: 640 });
  });

  it("animates collapse before shrinking OS window", async () => {
    const { ipc, callLog } = createMockIpc();
    setDockIpcForTests(ipc);

    await useDockStore.getState().expandTo("Expanded");
    callLog.length = 0;

    await useDockStore.getState().collapse();
    expect(useDockStore.getState().state).toBe("Collapsing");
    // OS window must NOT shrink yet while spring animation runs.
    expect(callLog).toEqual([]);

    await useDockStore.getState().finalizeCollapse();
    expect(useDockStore.getState().state).toBe("Collapsed");
    expect(callLog).toEqual(["plan:Collapsed:2", "apply:Collapsed:2:focus=false"]);
  });

  it("cancels OS shrink when collapse is interrupted", async () => {
    const { ipc, callLog } = createMockIpc();
    setDockIpcForTests(ipc);

    await useDockStore.getState().expandTo("Expanded");
    await useDockStore.getState().collapse();
    expect(useDockStore.getState().state).toBe("Collapsing");
    callLog.length = 0;

    await useDockStore.getState().expandTo("Expanded", { focusWindow: false });
    expect(useDockStore.getState().state).toBe("Expanded");

    // Stale finalizeCollapse call must be a no-op.
    await useDockStore.getState().finalizeCollapse();
    expect(useDockStore.getState().state).toBe("Expanded");
    expect(callLog.some((c) => c.startsWith("apply:Collapsed"))).toBe(false);
  });

  it("respects pin unless forced", async () => {
    const { ipc } = createMockIpc();
    setDockIpcForTests(ipc);

    await useDockStore.getState().expandTo("Expanded");
    await useDockStore.getState().setPinned(true);

    await useDockStore.getState().collapse();
    expect(useDockStore.getState().state).toBe("Expanded");

    await useDockStore.getState().collapse({ force: true });
    expect(useDockStore.getState().pinned).toBe(false);
    expect(useDockStore.getState().state).toBe("Collapsing");
  });
});
