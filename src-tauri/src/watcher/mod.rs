pub mod filter;
pub mod scanner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::event::{AccessKind, AccessMode, MetadataKind, ModifyKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::error::AppError;
use crate::events::IndexChangedEvent;
use crate::indexer::db::{IndexDb, RootInfo};
use crate::indexer::parser::normalize_rel_path;
use crate::watcher::filter::{ExcludeMatcher, is_markdown_file};

struct WatchedRoot {
    info: RootInfo,
    canon_path: PathBuf,
    matcher: ExcludeMatcher,
}

pub struct WatcherManager {
    db: IndexDb,
    app: AppHandle,
    debouncer: Mutex<Debouncer<RecommendedWatcher, RecommendedCache>>,
    watched: Arc<Mutex<HashMap<i64, WatchedRoot>>>,
}

impl WatcherManager {
    pub fn start(db: IndexDb, app: AppHandle) -> Result<Self, AppError> {
        let watched: Arc<Mutex<HashMap<i64, WatchedRoot>>> = Arc::new(Mutex::new(HashMap::new()));
        let (ev_tx, ev_rx) = std::sync::mpsc::channel::<Vec<DebouncedEvent>>();

        let debouncer = new_debouncer(
            Duration::from_millis(200),
            None,
            move |res: DebounceEventResult| match res {
                Ok(events) => {
                    let _ = ev_tx.send(events);
                }
                Err(errs) => {
                    for err in errs {
                        tracing::warn!("File watcher error: {err:?}");
                    }
                }
            },
        )
        .map_err(|e| AppError::Watcher(format!("Failed to initialize watcher: {e}")))?;

        let db_bg = db.clone();
        let app_bg = app.clone();
        let watched_bg = Arc::clone(&watched);
        std::thread::Builder::new()
            .name("dmd-watcher-events".into())
            .spawn(move || {
                while let Ok(events) = ev_rx.recv() {
                    process_debounced_events(&db_bg, &app_bg, &watched_bg, events);
                }
            })
            .map_err(|e| AppError::Watcher(format!("Failed to spawn watcher thread: {e}")))?;

        let mgr = Self {
            db,
            app,
            debouncer: Mutex::new(debouncer),
            watched,
        };

        mgr.sync_enabled_roots()?;
        Ok(mgr)
    }

    pub fn sync_enabled_roots(&self) -> Result<(), AppError> {
        let roots = self.db.list_roots()?;
        for root in roots {
            if root.enabled {
                if let Err(err) = self.watch_and_scan_root(root.clone()) {
                    tracing::warn!("Failed to watch root '{}': {err}", root.path);
                }
            } else {
                let _ = self.unwatch_root(i64::from(root.id));
            }
        }
        Ok(())
    }

    pub fn watch_and_scan_root(&self, root: RootInfo) -> Result<(), AppError> {
        if !root.enabled {
            return Ok(());
        }
        let canon = std::fs::canonicalize(&root.path).map_err(|e| {
            AppError::NotFound(format!("Root directory unavailable '{}': {e}", root.path))
        })?;
        let matcher = ExcludeMatcher::new(&root.exclude_globs);

        {
            let mut deb = self.debouncer.lock().unwrap();
            deb.watch(&canon, RecursiveMode::Recursive)
                .map_err(|e| AppError::Watcher(format!("Watch failed for '{}': {e}", root.path)))?;
        }

        {
            let mut map = self.watched.lock().unwrap();
            map.insert(
                i64::from(root.id),
                WatchedRoot {
                    info: root.clone(),
                    canon_path: canon,
                    matcher,
                },
            );
        }

        let db = self.db.clone();
        let app = self.app.clone();
        std::thread::Builder::new()
            .name(format!("dmd-scan-{}", root.id))
            .spawn(move || {
                if let Err(err) = scanner::scan_root(&db, &root, Some(&app)) {
                    tracing::error!("Cold scan failed for '{}': {err}", root.path);
                }
            })
            .map_err(|e| AppError::Watcher(format!("Failed to spawn scan thread: {e}")))?;

        Ok(())
    }

    pub fn unwatch_root(&self, root_id: i64) -> Result<(), AppError> {
        let removed = {
            let mut map = self.watched.lock().unwrap();
            map.remove(&root_id)
        };
        if let Some(wr) = removed {
            let mut deb = self.debouncer.lock().unwrap();
            let _ = deb.unwatch(&wr.canon_path);
        }
        Ok(())
    }

    pub fn rescan_root(&self, root_id: i64) -> Result<u32, AppError> {
        let root = self.db.get_root(root_id)?;
        scanner::scan_root(&self.db, &root, Some(&self.app))
    }

