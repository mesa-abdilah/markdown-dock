use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::app_state::AppState;
use crate::commands::roots::ensure_default_folder;
use crate::error::AppError;
use crate::events::IndexChangedEvent;
use crate::indexer::parser::parse_markdown;
use crate::indexer::render::render_safe_html;
use crate::watcher::filter::is_markdown_file;
use crate::watcher::scanner::build_indexable_doc;

#[derive(Debug, Clone, Copy, Deserialize, Type)]
pub enum PreviewMode {
    Rendered,
    Raw,
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
pub enum CopyFormat {
    Raw,
    Html,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DocPreview {
    pub doc_id: i32,
    pub title: String,
    pub file_name: String,
    pub rel_path: String,
    pub root_label: String,
    pub tags: Vec<String>,
    pub word_count: u32,
    pub mtime_ms: f64,
    pub html: Option<String>,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DroppedPreview {
    pub file_name: String,
    pub path: String,
    pub parent_dir: String,
    pub title: String,
    pub tags: Vec<String>,
    pub word_count: u32,
    pub html: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct UploadReport {
    pub target_folder_id: i32,
    pub target_folder_label: String,
    pub uploaded_doc_ids: Vec<i32>,
    pub uploaded_count: u32,
    pub skipped_count: u32,
    pub rejected_dirs: u32,
    pub first_doc_id: Option<i32>,
}

#[tauri::command]
#[specta::specta]
pub async fn get_preview(
    state: State<'_, AppState>,
    doc_id: i32,
    mode: PreviewMode,
) -> Result<DocPreview, AppError> {
    let db = state.db.clone();
    let id64 = i64::from(doc_id);
    let rec = tokio::task::spawn_blocking(move || db.get_document(id64))
        .await
        .map_err(|e| AppError::Db(format!("Preview lookup failed: {e}")))??;

    // Read current file from verified path if available, falling back to indexed body.
    let raw_full = match state.db.resolve_verified_doc_path(id64) {
        Ok((_, canon_path)) => {
            std::fs::read_to_string(&canon_path).unwrap_or_else(|_| rec.body.clone())
        }
        Err(_) => rec.body.clone(),
    };

    let html = match mode {
        PreviewMode::Rendered => Some(state.render_cache.render_or_cached(
            rec.id,
            &rec.sha256,
            &rec.body,
        )),
        PreviewMode::Raw => None,
    };

    Ok(DocPreview {
        doc_id,
        title: rec.title,
        file_name: rec.file_name,
        rel_path: rec.rel_path,
        root_label: rec.root_label,
        tags: rec.tags,
        word_count: rec.word_count,
        mtime_ms: (rec.mtime_ns as f64) / 1_000_000.0,
        html,
        raw: raw_full,
    })
}

#[tauri::command]
#[specta::specta]
pub fn copy_document(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
    format: CopyFormat,
) -> Result<(), AppError> {
    let id64 = i64::from(doc_id);
    let (rec, canon_path) = state.db.resolve_verified_doc_path(id64)?;
    let raw = std::fs::read_to_string(&canon_path).unwrap_or(rec.body.clone());

    match format {
        CopyFormat::Raw => {
            app.clipboard()
                .write_text(raw)
                .map_err(|e| AppError::Window(format!("Clipboard write failed: {e}")))?;
        }
        CopyFormat::Html => {
            let html = state
                .render_cache
                .render_or_cached(rec.id, &rec.sha256, &rec.body);
            app.clipboard()
                .write_html(html, Some(raw))
                .map_err(|e| AppError::Window(format!("Clipboard HTML write failed: {e}")))?;
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn open_in_editor(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
) -> Result<(), AppError> {
    let (_, canon_path) = state.db.resolve_verified_doc_path(i64::from(doc_id))?;
    app.opener()
        .open_path(canon_path.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| AppError::Window(format!("Failed to open editor: {e}")))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn reveal_in_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
) -> Result<(), AppError> {
    let (_, canon_path) = state.db.resolve_verified_doc_path(i64::from(doc_id))?;
    app.opener()
        .reveal_item_in_dir(canon_path)
        .map_err(|e| AppError::Window(format!("Failed to reveal file: {e}")))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn open_external_url(app: AppHandle, url: String) -> Result<(), AppError> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("mailto:"))
    {
        return Err(AppError::Security(
            "Only http, https, and mailto URLs are permitted".into(),
        ));
    }
    app.opener()
        .open_url(trimmed, None::<&str>)
        .map_err(|e| AppError::Window(format!("Failed to open URL: {e}")))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn preview_dropped_file(path: String) -> Result<DroppedPreview, AppError> {
    let p = Path::new(path.trim());
    if !p.exists() || !p.is_file() {
        return Err(AppError::NotFound(format!(
            "Dropped file not found: {path}"
        )));
    }
    if !is_markdown_file(p) {
        return Err(AppError::Security(
            "Only .md and .markdown files can be previewed".into(),
        ));
    }
    let meta = std::fs::metadata(p)?;
    if meta.len() > 4 * 1024 * 1024 {
        return Err(AppError::Security(
            "Dropped file exceeds 4 MiB preview limit".into(),
        ));
    }
    let canon = std::fs::canonicalize(p)?;
    let bytes = std::fs::read(&canon)?;
    let file_name = canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dropped.md".into());
    let parent_dir = canon
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();

    let parsed = parse_markdown(&bytes, &file_name);
    let html = render_safe_html(&parsed.body);
    let raw = String::from_utf8_lossy(&bytes).into_owned();

    Ok(DroppedPreview {
        file_name,
        path: canon.to_string_lossy().into_owned(),
        parent_dir,
        title: parsed.title,
        tags: parsed.tags,
        word_count: parsed.word_count,
        html,
        raw,
    })
}

fn unique_dest_path(target_dir: &Path, original_name: &str) -> (PathBuf, String) {
    let safe_name = Path::new(original_name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "note.md".to_string());

    let candidate = target_dir.join(&safe_name);
    if !candidate.exists() {
        return (candidate, safe_name);
    }

    let path_obj = Path::new(&safe_name);
    let stem = path_obj
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "note".to_string());
    let ext = path_obj
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_else(|| ".md".to_string());

    for idx in 2..=10_000u32 {
        let next_name = format!("{stem} ({idx}){ext}");
        let next_path = target_dir.join(&next_name);
        if !next_path.exists() {
            return (next_path, next_name);
        }
    }
    let fallback_name = format!("{stem}-{}.md", uuid::Uuid::new_v4().simple());
    (target_dir.join(&fallback_name), fallback_name)
}

#[tauri::command]
#[specta::specta]
pub fn upload_markdown_files(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    folder_id: Option<i32>,
) -> Result<UploadReport, AppError> {
    let target = match folder_id {
        Some(id) => state.db.get_root(i64::from(id))?,
        None => {
            let roots = state.db.list_roots()?;
            if let Some(first) = roots.into_iter().find(|r| r.enabled) {
                first
            } else {
                ensure_default_folder(&app, &state)?
            }
        }
    };

    let target_dir = Path::new(&target.path);
    std::fs::create_dir_all(target_dir)?;

    let mut uploaded_rel_paths = Vec::new();
    let mut skipped_count = 0u32;
    let mut rejected_dirs = 0u32;

    for raw_path in paths {
        let p = Path::new(raw_path.trim());
        if p.is_dir() {
            rejected_dirs += 1;
            continue;
        }
        if !p.exists() || !p.is_file() || !is_markdown_file(p) {
            skipped_count += 1;
            continue;
        }
        let file_name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "note.md".to_string());
        let (dest_path, rel_name) = unique_dest_path(target_dir, &file_name);
        std::fs::copy(p, &dest_path)?;
        uploaded_rel_paths.push(rel_name);
    }

    let mut uploaded_doc_ids = Vec::with_capacity(uploaded_rel_paths.len());
    if !uploaded_rel_paths.is_empty() {
        state.watcher.rescan_root(i64::from(target.id))?;
        for rel in &uploaded_rel_paths {
            if let Some(doc_id) = state
                .db
                .find_doc_id_by_rel_path(i64::from(target.id), rel)?
            {
                uploaded_doc_ids.push(doc_id);
            }
        }
        if let Ok(total_docs) = state.db.count_enabled_documents() {
            let _ = IndexChangedEvent { total_docs }.emit(&app);
        }
        state.sync.notify_docs_changed();
    }

    let first_doc_id = uploaded_doc_ids.first().copied();
    Ok(UploadReport {
        target_folder_id: target.id,
        target_folder_label: target.label,
        uploaded_count: uploaded_doc_ids.len() as u32,
        uploaded_doc_ids,
        skipped_count,
        rejected_dirs,
        first_doc_id,
    })
}

#[tauri::command]
#[specta::specta]
pub fn move_document_to_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
    target_root_id: i32,
) -> Result<i32, AppError> {
    let (rec, src_path) = state.db.resolve_verified_doc_path(i64::from(doc_id))?;
    if rec.root_id == i64::from(target_root_id) {
        return Ok(doc_id);
    }

    let target_root = state.db.get_root(i64::from(target_root_id))?;
    let target_dir = Path::new(&target_root.path);
    std::fs::create_dir_all(target_dir)?;

    let (dest_path, rel_name) = unique_dest_path(target_dir, &rec.file_name);
    if std::fs::rename(&src_path, &dest_path).is_err() {
        std::fs::copy(&src_path, &dest_path)?;
        let _ = std::fs::remove_file(&src_path);
    }

    let _ = state
        .db
        .delete_by_rel_paths(rec.root_id, vec![rec.rel_path]);
    state.watcher.rescan_root(i64::from(target_root.id))?;
    let new_doc_id = state
        .db
        .find_doc_id_by_rel_path(i64::from(target_root.id), &rel_name)?
        .ok_or_else(|| AppError::Db("Moved file was not indexed in target folder".into()))?;

    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    state.sync.notify_docs_changed();
    Ok(new_doc_id)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_document(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
) -> Result<(), AppError> {
    let (rec, file_path) = state.db.resolve_verified_doc_path(i64::from(doc_id))?;
    if file_path.exists() {
        std::fs::remove_file(&file_path)?;
    }
    let _ = state
        .db
        .delete_by_rel_paths(rec.root_id, vec![rec.rel_path]);
    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    state.sync.notify_docs_changed();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_document(
    app: AppHandle,
    state: State<'_, AppState>,
    doc_id: i32,
    content: String,
    mode: PreviewMode,
) -> Result<DocPreview, AppError> {
    let id64 = i64::from(doc_id);
    let (rec, canon_path) = state.db.resolve_verified_doc_path(id64)?;
    let root_canon = std::fs::canonicalize(&rec.root_path)?;

    let content_bytes = content.into_bytes();
    let canon_clone = canon_path.clone();
    tokio::task::spawn_blocking(move || std::fs::write(&canon_clone, &content_bytes))
        .await
        .map_err(|e| AppError::Io(format!("Save task failed: {e}")))??;

    if let Some(indexable) = build_indexable_doc(&root_canon, &canon_path) {
        let _ = state.db.upsert_batch(rec.root_id, vec![indexable])?;
    }

    let updated = state.db.get_document(id64)?;
    let raw_full = std::fs::read_to_string(&canon_path).unwrap_or_else(|_| updated.body.clone());
    let html = match mode {
        PreviewMode::Rendered => Some(state.render_cache.render_or_cached(
            updated.id,
            &updated.sha256,
            &updated.body,
        )),
        PreviewMode::Raw => None,
    };

    if let Ok(total_docs) = state.db.count_enabled_documents() {
        let _ = IndexChangedEvent { total_docs }.emit(&app);
    }
    state.sync.notify_docs_changed();

    Ok(DocPreview {
        doc_id,
        title: updated.title,
        file_name: updated.file_name,
        rel_path: updated.rel_path,
        root_label: updated.root_label,
        tags: updated.tags,
        word_count: updated.word_count,
        mtime_ms: (updated.mtime_ns as f64) / 1_000_000.0,
        html,
        raw: raw_full,
    })
}
