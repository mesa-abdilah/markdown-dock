import {
  commands,
  type AeadAlg,
  type AppSettings_Serialize,
  type ConflictInfo_Serialize,
  type CopyFormat,
  type DocPreview_Serialize,
  type DocSnippet,
  type DockMode,
  type DockPlan,
  type DockStateSnapshot,
  type DroppedPreview,
  type MonitorInfo,
  type PasswordStrength_Serialize,
  type PreviewMode,
  type PullReport,
  type RestoreCatalogItem,
  type RestoreReport,
  type RestoreSelection,
  type RootInfo,
  type SearchHit,
  type SettingsPatch_Deserialize,
  type SyncRunReport,
  type SyncStatusEvent_Serialize,
  type SyncStatusSnapshot_Serialize,
  type UploadReport_Serialize,
  type VaultSetupPending,
  type VaultStatus_Serialize,
} from "../bindings/commands";

export type AppSettings = AppSettings_Serialize;
export type SettingsPatch = SettingsPatch_Deserialize;
export type DocPreview = DocPreview_Serialize;
export type VaultStatus = VaultStatus_Serialize;
export type PasswordStrength = PasswordStrength_Serialize;
export type ConflictInfo = ConflictInfo_Serialize;
export type SyncStatusSnapshot = SyncStatusSnapshot_Serialize;
export type SyncStatusEvent = SyncStatusEvent_Serialize;
export type UploadReport = UploadReport_Serialize;

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const DEFAULT_FALLBACK_PLAN = (target: DockMode, seq: number): DockPlan => {
  const sizes: Record<DockMode, { width: number; height: number }> = {
    Collapsed: { width: 58, height: 98 },
    Peek: { width: 308, height: 184 },
    Expanded: { width: 460, height: 680 },
    Wide: { width: 1060, height: 760 },
    Fullscreen: { width: 1280, height: 800 },
    DropTarget: { width: 240, height: 320 },
  };
  const vis = sizes[target];
  return {
    seq,
    target_mode: target,
    anchor: "TopRight",
    monitor_name: "Browser-Preview",
    scale_factor: 1,
    rect_phys: { x: 0, y: 0, width: vis.width + 16, height: vis.height + 16 },
    window_logical: { width: vis.width + 16, height: vis.height + 16 },
    visual_logical: vis,
    shadow_pad: 8,
  };
};

let browserSeq = 0;

