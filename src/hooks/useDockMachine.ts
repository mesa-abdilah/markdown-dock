import { useCallback, useEffect } from "react";
import {
  events,
  type DockFocusRequestEvent,
  type DockModeChangedEvent,
  type DriveConnectionEvent,
  type HotkeyFallbackNoticeEvent,
  type HotkeyTriggeredEvent,
  type IndexChangedEvent,
  type IndexProgressEvent,
  type SyncStatusEvent_Serialize,
  type VaultStateChangedEvent,
} from "../bindings/commands";
import { useDockStore } from "../stores/dockStore";
import { useSearchStore } from "../stores/searchStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useSyncStore } from "../stores/syncStore";
import { useVaultStore } from "../stores/vaultStore";
import { useTauriEvent } from "./useTauriEvent";

export function useDockMachine() {
  const hydrateDock = useDockStore((s) => s.hydrate);
  const loadSettings = useSettingsStore((s) => s.load);
  const loadRoots = useSearchStore((s) => s.loadRoots);
  const runSearch = useSearchStore((s) => s.runSearch);
  const loadVaultStatus = useVaultStore((s) => s.loadStatus);
  const loadSyncStatus = useSyncStore((s) => s.loadStatus);

  useEffect(() => {
    void hydrateDock();
    void loadSettings();
    void loadVaultStatus();
    void loadSyncStatus();
    void loadRoots().then(() => runSearch());
  }, [hydrateDock, loadSettings, loadVaultStatus, loadSyncStatus, loadRoots, runSearch]);

  const onModeChanged = useCallback((payload: DockModeChangedEvent) => {
    useDockStore.getState().applyExternalModeEvent(payload);
  }, []);

  const onHotkeyTriggered = useCallback((payload: HotkeyTriggeredEvent) => {
    const store = useDockStore.getState();
    if (payload.targetMode === "Collapsed") {
      void store.collapse();
    } else if (
      payload.targetMode === "Expanded" ||
      payload.targetMode === "Wide" ||
      payload.targetMode === "Fullscreen"
    ) {
      void store.expandTo(payload.targetMode, { focusWindow: true });
    }
  }, []);

  const onHotkeyFallback = useCallback((payload: HotkeyFallbackNoticeEvent) => {
    useDockStore
      .getState()
      .pushNotice(
        `Hotkey ${payload.requested} was already in use; switched to ${payload.active}.`,
      );
    void useSettingsStore.getState().load();
  }, []);

  const onFocusRequest = useCallback((payload: DockFocusRequestEvent) => {
    if (payload.focusSearch) {
      useDockStore.getState().requestSearchFocus();
    }
  }, []);

  const onOpenSettings = useCallback(() => {
    const store = useDockStore.getState();
    void store.expandTo("Expanded", { focusWindow: true }).then(() => {
      store.toggleSettings(true);
    });
  }, []);

  const onIndexProgress = useCallback((payload: IndexProgressEvent) => {
    const search = useSearchStore.getState();
    if (payload.done >= payload.total) {
      search.setIndexingProgress(null);
    } else {
      search.setIndexingProgress({
        rootId: payload.rootId,
        done: payload.done,
        total: payload.total,
      });
    }
  }, []);

  const onIndexChanged = useCallback((payload: IndexChangedEvent) => {
    useDockStore.setState({ indexedCount: payload.totalDocs });
    const search = useSearchStore.getState();
    search.setIndexingProgress(null);
    void search.loadRoots();
    void search.runSearch();
  }, []);

  const onVaultStateChanged = useCallback((payload: VaultStateChangedEvent) => {
    useVaultStore.getState().applyExternalStatus(payload.state, payload.rememberInKeyring);
  }, []);

  const onSyncStatus = useCallback((payload: SyncStatusEvent_Serialize) => {
    useSyncStore.getState().applyStatusEvent(payload);
  }, []);

  const onDriveConnection = useCallback((payload: DriveConnectionEvent) => {
    useSyncStore.getState().applyConnectionEvent(payload.connected);
  }, []);

  useTauriEvent(events.dockModeChangedEvent.listen, onModeChanged);
  useTauriEvent(events.hotkeyTriggeredEvent.listen, onHotkeyTriggered);
  useTauriEvent(events.hotkeyFallbackNoticeEvent.listen, onHotkeyFallback);
  useTauriEvent(events.dockFocusRequestEvent.listen, onFocusRequest);
  useTauriEvent(events.openSettingsRequestEvent.listen, onOpenSettings);
  useTauriEvent(events.indexProgressEvent.listen, onIndexProgress);
  useTauriEvent(events.indexChangedEvent.listen, onIndexChanged);
  useTauriEvent(events.vaultStateChangedEvent.listen, onVaultStateChanged);
  useTauriEvent(events.syncStatusEvent.listen, onSyncStatus);
  useTauriEvent(events.driveConnectionEvent.listen, onDriveConnection);
}
