use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

use crate::window_manager::geometry::{AnchorEdge, DockMode};

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DockModeChangedEvent {
    pub mode: DockMode,
    pub pinned: bool,
    pub anchor: AnchorEdge,
    pub seq: u32,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyTriggeredEvent {
    pub shortcut: String,
    pub target_mode: DockMode,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyFallbackNoticeEvent {
    pub requested: String,
    pub active: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DockFocusRequestEvent {
    pub focus_search: bool,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct OpenSettingsRequestEvent;

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgressEvent {
    pub root_id: i32,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct IndexChangedEvent {
    pub total_docs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum VaultLockState {
    Uninitialized,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct VaultStateChangedEvent {
    pub state: VaultLockState,
    pub remember_in_keyring: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum SyncWorkerPhase {
    Idle,
    Syncing,
    Pulling,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusEvent {
    pub phase: SyncWorkerPhase,
    pub queued: u32,
    pub synced: u32,
    pub conflicts: u32,
    pub last_ok_sec: Option<i32>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DriveConnectionEvent {
    pub connected: bool,
}