export interface DockIpc {
  dockGetState: () => Promise<DockStateSnapshot>;
  dockPlanTransition: (target: DockMode) => Promise<DockPlan>;
  dockApply: (planSeq: number, focusWindow: boolean) => Promise<DockStateSnapshot>;
  dockSetPinned: (pinned: boolean) => Promise<DockStateSnapshot>;
  dockSetVerticalRatio: (ratio: number, persist: boolean) => Promise<DockPlan>;
  dockNudgeVerticalByScreenY: (centerPhysY: number, persist: boolean) => Promise<DockPlan>;
  dockRedock: () => Promise<DockPlan>;
  settingsGet: () => Promise<AppSettings>;
  settingsUpdate: (patch: SettingsPatch) => Promise<AppSettings>;
  listMonitors: () => Promise<MonitorInfo[]>;
  rootsList: () => Promise<RootInfo[]>;
  rootsCountDocs: () => Promise<number>;
  rootsCreateFolder: (name: string) => Promise<RootInfo>;
  rootsRenameFolder: (rootId: number, newName: string) => Promise<RootInfo>;
  rootsAdd: (path: string, label?: string | null) => Promise<RootInfo>;
  rootsRemove: (rootId: number) => Promise<void>;
  rootsSetEnabled: (rootId: number, enabled: boolean) => Promise<void>;
  rootsSetBackup: (rootId: number, backup: boolean) => Promise<void>;
  rootsRescan: (rootId?: number | null) => Promise<number>;
  searchDocs: (query: string, rootId?: number | null, limit?: number | null) => Promise<SearchHit[]>;
  getSnippets: (docIds: number[], query: string) => Promise<DocSnippet[]>;
  getPreview: (docId: number, mode: PreviewMode) => Promise<DocPreview>;
  copyDocument: (docId: number, format: CopyFormat) => Promise<void>;
  openInEditor: (docId: number) => Promise<void>;
  revealInFolder: (docId: number) => Promise<void>;
  openExternalUrl: (url: string) => Promise<void>;
  previewDroppedFile: (path: string) => Promise<DroppedPreview>;
  uploadMarkdownFiles: (paths: string[], folderId?: number | null) => Promise<UploadReport>;
  moveDocumentToFolder: (docId: number, targetRootId: number) => Promise<number>;
  deleteDocument: (docId: number) => Promise<void>;
  saveDocument: (docId: number, content: string, mode: PreviewMode) => Promise<DocPreview>;
  vaultStatus: () => Promise<VaultStatus>;
  vaultCheckPasswordStrength: (password: string) => Promise<PasswordStrength>;
  vaultBeginSetup: (
    password: string,
    aead: AeadAlg,
    rememberInKeyring: boolean,
  ) => Promise<VaultSetupPending>;
  vaultConfirmSetup: (suffixInput: string) => Promise<VaultStatus>;
  vaultCancelSetup: () => Promise<void>;
  vaultUnlockPassword: (password: string, rememberInKeyring: boolean) => Promise<VaultStatus>;
  vaultUnlockRecovery: (
    recoveryKey: string,
    newPassword: string | null,
    rememberInKeyring: boolean,
  ) => Promise<VaultStatus>;
  vaultChangePassword: (currentPassword: string, newPassword: string) => Promise<void>;
  vaultLock: (forgetKeyring: boolean) => Promise<VaultStatus>;
  syncStatus: () => Promise<SyncStatusSnapshot>;
  driveConnect: () => Promise<void>;
  driveDisconnect: () => Promise<void>;
  driveCheckRemoteKeyfile: () => Promise<boolean>;
  driveImportRemoteKeyfile: () => Promise<void>;
  syncNow: () => Promise<SyncRunReport>;
  syncPull: () => Promise<PullReport>;
  conflictsList: (unresolvedOnly: boolean) => Promise<ConflictInfo[]>;
  conflictsResolve: (conflictId: number, resolution: string) => Promise<void>;
  restoreList: () => Promise<RestoreCatalogItem[]>;
  restoreRun: (
    selections: RestoreSelection[],
    targetDir: string,
    overwrite: boolean,
  ) => Promise<RestoreReport>;
}

