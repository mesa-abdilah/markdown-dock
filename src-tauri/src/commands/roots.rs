use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::events::IndexChangedEvent;
use crate::indexer::db::RootInfo;

pub fn managed_library_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Db(format!("app_data_dir unavailable: {e}")))?;
    let lib_dir = data_dir.join("library");
    std::fs::create_dir_all(&lib_dir)?;
    Ok(std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir))
}

fn validate_folder_name(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("Folder name cannot be empty".into()));
    }
    if trimmed.chars().count() > 64 {
        return Err(AppError::Config(
            "Folder name must be 64 characters or fewer".into(),
        ));
    }
    if trimmed.contains(['/', '\\', '\0']) || trimmed == "." || trimmed == ".." {
        return Err(AppError::Security(
            "Folder name cannot contain path separators or traversal segments".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn folder_slug(name: &str) -> String {
    let mut slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "folder".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn create_managed_root_in_dir(
    db: &crate::indexer::db::IndexDb,
    lib_dir: &Path,
    raw_name: &str,
) -> Result<RootInfo, AppError> {
    let label = validate_folder_name(raw_name)?;
    let existing = db.list_roots()?;
    if existing
        .iter()
        .any(|r| r.label.eq_ignore_ascii_case(&label))
    {
        return Err(AppError::Config(format!(
            "Folder \"{label}\" already exists"
        )));
    }

    std::fs::create_dir_all(lib_dir)?;
    let base_slug = folder_slug(&label);
    let mut folder_dir = lib_dir.join(&base_slug);
    if folder_dir.exists() {
        let suffix = &Uuid::new_v4().simple().to_string()[..6];
        folder_dir = lib_dir.join(format!("{base_slug}-{suffix}"));
    }
    std::fs::create_dir_all(&folder_dir)?;
    let canon = std::fs::canonicalize(&folder_dir).unwrap_or(folder_dir);
    let canon_str = canon.to_string_lossy().into_owned();

    let mut root = db.add_root(canon_str, label, Vec::new())?;
    db.set_root_backup(i64::from(root.id), true)?;
    root.backup = true;
    Ok(root)
}

pub fn create_managed_folder_internal(
    app: &AppHandle,
    state: &AppState,
    raw_name: &str,
) -> Result<RootInfo, AppError> {
    let lib_dir = managed_library_dir(app)?;
    let root = create_managed_root_in_dir(&state.db, &lib_dir, raw_name)?;
    state.watcher.watch_and_scan_root(root.clone())?;

    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(app);
    }
    Ok(root)
}

pub fn ensure_default_folder(app: &AppHandle, state: &AppState) -> Result<RootInfo, AppError> {
    let existing = state.db.list_roots()?;
    if let Some(first) = existing.into_iter().next() {
        return Ok(first);
    }
    create_managed_folder_internal(app, state, "General")
}

#[tauri::command]
#[specta::specta]
pub fn roots_list(state: State<'_, AppState>) -> Result<Vec<RootInfo>, AppError> {
    state.db.list_roots()
}

#[tauri::command]
#[specta::specta]
pub fn roots_count_docs(state: State<'_, AppState>) -> Result<u32, AppError> {
    state.db.count_enabled_documents()
}

#[tauri::command]
#[specta::specta]
pub fn roots_create_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<RootInfo, AppError> {
    create_managed_folder_internal(&app, &state, &name)
}

#[tauri::command]
#[specta::specta]
pub fn roots_rename_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    root_id: i32,
    new_name: String,
) -> Result<RootInfo, AppError> {
    let label = validate_folder_name(&new_name)?;
    let existing = state.db.list_roots()?;
    if existing
        .iter()
        .any(|r| r.id != root_id && r.label.eq_ignore_ascii_case(&label))
    {
        return Err(AppError::Config(format!(
            "Folder \"{label}\" already exists"
        )));
    }
    let id64 = i64::from(root_id);
    state.db.rename_root(id64, label)?;
    let updated = state.db.get_root(id64)?;
    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    Ok(updated)
}

#[tauri::command]
#[specta::specta]
pub fn roots_add(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    label: Option<String>,
) -> Result<RootInfo, AppError> {
    let raw_path = Path::new(path.trim());
    if !raw_path.exists() || !raw_path.is_dir() {
        return Err(AppError::NotFound(format!(
            "Directory does not exist: {path}"
        )));
    }
    let canon = std::fs::canonicalize(raw_path)?;
    let canon_str = canon.to_string_lossy().into_owned();
    let derived_label = label
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            canon
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| canon_str.clone())
        });

    let root = state.db.add_root(canon_str, derived_label, Vec::new())?;
    state.watcher.watch_and_scan_root(root.clone())?;
    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    Ok(root)
}

#[tauri::command]
#[specta::specta]
pub fn roots_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    root_id: i32,
) -> Result<(), AppError> {
    let id64 = i64::from(root_id);
    let root = state.db.get_root(id64).ok();
    let _ = state.watcher.unwatch_root(id64);
    state.db.remove_root(id64)?;

    if let Some(r) = root
        && let Ok(lib_dir) = managed_library_dir(&app)
        && let Ok(root_canon) = std::fs::canonicalize(&r.path)
        && root_canon.starts_with(&lib_dir)
        && root_canon != lib_dir
    {
        let _ = std::fs::remove_dir_all(root_canon);
    }

    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    state.sync.notify_docs_changed();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn roots_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    root_id: i32,
    enabled: bool,
) -> Result<(), AppError> {
    let id64 = i64::from(root_id);
    state.db.set_root_enabled(id64, enabled)?;
    if enabled {
        let root = state.db.get_root(id64)?;
        state.watcher.watch_and_scan_root(root)?;
    } else {
        state.watcher.unwatch_root(id64)?;
    }
    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn roots_set_backup(
    state: State<'_, AppState>,
    root_id: i32,
    backup: bool,
) -> Result<(), AppError> {
    state.db.set_root_backup(i64::from(root_id), backup)?;
    if backup {
        state.sync.notify_docs_changed();
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn roots_rescan(state: State<'_, AppState>, root_id: Option<i32>) -> Result<u32, AppError> {
    match root_id {
        Some(id) => state.watcher.rescan_root(i64::from(id)),
        None => {
            let mut total = 0u32;
            for r in state.db.list_roots()? {
                if r.enabled {
                    total += state.watcher.rescan_root(i64::from(r.id))?;
                }
            }
            Ok(total)
        }
    }
}
