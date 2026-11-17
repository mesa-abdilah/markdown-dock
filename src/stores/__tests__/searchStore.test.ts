import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchHit } from "../../bindings/commands";
import type { DockIpc } from "../../lib/ipc";
import { useDockStore } from "../dockStore";
import { setSearchIpcForTests, useSearchStore } from "../searchStore";

const SAMPLE_HITS: SearchHit[] = [
  {
    doc_id: 1,
    root_id: 1,
    root_label: "Notes",
    rel_path: "rust/tauri.md",
    file_name: "tauri.md",
    title: "Tauri Architecture",
    tags: ["rust", "tauri"],
    word_count: 240,
    mtime_ms: 1000,
    score: 1.2,
  },
  {
    doc_id: 2,
    root_id: 1,
    root_label: "Notes",
    rel_path: "crypto/vault.md",
    file_name: "vault.md",
    title: "Vault Spec",
    tags: ["crypto"],
    word_count: 410,
    mtime_ms: 900,
    score: 0.9,
  },
];

function createMockIpc(): DockIpc {
  return {
    dockGetState: vi.fn(async () => ({
      mode: "Collapsed" as const,
      pinned: false,
      anchor: "TopRight" as const,
      seq: 1,
      vertical_ratio: 0.5,
      plan: {
        seq: 1,
        target_mode: "Collapsed" as const,
        anchor: "TopRight" as const,
        monitor_name: "Primary",
        scale_factor: 1,
        rect_phys: { x: 0, y: 0, width: 56, height: 164 },
        window_logical: { width: 56, height: 164 },
        visual_logical: { width: 40, height: 148 },
        shadow_pad: 8,
      },
    })),
    dockPlanTransition: vi.fn(async (target) => ({
      seq: 2,
      target_mode: target,
      anchor: "TopRight" as const,
      monitor_name: "Primary",
      scale_factor: 1,
      rect_phys: { x: 0, y: 0, width: 776, height: 736 },
      window_logical: { width: 776, height: 736 },
      visual_logical: { width: 760, height: 720 },
      shadow_pad: 8,
    })),
    dockApply: vi.fn(async (seq) => ({
      mode: "Wide" as const,
      pinned: false,
      anchor: "TopRight" as const,
      seq,
      vertical_ratio: 0.5,
      plan: {
        seq,
        target_mode: "Wide" as const,
        anchor: "TopRight" as const,
        monitor_name: "Primary",
        scale_factor: 1,
        rect_phys: { x: 0, y: 0, width: 776, height: 736 },
        window_logical: { width: 776, height: 736 },
        visual_logical: { width: 760, height: 720 },
        shadow_pad: 8,
      },
    })),
    dockSetPinned: vi.fn(),
    dockSetVerticalRatio: vi.fn(),
    dockNudgeVerticalByScreenY: vi.fn(),
    dockRedock: vi.fn(),
    settingsGet: vi.fn(),
    settingsUpdate: vi.fn(),
    listMonitors: vi.fn(async () => []),
    rootsList: vi.fn(async () => [
      {
        id: 1,
        root_uuid: "u1",
        path: "D:\\Notes",
        label: "Notes",
        enabled: true,
        backup: false,
        exclude_globs: [],
        doc_count: 2,
      },
    ]),
    rootsCountDocs: vi.fn(async () => 2),
    rootsCreateFolder: vi.fn(async (name) => ({
      id: 2,
      root_uuid: "u2",
      path: `D:\\library\\${name}`,
      label: name,
      enabled: true,
      backup: true,
      exclude_globs: [],
      doc_count: 0,
    })),
    rootsRenameFolder: vi.fn(async (rootId, newName) => ({
      id: rootId,
      root_uuid: "u1",
      path: "D:\\Notes",
      label: newName,
      enabled: true,
      backup: true,
      exclude_globs: [],
      doc_count: 2,
    })),
    rootsAdd: vi.fn(),
    rootsRemove: vi.fn(),
    rootsSetEnabled: vi.fn(),
    rootsSetBackup: vi.fn(),
    rootsRescan: vi.fn(async () => 0),
    searchDocs: vi.fn(async (q) =>
      q ? SAMPLE_HITS.filter((h) => h.title.toLowerCase().includes(q.toLowerCase())) : SAMPLE_HITS,
    ),
    getSnippets: vi.fn(async (ids: number[]) =>
      ids.map((id) => ({ doc_id: id, html: "<mark>Tauri</mark> snippet" })),
    ),
    getPreview: vi.fn(async (docId, mode) => ({
      doc_id: docId,
      title: "Tauri Architecture",
      file_name: "tauri.md",
      rel_path: "rust/tauri.md",
      root_label: "Notes",
      tags: ["rust"],
      word_count: 240,
      mtime_ms: 1000,
      html: mode === "Rendered" ? "<h1>Tauri Architecture</h1>" : null,
      raw: "# Tauri Architecture",
    })),
    copyDocument: vi.fn(async () => {}),
    openInEditor: vi.fn(async () => {}),
    revealInFolder: vi.fn(async () => {}),
    openExternalUrl: vi.fn(async () => {}),
    previewDroppedFile: vi.fn(),
    uploadMarkdownFiles: vi.fn(async (_paths, folderId) => ({
      target_folder_id: folderId ?? 1,
      target_folder_label: "Notes",
      uploaded_doc_ids: [99],
      uploaded_count: 1,
      skipped_count: 0,
      rejected_dirs: 0,
      first_doc_id: 99,
    })),
    moveDocumentToFolder: vi.fn(async () => 101),
    deleteDocument: vi.fn(async () => {}),
    saveDocument: vi.fn(async (docId, content, mode) => ({
      doc_id: docId,
      title: "Tauri Architecture",
      file_name: "tauri.md",
      rel_path: "rust/tauri.md",
      root_label: "Notes",
      tags: ["rust"],
      word_count: content.split(/\s+/).filter(Boolean).length,
      mtime_ms: 1001,
      html: mode === "Rendered" ? `<h1>Edited</h1>` : null,
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
}

describe("searchStore", () => {
  beforeEach(() => {
    useDockStore.setState({ state: "Expanded", indexedCount: 0 });
    useSearchStore.setState({
      query: "",
      rootFilter: null,
      roots: [],
      hits: [],
      snippets: {},
      selectedIndex: 0,
      activeDocId: null,
      preview: null,
      droppedPreview: null,
      previewMode: "Raw",
      draftContent: "",
      undoStack: [],
      redoStack: [],
      dirty: false,
      saving: false,
      loadingSearch: false,
      loadingPreview: false,
      indexingProgress: null,
      copiedFeedback: null,
      collapsedFolderIds: {},
      error: null,
    });
  });

  it("loads roots and syncs dock indexedCount", async () => {
    setSearchIpcForTests(createMockIpc());
    await useSearchStore.getState().loadRoots();
    expect(useSearchStore.getState().roots).toHaveLength(1);
    expect(useDockStore.getState().indexedCount).toBe(2);
  });

  it("opens in Raw mode by default, supports undo/redo without auto-save, and saves explicitly", async () => {
    const ipc = createMockIpc();
    setSearchIpcForTests(ipc);

    await useSearchStore.getState().runSearch("tauri");
    expect(useSearchStore.getState().hits).toHaveLength(1);

    await useSearchStore.getState().fetchSnippets([1]);
    expect(useSearchStore.getState().snippets[1]).toContain("<mark>Tauri</mark>");

    await useSearchStore.getState().openPreview(1);
    expect(useSearchStore.getState().previewMode).toBe("Raw");
    expect(useSearchStore.getState().draftContent).toBe("# Tauri Architecture");

    useSearchStore.getState().setDraftContent("# Updated Content");
    expect(useSearchStore.getState().dirty).toBe(true);
    expect(useSearchStore.getState().undoStack).toEqual(["# Tauri Architecture"]);

    // Undo restores previous draft cleanly and clears dirty when back to saved raw
    useSearchStore.getState().undoDraft();
    expect(useSearchStore.getState().draftContent).toBe("# Tauri Architecture");
    expect(useSearchStore.getState().dirty).toBe(false);
    expect(useSearchStore.getState().redoStack).toEqual(["# Updated Content"]);

    // Redo re-applies the undone edit
    useSearchStore.getState().redoDraft();
    expect(useSearchStore.getState().draftContent).toBe("# Updated Content");
    expect(useSearchStore.getState().dirty).toBe(true);

    // Toggling preview mode does NOT auto-save unsaved edits
    await useSearchStore.getState().togglePreviewMode();
    expect(ipc.saveDocument).not.toHaveBeenCalled();
    expect(useSearchStore.getState().previewMode).toBe("Rendered");
    await useSearchStore.getState().togglePreviewMode();
    expect(useSearchStore.getState().previewMode).toBe("Raw");

    // Explicit save persists edits
    const saved = await useSearchStore.getState().saveCurrentDraft();
    expect(saved).toBe(true);
    expect(useSearchStore.getState().dirty).toBe(false);
    expect(ipc.saveDocument).toHaveBeenCalledWith(1, "# Updated Content", "Raw");
  });

  it("creates folders, uploads .md files, moves between folders, and deletes", async () => {
    const ipc = createMockIpc();
    setSearchIpcForTests(ipc);

    const folder = await useSearchStore.getState().createFolder("Kuliah");
    expect(folder?.label).toBe("Kuliah");
    expect(ipc.rootsCreateFolder).toHaveBeenCalledWith("Kuliah");

    const renamed = await useSearchStore.getState().renameFolder(1, "Kerja");
    expect(renamed).toBe(true);
    expect(ipc.rootsRenameFolder).toHaveBeenCalledWith(1, "Kerja");

    const report = await useSearchStore.getState().uploadFiles(["D:\\notes\\hello.md"], 2);
    expect(report?.uploaded_count).toBe(1);
    expect(ipc.uploadMarkdownFiles).toHaveBeenCalledWith(["D:\\notes\\hello.md"], 2);

    await useSearchStore.getState().moveDocToFolder(99, 1);
    expect(ipc.moveDocumentToFolder).toHaveBeenCalledWith(99, 1);

    useSearchStore.getState().toggleFolderCollapsed(1);
    expect(useSearchStore.getState().collapsedFolderIds[1]).toBe(true);

    await useSearchStore.getState().deleteDoc(101);
    expect(ipc.deleteDocument).toHaveBeenCalledWith(101);
  });
});