export const defaultDockIpc: DockIpc = {
  async dockGetState() {
    if (!isTauriRuntime()) {
      const plan = DEFAULT_FALLBACK_PLAN("Collapsed", ++browserSeq);
      return {
        mode: "Collapsed",
        pinned: false,
        anchor: "TopRight",
        seq: plan.seq,
        vertical_ratio: 0.5,
        plan,
      };
    }
    return commands.dockGetState();
  },
  async dockPlanTransition(target) {
    if (!isTauriRuntime()) {
      return DEFAULT_FALLBACK_PLAN(target, ++browserSeq);
    }
    return commands.dockPlanTransition(target);
  },
  async dockApply(planSeq, focusWindow) {
    if (!isTauriRuntime()) {
      const plan = DEFAULT_FALLBACK_PLAN("Expanded", planSeq);
      return {
        mode: plan.target_mode,
        pinned: false,
        anchor: plan.anchor,
        seq: planSeq,
        vertical_ratio: 0.5,
        plan,
      };
    }
    return commands.dockApply(planSeq, focusWindow);
  },
  async dockSetPinned(pinned) {
    if (!isTauriRuntime()) {
      const plan = DEFAULT_FALLBACK_PLAN("Expanded", browserSeq);
      return {
        mode: "Expanded",
        pinned,
        anchor: "TopRight",
        seq: browserSeq,
        vertical_ratio: 0.5,
        plan,
      };
    }
    return commands.dockSetPinned(pinned);
  },
  async dockSetVerticalRatio(ratio, persist) {
    if (!isTauriRuntime()) {
      void ratio;
      void persist;
      return DEFAULT_FALLBACK_PLAN("Collapsed", ++browserSeq);
    }
    return commands.dockSetVerticalRatio(ratio, persist);
  },
  async dockNudgeVerticalByScreenY(centerPhysY, persist) {
    if (!isTauriRuntime()) {
      void centerPhysY;
      void persist;
      return DEFAULT_FALLBACK_PLAN("Collapsed", ++browserSeq);
    }
    return commands.dockNudgeVerticalByScreenY(centerPhysY, persist);
  },
  async dockRedock() {
    if (!isTauriRuntime()) {
      return DEFAULT_FALLBACK_PLAN("Collapsed", ++browserSeq);
    }
    return commands.dockRedock();
  },
  async settingsGet() {
    if (!isTauriRuntime()) {
      return {
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
    }
    return commands.settingsGet();
  },
  async settingsUpdate(patch) {
    if (!isTauriRuntime()) {
      return {
        monitor_policy: patch.monitor_policy ?? { kind: "Primary" },
        vertical_ratio: patch.vertical_ratio ?? 0.5,
        hotkey: patch.hotkey ?? "Alt+Space",
        pinned_by_default: patch.pinned_by_default ?? false,
        reduced_motion_override: patch.reduced_motion_override ?? null,
        autostart: patch.autostart ?? false,
        google_client_id: patch.google_client_id ?? "",
        google_client_secret: patch.google_client_secret ?? "",
        sync_retention_days: patch.sync_retention_days ?? 30,
      };
    }
    return commands.settingsUpdate(patch);
  },
  async listMonitors() {
    if (!isTauriRuntime()) return [];
    return commands.listMonitors();
  },
  async rootsList() {
    if (!isTauriRuntime()) return [];
    return commands.rootsList();
  },
  async rootsCountDocs() {
    if (!isTauriRuntime()) return 0;
    return commands.rootsCountDocs();
  },
  async rootsCreateFolder(name) {
    if (!isTauriRuntime()) {
      return {
        id: Date.now(),
        root_uuid: "preview",
        path: name,
        label: name,
        enabled: true,
        backup: true,
        exclude_globs: [],
        doc_count: 0,
      };
    }
    return commands.rootsCreateFolder(name);
  },
  async rootsRenameFolder(rootId, newName) {
    if (!isTauriRuntime()) {
      return {
        id: rootId,
        root_uuid: "preview",
        path: newName,
        label: newName,
        enabled: true,
        backup: true,
        exclude_globs: [],
        doc_count: 0,
      };
    }
    return commands.rootsRenameFolder(rootId, newName);
  },
  async rootsAdd(path, label = null) {
    if (!isTauriRuntime()) {
      return {
        id: 1,
        root_uuid: "preview",
        path,
        label: label ?? path,
        enabled: true,
        backup: false,
        exclude_globs: [],
        doc_count: 0,
      };
    }
    return commands.rootsAdd(path, label);
  },
  async rootsRemove(rootId) {
    if (!isTauriRuntime()) return;
    await commands.rootsRemove(rootId);
  },
  async rootsSetEnabled(rootId, enabled) {
    if (!isTauriRuntime()) return;
    await commands.rootsSetEnabled(rootId, enabled);
  },
  async rootsSetBackup(rootId, backup) {
    if (!isTauriRuntime()) return;
    await commands.rootsSetBackup(rootId, backup);
  },
  async rootsRescan(rootId = null) {
    if (!isTauriRuntime()) return 0;
    return commands.rootsRescan(rootId);
  },
  async searchDocs(query, rootId = null, limit = 100) {
    if (!isTauriRuntime()) {
      void query;
      void rootId;
      void limit;
      return [];
    }
    return commands.searchDocs(query, rootId, limit);
  },
  async getSnippets(docIds, query) {
    if (!isTauriRuntime()) {
      void docIds;
      void query;
      return [];
    }
    return commands.getSnippets(docIds, query);
  },
  async getPreview(docId, mode) {
    if (!isTauriRuntime()) {
      return {
        doc_id: docId,
        title: "Preview",
        file_name: "preview.md",
        rel_path: "preview.md",
        root_label: "Notes",
        tags: [],
        word_count: 0,
        mtime_ms: Date.now(),
        html: mode === "Rendered" ? "<p>Preview unavailable outside Tauri runtime.</p>" : null,
        raw: "",
      };
    }
    return commands.getPreview(docId, mode);
  },
  async copyDocument(docId, format) {
    if (!isTauriRuntime()) {
      void docId;
      void format;
      return;
    }
    await commands.copyDocument(docId, format);
  },
  async openInEditor(docId) {
    if (!isTauriRuntime()) {
      void docId;
      return;
    }
    await commands.openInEditor(docId);
  },
  async revealInFolder(docId) {
    if (!isTauriRuntime()) {
      void docId;
      return;
    }
    await commands.revealInFolder(docId);
  },
  async openExternalUrl(url) {
    if (!isTauriRuntime()) {
      window.open(url, "_blank", "noopener,noreferrer");
      return;
    }
    await commands.openExternalUrl(url);
  },
  async previewDroppedFile(path) {
    if (!isTauriRuntime()) {
      return {
        file_name: path,
        path,
        parent_dir: "",
        title: path,
        tags: [],
        word_count: 0,
        html: "<p>Dropped file preview.</p>",
        raw: "",
      };
    }
    return commands.previewDroppedFile(path);
  },
  async uploadMarkdownFiles(paths, folderId = null) {
    if (!isTauriRuntime()) {
      void paths;
      return {
        target_folder_id: folderId ?? 1,
        target_folder_label: "General",
        uploaded_doc_ids: [],
        uploaded_count: 0,
        skipped_count: 0,
        rejected_dirs: 0,
        first_doc_id: null,
      };
    }
    return commands.uploadMarkdownFiles(paths, folderId);
  },
  async moveDocumentToFolder(docId, targetRootId) {
    if (!isTauriRuntime()) {
      void targetRootId;
      return docId;
    }
    return commands.moveDocumentToFolder(docId, targetRootId);
  },
  async deleteDocument(docId) {
    if (!isTauriRuntime()) {
      void docId;
      return;
    }
    await commands.deleteDocument(docId);
  },
  async saveDocument(docId, content, mode) {
    if (!isTauriRuntime()) {
      return {
        doc_id: docId,
        title: "Edited Note",
        file_name: "note.md",
        rel_path: "note.md",
        root_label: "General",
        tags: [],
        word_count: content.trim() ? content.trim().split(/\s+/).length : 0,
        mtime_ms: Date.now(),
        html: mode === "Rendered" ? `<pre>${content}</pre>` : null,
        raw: content,
      };
    }
    return commands.saveDocument(docId, content, mode);
  },
  async vaultStatus() {
    if (!isTauriRuntime()) {
      return {
        state: "Uninitialized",
        remember_in_keyring: false,
        idle_lock_minutes: 15,
        key_epoch: null,
        aead: null,
      };
    }
    return commands.vaultStatus();
  },
  async vaultCheckPasswordStrength(password) {
    if (!isTauriRuntime()) {
      return {
        score: password.length >= 12 ? 3 : 1,
        warning: password.length < 12 ? "Too short" : null,
        suggestions: [],
      };
    }
    return commands.vaultCheckPasswordStrength(password);
  },
  async vaultBeginSetup(password, aead, rememberInKeyring) {
    if (!isTauriRuntime()) {
      void password;
      void aead;
      void rememberInKeyring;
      return {
        recovery_key: "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF-GGGG-HHHH-IIII-JJJJ-KKKK-LLLL-MMMM",
        recovery_suffix: "LLMMMM",
      };
    }
    return commands.vaultBeginSetup(password, aead, rememberInKeyring);
  },
  async vaultConfirmSetup(suffixInput) {
    if (!isTauriRuntime()) {
      void suffixInput;
      return {
        state: "Unlocked",
        remember_in_keyring: false,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm",
      };
    }
    return commands.vaultConfirmSetup(suffixInput);
  },
  async vaultCancelSetup() {
    if (!isTauriRuntime()) return;
    await commands.vaultCancelSetup();
  },
  async vaultUnlockPassword(password, rememberInKeyring) {
    if (!isTauriRuntime()) {
      void password;
      return {
        state: "Unlocked",
        remember_in_keyring: rememberInKeyring,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm",
      };
    }
    return commands.vaultUnlockPassword(password, rememberInKeyring);
  },
  async vaultUnlockRecovery(recoveryKey, newPassword, rememberInKeyring) {
    if (!isTauriRuntime()) {
      void recoveryKey;
      void newPassword;
      return {
        state: "Unlocked",
        remember_in_keyring: rememberInKeyring,
        idle_lock_minutes: 15,
        key_epoch: 1,
        aead: "Aes256Gcm",
      };
    }
    return commands.vaultUnlockRecovery(recoveryKey, newPassword, rememberInKeyring);
  },
  async vaultChangePassword(currentPassword, newPassword) {
    if (!isTauriRuntime()) {
      void currentPassword;
      void newPassword;
      return;
    }
    await commands.vaultChangePassword(currentPassword, newPassword);
  },
  async vaultLock(forgetKeyring) {
    if (!isTauriRuntime()) {
      return {
        state: "Locked",
        remember_in_keyring: !forgetKeyring,
        idle_lock_minutes: 15,
        key_epoch: null,
        aead: null,
      };
    }
    return commands.vaultLock(forgetKeyring);
  },
  async syncStatus() {
    if (!isTauriRuntime()) {
      return {
        connected: false,
        phase: "Idle",
        queued: 0,
        synced: 0,
        conflicts: 0,
        last_ok_sec: null,
        last_error: null,
      };
    }
    return commands.syncStatus();
  },
  async driveConnect() {
    if (!isTauriRuntime()) return;
    await commands.driveConnect();
  },
  async driveDisconnect() {
    if (!isTauriRuntime()) return;
    await commands.driveDisconnect();
  },
  async driveCheckRemoteKeyfile() {
    if (!isTauriRuntime()) return false;
    return commands.driveCheckRemoteKeyfile();
  },
  async driveImportRemoteKeyfile() {
    if (!isTauriRuntime()) return;
    await commands.driveImportRemoteKeyfile();
  },
  async syncNow() {
    if (!isTauriRuntime()) {
      return { uploaded: 0, skipped: 0, tombstoned: 0, conflicts_detected: 0, errors: 0 };
    }
    return commands.syncNow();
  },
  async syncPull() {
    if (!isTauriRuntime()) {
      return { pulled: 0, conflict_copies: 0, skipped: 0 };
    }
    return commands.syncPull();
  },
  async conflictsList(unresolvedOnly) {
    if (!isTauriRuntime()) return [];
    return commands.conflictsList(unresolvedOnly);
  },
  async conflictsResolve(conflictId, resolution) {
    if (!isTauriRuntime()) return;
    await commands.conflictsResolve(conflictId, resolution);
  },
  async restoreList() {
    if (!isTauriRuntime()) return [];
    return commands.restoreList();
  },
  async restoreRun(selections, targetDir, overwrite) {
    if (!isTauriRuntime()) {
      void selections;
      void targetDir;
      void overwrite;
      return { restored: 0, skipped_existing: 0, failed: 0 };
    }
    return commands.restoreRun(selections, targetDir, overwrite);
  },
};
