import { create } from "zustand";
import type {
  CopyFormat,
  DroppedPreview,
  PreviewMode,
  RootInfo,
  SearchHit,
} from "../bindings/commands";
import { defaultDockIpc, type DocPreview, type DockIpc, type UploadReport } from "../lib/ipc";
import { useDockStore } from "./dockStore";

let activeIpc: DockIpc = defaultDockIpc;

export function setSearchIpcForTests(ipc: DockIpc) {
  activeIpc = ipc;
}

function extractSearchError(err: unknown): string {
  if (typeof err === "object" && err !== null && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}

let searchSeq = 0;
let copiedTimer: number | null = null;
let lastEditAt = 0;
const MAX_HISTORY_ENTRIES = 200;
const COALESCE_WINDOW_MS = 450;

export interface SearchStoreState {
  query: string;
  rootFilter: number | null;
  roots: RootInfo[];
  hits: SearchHit[];
  snippets: Record<number, string>;
  selectedIndex: number;
  activeDocId: number | null;
  preview: DocPreview | null;
  droppedPreview: DroppedPreview | null;
  previewMode: PreviewMode;
  draftContent: string;
  undoStack: string[];
  redoStack: string[];
  dirty: boolean;
  saving: boolean;
  loadingSearch: boolean;
  loadingPreview: boolean;
  indexingProgress: { rootId: number; done: number; total: number } | null;
  copiedFeedback: CopyFormat | null;
  collapsedFolderIds: Record<number, boolean>;
  error: string | null;

  loadRoots: () => Promise<void>;
  createFolder: (name: string) => Promise<RootInfo | null>;
  renameFolder: (rootId: number, newName: string) => Promise<boolean>;
  addRoot: (path: string, label?: string) => Promise<void>;
  removeRoot: (rootId: number) => Promise<void>;
  setRootEnabled: (rootId: number, enabled: boolean) => Promise<void>;
  rescan: (rootId?: number | null) => Promise<void>;
  toggleFolderCollapsed: (rootId: number) => void;
  uploadFiles: (paths: string[], targetFolderId?: number | null) => Promise<UploadReport | null>;
  moveDocToFolder: (docId: number, targetRootId: number) => Promise<void>;
  deleteDoc: (docId: number) => Promise<void>;
  setQuery: (query: string) => void;
  setRootFilter: (rootId: number | null) => void;
  setSelectedIndex: (index: number) => void;
  moveSelection: (delta: number) => void;
  runSearch: (overrideQuery?: string, overrideRoot?: number | null) => Promise<void>;
  fetchSnippets: (docIds: number[]) => Promise<void>;
  openPreview: (docId: number) => Promise<void>;
  setDraftContent: (content: string) => void;
  undoDraft: () => void;
  redoDraft: () => void;
  saveCurrentDraft: () => Promise<boolean>;
  openDroppedFile: (path: string) => Promise<void>;
  closePreview: () => Promise<void>;
  togglePreviewMode: () => Promise<void>;
  copyDoc: (docId: number, format?: CopyFormat) => Promise<void>;
  openInEditor: (docId: number) => Promise<void>;
  revealInFolder: (docId: number) => Promise<void>;
  setIndexingProgress: (p: { rootId: number; done: number; total: number } | null) => void;
  clearError: () => void;
}

export const useSearchStore = create<SearchStoreState>((set, get) => ({
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

  async loadRoots() {
    try {
      const [roots, totalDocs] = await Promise.all([
        activeIpc.rootsList(),
        activeIpc.rootsCountDocs(),
      ]);
      set({ roots });
      useDockStore.setState({ indexedCount: totalDocs });
    } catch (err) {
      set({ error: extractSearchError(err) });
    }
  },

  async createFolder(name) {
    set({ error: null });
    try {
      const created = await activeIpc.rootsCreateFolder(name);
      await get().loadRoots();
      await get().runSearch();
      return created;
    } catch (err) {
      set({ error: extractSearchError(err) });
      return null;
    }
  },

  async renameFolder(rootId, newName) {
    set({ error: null });
    try {
      await activeIpc.rootsRenameFolder(rootId, newName);
      await get().loadRoots();
      await get().runSearch();
      if (get().activeDocId !== null) {
        const current = get().activeDocId;
        if (current !== null) {
          const preview = await activeIpc.getPreview(current, get().previewMode);
          set({ preview });
        }
      }
      return true;
    } catch (err) {
      set({ error: extractSearchError(err) });
      return false;
    }
  },

  toggleFolderCollapsed(rootId) {
    set((s) => ({
      collapsedFolderIds: {
        ...s.collapsedFolderIds,
        [rootId]: !s.collapsedFolderIds[rootId],
      },
    }));
  },

  async uploadFiles(paths, targetFolderId = null) {
    set({ error: null });
    try {
      const chosenFolder = targetFolderId ?? get().rootFilter;
      const report = await activeIpc.uploadMarkdownFiles(paths, chosenFolder);
      await get().loadRoots();
      await get().runSearch();

      if (report.uploaded_count > 0) {
        const skippedNote =
          report.rejected_dirs > 0 || report.skipped_count > 0
            ? ` (skipped ${report.rejected_dirs + report.skipped_count} non-.md / folder item(s))`
            : "";
        useDockStore.setState({
          notice: {
            id: Date.now(),
            text: `Uploaded ${report.uploaded_count} .md file(s) to "${report.target_folder_label}"${skippedNote}.`,
          },
        });
        if (report.first_doc_id !== null) {
          await get().openPreview(report.first_doc_id);
        } else {
          await useDockStore.getState().expandTo("Expanded", { focusWindow: true });
        }
      } else {
        const msg =
          report.rejected_dirs > 0
            ? "Drag & drop only uploads .md files. Create folders inside the app to group files."
            : "No Markdown (.md / .markdown) files found in selection.";
        useDockStore.setState({ notice: { id: Date.now(), text: msg } });
        await useDockStore.getState().expandTo("Expanded", { focusWindow: true });
      }
      return report;
    } catch (err) {
      set({ error: extractSearchError(err) });
      await useDockStore.getState().expandTo("Expanded", { focusWindow: true });
      return null;
    }
  },

  async moveDocToFolder(docId, targetRootId) {
    set({ error: null });
    try {
      const newDocId = await activeIpc.moveDocumentToFolder(docId, targetRootId);
      const wasActive = get().activeDocId === docId;
      await get().loadRoots();
      await get().runSearch();
      if (wasActive) {
        await get().openPreview(newDocId);
      }
    } catch (err) {
      set({ error: extractSearchError(err) });
    }
  },

  async deleteDoc(docId) {
    set({ error: null });
    try {
      await activeIpc.deleteDocument(docId);
      if (get().activeDocId === docId) {
        await get().closePreview();
      }
      await get().loadRoots();
      await get().runSearch();
    } catch (err) {
      set({ error: extractSearchError(err) });
    }
  },

  async addRoot(path, label) {
    set({ error: null });
    try {
      await activeIpc.rootsAdd(path, label ?? null);
      await get().loadRoots();
      await get().runSearch();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async removeRoot(rootId) {
    set({ error: null });
    try {
      await activeIpc.rootsRemove(rootId);
      if (get().rootFilter === rootId) {
        set({ rootFilter: null });
      }
      await get().loadRoots();
      await get().runSearch();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async setRootEnabled(rootId, enabled) {
    set({ error: null });
    try {
      await activeIpc.rootsSetEnabled(rootId, enabled);
      await get().loadRoots();
      await get().runSearch();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async rescan(rootId = null) {
    set({ error: null });
    try {
      await activeIpc.rootsRescan(rootId);
      await get().loadRoots();
      await get().runSearch();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setQuery(query) {
    set({ query });
    void get().runSearch(query, get().rootFilter);
  },

  setRootFilter(rootId) {
    set({ rootFilter: rootId });
    void get().runSearch(get().query, rootId);
  },

  setSelectedIndex(index) {
    const len = get().hits.length;
    if (len === 0) {
      set({ selectedIndex: 0 });
      return;
    }
    set({ selectedIndex: Math.max(0, Math.min(index, len - 1)) });
  },

  moveSelection(delta) {
    const { hits, selectedIndex } = get();
    if (hits.length === 0) return;
    const next = Math.max(0, Math.min(selectedIndex + delta, hits.length - 1));
    set({ selectedIndex: next });
  },

  async runSearch(overrideQuery, overrideRoot) {
    const query = overrideQuery ?? get().query;
    const rootFilter = overrideRoot !== undefined ? overrideRoot : get().rootFilter;
    const reqId = ++searchSeq;
    set({ loadingSearch: true });
    try {
      const hits = await activeIpc.searchDocs(query, rootFilter, 150);
      if (reqId !== searchSeq) return;
      set({
        hits,
        snippets: {},
        selectedIndex: 0,
        loadingSearch: false,
      });
    } catch (err) {
      if (reqId !== searchSeq) return;
      set({ loadingSearch: false, error: String(err) });
    }
  },

  async fetchSnippets(docIds) {
    const existing = get().snippets;
    const missing = docIds.filter((id) => existing[id] === undefined);
    if (missing.length === 0) return;

    const currentQuery = get().query;
    try {
      const items = await activeIpc.getSnippets(missing, currentQuery);
      if (get().query !== currentQuery) return;
      set((s) => {
        const next = { ...s.snippets };
        for (const item of items) {
          next[item.doc_id] = item.html;
        }
        return { snippets: next };
      });
    } catch {
      // Non-fatal snippet failure
    }
  },

  async openPreview(docId) {
    lastEditAt = 0;
    set({
      activeDocId: docId,
      droppedPreview: null,
      previewMode: "Raw",
      loadingPreview: true,
      undoStack: [],
      redoStack: [],
      dirty: false,
      error: null,
    });
    try {
      if (useDockStore.getState().state !== "Fullscreen") {
        await useDockStore.getState().expandTo("Wide", { focusWindow: true });
      }
      const preview = await activeIpc.getPreview(docId, "Raw");
      if (get().activeDocId === docId) {
        set({
          preview,
          draftContent: preview.raw,
          undoStack: [],
          redoStack: [],
          dirty: false,
          loadingPreview: false,
        });
      }
    } catch (err) {
      set({ loadingPreview: false, error: extractSearchError(err) });
    }
  },

  setDraftContent(content) {
    const prev = get().draftContent;
    if (content === prev) return;
    const savedRaw = get().preview?.raw ?? "";
    const now = Date.now();
    const lenDelta = Math.abs(content.length - prev.length);
    const tailChar = content.length > prev.length ? content.slice(-1) : "";
    const isBoundary =
      lenDelta !== 1 ||
      tailChar === " " ||
      tailChar === "\n" ||
      tailChar === "\t" ||
      now - lastEditAt > COALESCE_WINDOW_MS;

    lastEditAt = now;
    set((s) => {
      let nextUndo = s.undoStack;
      if (isBoundary || s.undoStack.length === 0) {
        nextUndo = [...s.undoStack, prev];
        if (nextUndo.length > MAX_HISTORY_ENTRIES) {
          nextUndo = nextUndo.slice(nextUndo.length - MAX_HISTORY_ENTRIES);
        }
      }
      return {
        draftContent: content,
        undoStack: nextUndo,
        redoStack: [],
        dirty: content !== savedRaw,
      };
    });
  },

  undoDraft() {
    const { undoStack, draftContent, preview } = get();
    if (undoStack.length === 0) return;
    lastEditAt = 0;
    const previous = undoStack[undoStack.length - 1];
    const savedRaw = preview?.raw ?? "";
    set((s) => ({
      draftContent: previous,
      undoStack: s.undoStack.slice(0, -1),
      redoStack: [...s.redoStack, draftContent],
      dirty: previous !== savedRaw,
    }));
  },

  redoDraft() {
    const { redoStack, draftContent, preview } = get();
    if (redoStack.length === 0) return;
    lastEditAt = 0;
    const next = redoStack[redoStack.length - 1];
    const savedRaw = preview?.raw ?? "";
    set((s) => ({
      draftContent: next,
      undoStack: [...s.undoStack, draftContent],
      redoStack: s.redoStack.slice(0, -1),
      dirty: next !== savedRaw,
    }));
  },

  async saveCurrentDraft() {
    const docId = get().activeDocId;
    if (docId === null) return false;
    const content = get().draftContent;
    const mode = get().previewMode;
    set({ saving: true, error: null });
    try {
      const updated = await activeIpc.saveDocument(docId, content, mode);
      if (get().activeDocId === docId) {
        const currentDraft = get().draftContent;
        set({
          preview: updated,
          dirty: currentDraft !== updated.raw,
          saving: false,
        });
      } else {
        set({ saving: false });
      }
      await get().loadRoots();
      await get().runSearch();
      return true;
    } catch (err) {
      set({ saving: false, error: extractSearchError(err) });
      return false;
    }
  },

  async openDroppedFile(path) {
    lastEditAt = 0;
    set({
      activeDocId: null,
      preview: null,
      previewMode: "Raw",
      draftContent: "",
      undoStack: [],
      redoStack: [],
      dirty: false,
      loadingPreview: true,
      error: null,
    });
    try {
      const dropped = await activeIpc.previewDroppedFile(path);
      set({ droppedPreview: dropped, draftContent: dropped.raw, loadingPreview: false });
      await useDockStore.getState().expandTo("Wide", { focusWindow: true });
    } catch (err) {
      set({ loadingPreview: false, error: extractSearchError(err) });
      await useDockStore.getState().expandTo("Expanded", { focusWindow: true });
    }
  },

  async closePreview() {
    lastEditAt = 0;
    set({
      activeDocId: null,
      preview: null,
      droppedPreview: null,
      draftContent: "",
      undoStack: [],
      redoStack: [],
      dirty: false,
    });
    const st = useDockStore.getState().state;
    if (st === "Wide" || st === "Fullscreen") {
      await useDockStore.getState().expandTo("Expanded", { focusWindow: true });
    }
  },

  async togglePreviewMode() {
    const nextMode: PreviewMode = get().previewMode === "Rendered" ? "Raw" : "Rendered";
    set({ previewMode: nextMode });
    const docId = get().activeDocId;
    if (docId !== null && nextMode === "Rendered" && !get().preview?.html && !get().dirty) {
      set({ loadingPreview: true });
      try {
        const preview = await activeIpc.getPreview(docId, nextMode);
        if (get().activeDocId === docId) {
          set({ preview, loadingPreview: false });
        }
      } catch (err) {
        set({ loadingPreview: false, error: extractSearchError(err) });
      }
    }
  },

  async copyDoc(docId, format = "Raw") {
    try {
      await activeIpc.copyDocument(docId, format);
      set({ copiedFeedback: format });
      if (copiedTimer !== null) window.clearTimeout(copiedTimer);
      copiedTimer = window.setTimeout(() => {
        set({ copiedFeedback: null });
      }, 1600);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async openInEditor(docId) {
    try {
      await activeIpc.openInEditor(docId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async revealInFolder(docId) {
    try {
      await activeIpc.revealInFolder(docId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setIndexingProgress(p) {
    set({ indexingProgress: p });
  },

  clearError() {
    set({ error: null });
  },
}));
