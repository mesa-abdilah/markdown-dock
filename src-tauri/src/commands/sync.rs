use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::gdrive_sync::manifest::RestoreCatalogItem;
use crate::gdrive_sync::restore::{PullReport, RestoreReport, RestoreSelection};
use crate::gdrive_sync::worker::{SyncRunReport, SyncStatusSnapshot};
use crate::indexer::db::ConflictInfo;

#[tauri::command]
#[specta::specta]
pub fn sync_status(state: State<'_, AppState>) -> SyncStatusSnapshot {
    state.sync.status()
}

#[tauri::command]
#[specta::specta]
pub async fn drive_connect(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let opener_handle = app.clone();
    state
        .sync
        .connect_drive(Some(&app), move |url| {
            opener_handle
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| AppError::Auth(format!("Failed to open browser: {e}")))
        })
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn drive_disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    state.sync.disconnect_drive(Some(&app)).await
}

#[tauri::command]
#[specta::specta]
pub async fn drive_check_remote_keyfile(state: State<'_, AppState>) -> Result<bool, AppError> {
    state.sync.check_remote_keyfile().await
}

#[tauri::command]
#[specta::specta]
pub async fn drive_import_remote_keyfile(state: State<'_, AppState>) -> Result<(), AppError> {
    state.sync.import_remote_keyfile().await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_now(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncRunReport, AppError> {
    state.sync.sync_now(Some(&app)).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_pull(app: AppHandle, state: State<'_, AppState>) -> Result<PullReport, AppError> {
    let report = state.sync.pull_remote_changes(Some(&app)).await?;
    // Rescan enabled roots so newly pulled files become searchable immediately.
    for root in state.db.list_roots()? {
        if root.enabled {
            let _ = state.watcher.rescan_root(i64::from(root.id));
        }
    }
    Ok(report)
}

#[tauri::command]
#[specta::specta]
pub fn conflicts_list(
    state: State<'_, AppState>,
    unresolved_only: bool,
) -> Result<Vec<ConflictInfo>, AppError> {
    state.db.list_conflicts(unresolved_only)
}

#[tauri::command]
#[specta::specta]
pub fn conflicts_resolve(
    state: State<'_, AppState>,
    conflict_id: i32,
    resolution: String,
) -> Result<(), AppError> {
    match resolution.as_str() {
        "keep_both" | "keep_local" | "keep_remote" => state
            .db
            .resolve_conflict(i64::from(conflict_id), resolution),
        _ => Err(AppError::Sync(format!(
            "Invalid conflict resolution: {resolution}"
        ))),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn restore_list(state: State<'_, AppState>) -> Result<Vec<RestoreCatalogItem>, AppError> {
    state.sync.list_restore_items().await
}

#[tauri::command]
#[specta::specta]
pub async fn restore_run(
    state: State<'_, AppState>,
    selections: Vec<RestoreSelection>,
    target_dir: String,
    overwrite: bool,
) -> Result<RestoreReport, AppError> {
    let path = PathBuf::from(target_dir.trim());
    state.sync.run_restore(&selections, path, overwrite).await
}