    /// Registers file watchers for any enabled roots added outside `WatcherManager` (e.g. during sync pull).
    pub fn watch_missing_enabled_roots(&self) -> Result<(), AppError> {
        let roots = self.db.list_roots()?;
        for root in roots {
            if !root.enabled {
                continue;
            }
            let already_watched = {
                let map = self.watched.lock().unwrap();
                map.contains_key(&i64::from(root.id))
            };
            if !already_watched && let Err(err) = self.watch_and_scan_root(root.clone()) {
                tracing::warn!("Failed to watch pulled root '{}': {err}", root.path);
            }
        }
        Ok(())
    }
}

fn process_debounced_events(
    db: &IndexDb,
    app: &AppHandle,
    watched: &Arc<Mutex<HashMap<i64, WatchedRoot>>>,
    events: Vec<DebouncedEvent>,
) {
    let snapshot: Vec<(i64, RootInfo, PathBuf, ExcludeMatcher)> = {
        let guard = watched.lock().unwrap();
        guard
            .iter()
            .map(|(&id, wr)| {
                (
                    id,
                    wr.info.clone(),
                    wr.canon_path.clone(),
                    wr.matcher.clone(),
                )
            })
            .collect()
    };

    let mut need_rescan_roots: Vec<RootInfo> = Vec::new();
    let mut seen_upsert_paths: HashMap<i64, std::collections::HashSet<PathBuf>> = HashMap::new();
    let mut upserts: HashMap<i64, Vec<crate::indexer::db::IndexableDoc>> = HashMap::new();
    let mut deletes: HashMap<i64, Vec<String>> = HashMap::new();

    for ev in events {
        if should_ignore_event_kind(ev.kind) {
            continue;
        }

        // Detect buffer overflow / rescan signal
        if ev.need_rescan() {
            for (_, info, _, _) in &snapshot {
                need_rescan_roots.push(info.clone());
            }
            continue;
        }

        for path in &ev.paths {
            // Canonicalize parent/path softly because deleted files cannot be canonicalized.
            let norm_path = soft_canonicalize(path);
            for (root_id, info, canon_root, matcher) in &snapshot {
                let Ok(rel) = norm_path.strip_prefix(canon_root) else {
                    continue;
                };
                if matcher.is_excluded_rel(rel) {
                    continue;
                }

                if norm_path.exists() {
                    if norm_path.is_dir() {
                        // A folder was created/renamed/moved in: reconcile this root.
                        need_rescan_roots.push(info.clone());
                    } else if is_markdown_file(&norm_path)
                        && seen_upsert_paths
                            .entry(*root_id)
                            .or_default()
                            .insert(norm_path.clone())
                        && let Some(doc) = scanner::build_indexable_doc(canon_root, &norm_path)
                    {
                        upserts.entry(*root_id).or_default().push(doc);
                    }
                } else if is_markdown_file(&norm_path) {
                    deletes
                        .entry(*root_id)
                        .or_default()
                        .push(normalize_rel_path(rel));
                } else {
                    // Possibly a directory deletion/move-out: trigger reconcile for safety.
                    need_rescan_roots.push(info.clone());
                }
            }
        }
    }

    let mut any_change = false;
    for (root_id, docs) in upserts {
        if let Ok(n) = db.upsert_batch(root_id, docs)
            && n > 0
        {
            any_change = true;
        }
    }
    for (root_id, rels) in deletes {
        if let Ok(n) = db.delete_by_rel_paths(root_id, rels)
            && n > 0
        {
            any_change = true;
        }
    }

    let mut seen_rescan = HashMap::<i32, RootInfo>::new();
    for r in need_rescan_roots {
        seen_rescan.entry(r.id).or_insert(r);
    }
    for (_, root) in seen_rescan {
        let _ = scanner::scan_root(db, &root, Some(app));
    }

    if any_change {
        if let Ok(total_docs) = db.count_enabled_documents() {
            let _ = IndexChangedEvent { total_docs }.emit(app);
        }
        if let Some(state) = app.try_state::<crate::app_state::AppState>() {
            state.sync.notify_docs_changed();
        }
    }
}

/// Filters out read/open/access-time events that would re-trigger indexing when the watcher or
/// scanner reads a file (notably Linux `inotify` `WatchMask::OPEN`).
fn should_ignore_event_kind(kind: EventKind) -> bool {
    match kind {
        EventKind::Other => true,
        EventKind::Access(access) => !matches!(access, AccessKind::Close(AccessMode::Write)),
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)) => true,
        _ => false,
    }
}

fn soft_canonicalize(path: &PathBuf) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && let Ok(cp) = std::fs::canonicalize(parent)
    {
        return cp.join(name);
    }
    path.clone()
}

#[cfg(test)]
mod tests {
    use notify::event::{CreateKind, DataChange, RemoveKind, RenameMode};

    use super::*;

    #[test]
    fn ignores_non_mutating_access_and_atime_events() {
        assert!(should_ignore_event_kind(EventKind::Other));
        assert!(should_ignore_event_kind(EventKind::Access(
            AccessKind::Open(AccessMode::Any)
        )));
        assert!(should_ignore_event_kind(EventKind::Access(
            AccessKind::Open(AccessMode::Read)
        )));
        assert!(should_ignore_event_kind(EventKind::Access(
            AccessKind::Read
        )));
        assert!(should_ignore_event_kind(EventKind::Access(
            AccessKind::Close(AccessMode::Read)
        )));
        assert!(should_ignore_event_kind(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::AccessTime)
        )));
    }

    #[test]
    fn allows_mutating_events() {
        assert!(!should_ignore_event_kind(EventKind::Access(
            AccessKind::Close(AccessMode::Write)
        )));
        assert!(!should_ignore_event_kind(EventKind::Create(
            CreateKind::File
        )));
        assert!(!should_ignore_event_kind(EventKind::Modify(
            ModifyKind::Data(DataChange::Any)
        )));
        assert!(!should_ignore_event_kind(EventKind::Modify(
            ModifyKind::Name(RenameMode::To)
        )));
        assert!(!should_ignore_event_kind(EventKind::Remove(
            RemoveKind::File
        )));
    }
}
