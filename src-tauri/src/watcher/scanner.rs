use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use walkdir::WalkDir;

use crate::error::AppError;
use crate::events::{IndexChangedEvent, IndexProgressEvent};
use crate::indexer::db::{IndexDb, IndexableDoc, RootInfo};
use crate::indexer::parser::{normalize_rel_path, parse_markdown};
use crate::watcher::filter::{ExcludeMatcher, is_markdown_file};

const MAX_INDEXABLE_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB guard

pub fn file_mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub fn build_indexable_doc(root_canon: &Path, abs_path: &Path) -> Option<IndexableDoc> {
    let meta = std::fs::metadata(abs_path).ok()?;
    if !meta.is_file() || meta.len() > MAX_INDEXABLE_BYTES {
        return None;
    }
    let rel = abs_path.strip_prefix(root_canon).ok()?;
    let rel_path = normalize_rel_path(rel);
    let file_name = abs_path.file_name()?.to_string_lossy().into_owned();
    let bytes = std::fs::read(abs_path).ok()?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha_slice = hasher.finalize();
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(&sha_slice);

    let parsed = parse_markdown(&bytes, &file_name);
    Some(IndexableDoc {
        rel_path,
        file_name,
        parsed,
        size: meta.len() as i64,
        mtime_ns: file_mtime_ns(&meta),
        sha256,
    })
}

pub fn scan_root(db: &IndexDb, root: &RootInfo, app: Option<&AppHandle>) -> Result<u32, AppError> {
    if !root.enabled {
        return Ok(0);
    }
    let root_canon = std::fs::canonicalize(&root.path)
        .map_err(|e| AppError::NotFound(format!("Cannot access root '{}': {e}", root.path)))?;
    let matcher = ExcludeMatcher::new(&root.exclude_globs);
    let existing = db.existing_file_stamps(i64::from(root.id))?;

    let mut present_rel = HashSet::<String>::new();
    let mut to_parse = Vec::<PathBuf>::new();

    for entry in WalkDir::new(&root_canon)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            match e.path().strip_prefix(&root_canon) {
                Ok(rel) => !matcher.is_excluded_rel(rel),
                Err(_) => false,
            }
        })
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_markdown_file(path) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(&root_canon) else {
            continue;
        };
        let rel_norm = normalize_rel_path(rel);
        present_rel.insert(rel_norm.clone());

        if let Ok(meta) = entry.metadata() {
            let size = meta.len() as i64;
            let mtime_ns = file_mtime_ns(&meta);
            if let Some(&(old_size, old_mtime)) = existing.get(&rel_norm)
                && old_size == size
                && old_mtime == mtime_ns
            {
                continue;
            }
        }

        to_parse.push(path.to_path_buf());
    }

    let total = to_parse.len() as u32;
    if total > 0
        && let Some(handle) = app
    {
        let _ = IndexProgressEvent {
            root_id: root.id,
            done: 0,
            total,
        }
        .emit(handle);
    }

    let parsed_docs: Vec<IndexableDoc> = to_parse
        .par_iter()
        .filter_map(|p| build_indexable_doc(&root_canon, p))
        .collect();

    let mut upserted_total = 0u32;
    for chunk in parsed_docs.chunks(500) {
        let count = db.upsert_batch(i64::from(root.id), chunk.to_vec())?;
        upserted_total += count;
        if let Some(handle) = app {
            let _ = IndexProgressEvent {
                root_id: root.id,
                done: upserted_total.min(total),
                total,
            }
            .emit(handle);
        }
    }

    let removed = db.reconcile_missing(i64::from(root.id), present_rel)?;
    if (upserted_total > 0 || removed > 0)
        && let Some(handle) = app
    {
        if let Ok(total_docs) = db.count_enabled_documents() {
            let _ = IndexChangedEvent { total_docs }.emit(handle);
        }
        if root.backup
            && let Some(state) = handle.try_state::<crate::app_state::AppState>()
        {
            state.sync.notify_docs_changed();
        }
    }

    Ok(upserted_total)
}
