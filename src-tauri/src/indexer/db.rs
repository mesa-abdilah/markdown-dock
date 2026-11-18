use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

use crate::error::AppError;
use crate::indexer::fuzzy::{DocMeta, FuzzyCache};
use crate::indexer::parser::ParsedDocument;

pub type ReadPool = Pool<SqliteConnectionManager>;

const PRAGMAS: &str = "
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA foreign_keys = ON;
  PRAGMA temp_store = MEMORY;
  PRAGMA mmap_size = 268435456;
  PRAGMA cache_size = -16000;
  PRAGMA busy_timeout = 5000;
";

#[derive(Debug, Clone, Serialize, Type)]
pub struct RootInfo {
    pub id: i32,
    pub root_uuid: String,
    pub path: String,
    pub label: String,
    pub enabled: bool,
    pub backup: bool,
    pub exclude_globs: Vec<String>,
    pub doc_count: u32,
}

#[derive(Debug, Clone)]
pub struct IndexableDoc {
    pub rel_path: String,
    pub file_name: String,
    pub parsed: ParsedDocument,
    pub size: i64,
    pub mtime_ns: i64,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct DocRecord {
    pub id: i64,
    pub root_id: i64,
    pub root_path: String,
    pub root_label: String,
    pub rel_path: String,
    pub file_name: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub size: i64,
    pub mtime_ns: i64,
    pub sha256: [u8; 32],
    pub word_count: u32,
}

#[derive(Debug, Clone)]
pub struct BackupLocalDoc {
    pub root_id: i64,
    pub root_uuid: String,
    pub root_path: String,
    pub root_label: String,
    pub rel_path: String,
    pub size: i64,
    pub mtime_ns: i64,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct SyncStateRow {
    pub file_id: String,
    pub root_uuid: String,
    pub rel_path: String,
    pub local_sha256: Option<[u8; 32]>,
    pub last_synced_sha256: Option<[u8; 32]>,
    pub blob_name: Option<String>,
    pub blob_drive_id: Option<String>,
    pub remote_md5: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ConflictInfo {
    pub id: i32,
    pub file_id: String,
    pub root_uuid: String,
    pub rel_path: String,
    pub local_sha256_hex: Option<String>,
    pub remote_sha256_hex: String,
    pub remote_device: String,
    pub remote_blob_name: String,
    pub remote_blob_id: String,
    pub detected_at: i32,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewConflict {
    pub file_id: String,
    pub root_uuid: String,
    pub rel_path: String,
    pub local_sha256: Option<[u8; 32]>,
    pub remote_sha256: [u8; 32],
    pub remote_device: String,
    pub remote_blob_name: String,
    pub remote_blob_id: String,
}

enum WriteJob {
    AddRoot {
        path: String,
        label: String,
        exclude_globs: Vec<String>,
        reply: mpsc::SyncSender<Result<RootInfo, AppError>>,
    },
    RemoveRoot {
        root_id: i64,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    SetRootEnabled {
        root_id: i64,
        enabled: bool,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    SetRootBackup {
        root_id: i64,
        backup: bool,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    UpdateRootExcludes {
        root_id: i64,
        exclude_globs: Vec<String>,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    UpsertBatch {
        root_id: i64,
        docs: Vec<IndexableDoc>,
        reply: mpsc::SyncSender<Result<u32, AppError>>,
    },
    DeleteByRelPaths {
        root_id: i64,
        rel_paths: Vec<String>,
        reply: mpsc::SyncSender<Result<u32, AppError>>,
    },
    ReconcileMissing {
        root_id: i64,
        present: HashSet<String>,
        reply: mpsc::SyncSender<Result<u32, AppError>>,
    },
    SyncMetaSet {
        key: String,
        value: String,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    UpsertSyncState {
        row: SyncStateRow,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    InsertConflict {
        conflict: NewConflict,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    ResolveConflict {
        conflict_id: i64,
        resolution: String,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    SetRootUuid {
        root_id: i64,
        root_uuid: String,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    RenameRoot {
        root_id: i64,
        label: String,
        reply: mpsc::SyncSender<Result<(), AppError>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct IndexDb {
    pool: ReadPool,
    writer_tx: Sender<WriteJob>,
    pub fuzzy_cache: Arc<FuzzyCache>,
}

impl IndexDb {
    pub fn open(db_path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut writer_conn = Connection::open(db_path).map_err(db_err)?;
        writer_conn.execute_batch(PRAGMAS).map_err(db_err)?;
        run_migrations(&mut writer_conn)?;

        let manager = SqliteConnectionManager::file(db_path).with_init(|c| {
            c.execute_batch(PRAGMAS)?;
            Ok(())
        });
        let pool = Pool::builder()
            .max_size(4)
            .build(manager)
            .map_err(|e| AppError::Db(format!("Pool init failed: {e}")))?;

        let fuzzy_cache = Arc::new(FuzzyCache::default());
        let initial_meta = {
            let conn = pool.get().map_err(pool_err)?;
            load_fuzzy_meta(&conn)?
        };
        fuzzy_cache.replace_all(initial_meta);

        let (tx, rx) = mpsc::channel::<WriteJob>();
        let cache_for_thread = Arc::clone(&fuzzy_cache);
        std::thread::Builder::new()
            .name("dmd-sqlite-writer".into())
            .spawn(move || writer_loop(writer_conn, rx, cache_for_thread))
            .map_err(|e| AppError::Db(format!("Failed to spawn writer thread: {e}")))?;

        Ok(Self {
            pool,
            writer_tx: tx,
            fuzzy_cache,
        })
    }

    pub fn read_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, AppError> {
        self.pool.get().map_err(pool_err)
    }

    pub fn add_root(
        &self,
        path: String,
        label: String,
        exclude_globs: Vec<String>,
    ) -> Result<RootInfo, AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::AddRoot {
                path,
                label,
                exclude_globs,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn remove_root(&self, root_id: i64) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::RemoveRoot { root_id, reply })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn set_root_enabled(&self, root_id: i64, enabled: bool) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::SetRootEnabled {
                root_id,
                enabled,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn set_root_backup(&self, root_id: i64, backup: bool) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::SetRootBackup {
                root_id,
                backup,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn set_root_uuid(&self, root_id: i64, root_uuid: String) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::SetRootUuid {
                root_id,
                root_uuid,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn rename_root(&self, root_id: i64, label: String) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::RenameRoot {
                root_id,
                label,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn update_root_excludes(
        &self,
        root_id: i64,
        exclude_globs: Vec<String>,
    ) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::UpdateRootExcludes {
                root_id,
                exclude_globs,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn upsert_batch(&self, root_id: i64, docs: Vec<IndexableDoc>) -> Result<u32, AppError> {
        if docs.is_empty() {
            return Ok(0);
        }
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::UpsertBatch {
                root_id,
                docs,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn delete_by_rel_paths(
        &self,
        root_id: i64,
        rel_paths: Vec<String>,
    ) -> Result<u32, AppError> {
        if rel_paths.is_empty() {
            return Ok(0);
        }
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::DeleteByRelPaths {
                root_id,
                rel_paths,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn reconcile_missing(
        &self,
        root_id: i64,
        present: HashSet<String>,
    ) -> Result<u32, AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::ReconcileMissing {
                root_id,
                present,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn list_roots(&self) -> Result<Vec<RootInfo>, AppError> {
        let conn = self.read_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT r.id, r.root_uuid, r.path, r.label, r.enabled, r.backup, r.exclude_globs,
                        (SELECT COUNT(*) FROM documents d WHERE d.root_id = r.id) AS doc_count
                 FROM roots r
                 ORDER BY r.added_at ASC, r.id ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let globs_raw: String = row.get(6)?;
                let globs: Vec<String> = serde_json::from_str(&globs_raw).unwrap_or_default();
                let doc_count: i64 = row.get(7)?;
                Ok(RootInfo {
                    id: id as i32,
                    root_uuid: row.get(1)?,
                    path: row.get(2)?,
                    label: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    backup: row.get::<_, i64>(5)? != 0,
                    exclude_globs: globs,
                    doc_count: doc_count.max(0) as u32,
                })
            })
            .map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn get_root(&self, root_id: i64) -> Result<RootInfo, AppError> {
        self.list_roots()?
            .into_iter()
            .find(|r| i64::from(r.id) == root_id)
            .ok_or_else(|| AppError::NotFound(format!("Root {root_id} not found")))
    }

    pub fn count_enabled_documents(&self) -> Result<u32, AppError> {
        let conn = self.read_conn()?;
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM documents d JOIN roots r ON r.id = d.root_id WHERE r.enabled = 1",
                [],
                |row| row.get(0),
            )
            .map_err(db_err)?;
        Ok(cnt.max(0) as u32)
    }

    pub fn existing_file_stamps(
        &self,
        root_id: i64,
    ) -> Result<HashMap<String, (i64, i64)>, AppError> {
        let conn = self.read_conn()?;
        let mut stmt = conn
            .prepare("SELECT rel_path, size, mtime_ns FROM documents WHERE root_id = ?1")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([root_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?),
                ))
            })
            .map_err(db_err)?;
        let mut map = HashMap::new();
        for item in rows {
            let (p, stamp) = item.map_err(db_err)?;
            map.insert(p, stamp);
        }
        Ok(map)
    }

    pub fn find_doc_id_by_rel_path(
        &self,
        root_id: i64,
        rel_path: &str,
    ) -> Result<Option<i32>, AppError> {
        let conn = self.read_conn()?;
        match conn.query_row(
            "SELECT id FROM documents WHERE root_id = ?1 AND rel_path = ?2",
            params![root_id, rel_path],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Ok(Some(id as i32)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(db_err(e)),
        }
    }

    pub fn get_document(&self, doc_id: i64) -> Result<DocRecord, AppError> {
        let conn = self.read_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT d.id, d.root_id, r.path, r.label, d.rel_path, d.file_name, d.title,
                        d.tags_text, d.body, d.size, d.mtime_ns, d.sha256, d.word_count
                 FROM documents d
                 JOIN roots r ON r.id = d.root_id
                 WHERE d.id = ?1 AND r.enabled = 1",
            )
            .map_err(db_err)?;

        stmt.query_row([doc_id], |row| {
            let sha_blob: Vec<u8> = row.get(11)?;
            let mut sha = [0u8; 32];
            if sha_blob.len() == 32 {
                sha.copy_from_slice(&sha_blob);
            }
            let tags_text: String = row.get(7)?;
            let tags = tags_text
                .split_whitespace()
                .map(std::string::ToString::to_string)
                .collect();
            Ok(DocRecord {
                id: row.get(0)?,
                root_id: row.get(1)?,
                root_path: row.get(2)?,
                root_label: row.get(3)?,
                rel_path: row.get(4)?,
                file_name: row.get(5)?,
                title: row.get(6)?,
                tags,
                body: row.get(8)?,
                size: row.get(9)?,
                mtime_ns: row.get(10)?,
                sha256: sha,
                word_count: row.get::<_, i64>(12)?.max(0) as u32,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("Document {doc_id} not found"))
            }
            other => db_err(other),
        })
    }

    /// Resolves a document to a canonical filesystem path and verifies it resides strictly inside its registered root.
    pub fn resolve_verified_doc_path(&self, doc_id: i64) -> Result<(DocRecord, PathBuf), AppError> {
        let rec = self.get_document(doc_id)?;
        let root_canon = std::fs::canonicalize(&rec.root_path).map_err(|e| {
            AppError::NotFound(format!(
                "Root directory unavailable ({}): {e}",
                rec.root_path
            ))
        })?;
        let candidate = root_canon.join(rec.rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let file_canon = std::fs::canonicalize(&candidate).map_err(|e| {
            AppError::NotFound(format!(
                "Markdown file missing ({}): {e}",
                candidate.display(),
            ))
        })?;
        if !file_canon.starts_with(&root_canon) {
            return Err(AppError::Security(
                "Resolved document path escapes registered root".into(),
            ));
        }
        Ok((rec, file_canon))
    }

    pub fn shutdown(&self) {
        let _ = self.writer_tx.send(WriteJob::Shutdown);
    }

    pub fn sync_meta_get(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = self.read_conn()?;
        match conn.query_row("SELECT value FROM sync_meta WHERE key = ?1", [key], |r| {
            r.get(0)
        }) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(db_err(e)),
        }
    }

    pub fn sync_meta_set(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::SyncMetaSet {
                key: key.into(),
                value: value.into(),
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn ensure_device_id(&self) -> Result<String, AppError> {
        if let Some(existing) = self.sync_meta_get("device_id")? {
            return Ok(existing);
        }
        let id = Uuid::new_v4().to_string();
        self.sync_meta_set("device_id", id.clone())?;
        Ok(id)
    }

    pub fn list_backup_docs(&self) -> Result<Vec<BackupLocalDoc>, AppError> {
        let conn = self.read_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT d.root_id, r.root_uuid, r.path, r.label, d.rel_path, d.size, d.mtime_ns, d.sha256
                 FROM documents d
                 JOIN roots r ON r.id = d.root_id
                 WHERE r.enabled = 1 AND r.backup = 1",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], |row| {
                let blob: Vec<u8> = row.get(7)?;
                let mut sha = [0u8; 32];
                if blob.len() == 32 {
                    sha.copy_from_slice(&blob);
                }
                Ok(BackupLocalDoc {
                    root_id: row.get(0)?,
                    root_uuid: row.get(1)?,
                    root_path: row.get(2)?,
                    root_label: row.get(3)?,
                    rel_path: row.get(4)?,
                    size: row.get(5)?,
                    mtime_ns: row.get(6)?,
                    sha256: sha,
                })
            })
            .map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn list_sync_states(&self) -> Result<Vec<SyncStateRow>, AppError> {
        let conn = self.read_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT file_id, root_uuid, rel_path, local_sha256, last_synced_sha256,
                        blob_name, blob_drive_id, remote_md5, status, attempts,
                        next_attempt_at, last_error, updated_at
                 FROM sync_state",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SyncStateRow {
                    file_id: row.get(0)?,
                    root_uuid: row.get(1)?,
                    rel_path: row.get(2)?,
                    local_sha256: opt_sha(row.get(3)?),
                    last_synced_sha256: opt_sha(row.get(4)?),
                    blob_name: row.get(5)?,
                    blob_drive_id: row.get(6)?,
                    remote_md5: row.get(7)?,
                    status: row.get(8)?,
                    attempts: row.get(9)?,
                    next_attempt_at: row.get(10)?,
                    last_error: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn upsert_sync_state(&self, row: SyncStateRow) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::UpsertSyncState { row, reply })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn list_conflicts(&self, unresolved_only: bool) -> Result<Vec<ConflictInfo>, AppError> {
        let conn = self.read_conn()?;
        let sql = if unresolved_only {
            "SELECT id, file_id, root_uuid, rel_path, local_sha256, remote_sha256,
                    remote_device, remote_blob_name, remote_blob_id, detected_at, resolution
             FROM conflicts WHERE resolution IS NULL ORDER BY detected_at DESC"
        } else {
            "SELECT id, file_id, root_uuid, rel_path, local_sha256, remote_sha256,
                    remote_device, remote_blob_name, remote_blob_id, detected_at, resolution
             FROM conflicts ORDER BY detected_at DESC"
        };
        let mut stmt = conn.prepare(sql).map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                let local: Option<Vec<u8>> = row.get(4)?;
                let remote: Vec<u8> = row.get(5)?;
                Ok(ConflictInfo {
                    id: row.get::<_, i64>(0)? as i32,
                    file_id: row.get(1)?,
                    root_uuid: row.get(2)?,
                    rel_path: row.get(3)?,
                    local_sha256_hex: local.map(hex::encode),
                    remote_sha256_hex: hex::encode(remote),
                    remote_device: row.get(6)?,
                    remote_blob_name: row.get(7)?,
                    remote_blob_id: row.get(8)?,
                    detected_at: row.get::<_, i64>(9)? as i32,
                    resolution: row.get(10)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn insert_conflict(&self, conflict: NewConflict) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::InsertConflict { conflict, reply })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn resolve_conflict(&self, conflict_id: i64, resolution: String) -> Result<(), AppError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.writer_tx
            .send(WriteJob::ResolveConflict {
                conflict_id,
                resolution,
                reply,
            })
            .map_err(|_| AppError::Db("Writer thread closed".into()))?;
        rx.recv()
            .map_err(|_| AppError::Db("Writer reply dropped".into()))?
    }

    pub fn sync_counts(&self) -> Result<(u32, u32, u32), AppError> {
        let conn = self.read_conn()?;
        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_state WHERE status IN ('pending','uploading','error')",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        let synced: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_state WHERE status = 'synced'",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        let conflicts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conflicts WHERE resolution IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        Ok((
            queued.max(0) as u32,
            synced.max(0) as u32,
            conflicts.max(0) as u32,
        ))
    }
}

fn opt_sha(blob: Option<Vec<u8>>) -> Option<[u8; 32]> {
    blob.and_then(|v| v.try_into().ok())
}

fn run_migrations(conn: &mut Connection) -> Result<(), AppError> {
    let migrations = Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_init.sql")),
        M::up(include_str!("../../migrations/0002_sync.sql")),
    ]);
    migrations
        .to_latest(conn)
        .map_err(|e| AppError::Db(format!("Migration failed: {e}")))
}

fn writer_loop(mut conn: Connection, rx: mpsc::Receiver<WriteJob>, fuzzy_cache: Arc<FuzzyCache>) {
    while let Ok(job) = rx.recv() {
        match job {
            WriteJob::AddRoot {
                path,
                label,
                exclude_globs,
                reply,
            } => {
                let res = handle_add_root(&conn, &path, &label, &exclude_globs);
                let _ = reply.send(res);
            }
            WriteJob::RemoveRoot { root_id, reply } => {
                let res = conn
                    .execute("DELETE FROM roots WHERE id = ?1", [root_id])
                    .map(|_| ())
                    .map_err(db_err);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::SetRootEnabled {
                root_id,
                enabled,
                reply,
            } => {
                let res = conn
                    .execute(
                        "UPDATE roots SET enabled = ?2 WHERE id = ?1",
                        params![root_id, i32::from(enabled)],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::SetRootBackup {
                root_id,
                backup,
                reply,
            } => {
                let res = conn
                    .execute(
                        "UPDATE roots SET backup = ?2 WHERE id = ?1",
                        params![root_id, i32::from(backup)],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::SetRootUuid {
                root_id,
                root_uuid,
                reply,
            } => {
                let res = conn
                    .execute(
                        "UPDATE roots SET root_uuid = ?2 WHERE id = ?1",
                        params![root_id, root_uuid],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::RenameRoot {
                root_id,
                label,
                reply,
            } => {
                let res = conn
                    .execute(
                        "UPDATE roots SET label = ?2 WHERE id = ?1",
                        params![root_id, label],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::UpdateRootExcludes {
                root_id,
                exclude_globs,
                reply,
            } => {
                let json = serde_json::to_string(&exclude_globs).unwrap_or_else(|_| "[]".into());
                let res = conn
                    .execute(
                        "UPDATE roots SET exclude_globs = ?2 WHERE id = ?1",
                        params![root_id, json],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::UpsertBatch {
                root_id,
                docs,
                reply,
            } => {
                let res = handle_upsert_batch(&mut conn, root_id, &docs);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::DeleteByRelPaths {
                root_id,
                rel_paths,
                reply,
            } => {
                let res = handle_delete_rel_paths(&mut conn, root_id, &rel_paths);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::ReconcileMissing {
                root_id,
                present,
                reply,
            } => {
                let res = handle_reconcile_missing(&mut conn, root_id, &present);
                if res.is_ok()
                    && let Ok(meta) = load_fuzzy_meta(&conn)
                {
                    fuzzy_cache.replace_all(meta);
                }
                let _ = reply.send(res);
            }
            WriteJob::SyncMetaSet { key, value, reply } => {
                let res = conn
                    .execute(
                        "INSERT INTO sync_meta (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        params![key, value],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::UpsertSyncState { row, reply } => {
                let res = conn
                    .execute(
                        "INSERT INTO sync_state (
                            file_id, root_uuid, rel_path, local_sha256, last_synced_sha256,
                            blob_name, blob_drive_id, remote_md5, status, attempts,
                            next_attempt_at, last_error, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                         ON CONFLICT(file_id) DO UPDATE SET
                            root_uuid = excluded.root_uuid,
                            rel_path = excluded.rel_path,
                            local_sha256 = excluded.local_sha256,
                            last_synced_sha256 = excluded.last_synced_sha256,
                            blob_name = excluded.blob_name,
                            blob_drive_id = excluded.blob_drive_id,
                            remote_md5 = excluded.remote_md5,
                            status = excluded.status,
                            attempts = excluded.attempts,
                            next_attempt_at = excluded.next_attempt_at,
                            last_error = excluded.last_error,
                            updated_at = excluded.updated_at",
                        params![
                            row.file_id,
                            row.root_uuid,
                            row.rel_path,
                            row.local_sha256.as_ref().map(|s| s.as_slice()),
                            row.last_synced_sha256.as_ref().map(|s| s.as_slice()),
                            row.blob_name,
                            row.blob_drive_id,
                            row.remote_md5,
                            row.status,
                            row.attempts,
                            row.next_attempt_at,
                            row.last_error,
                            row.updated_at,
                        ],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::InsertConflict { conflict, reply } => {
                let now = now_unix_sec();
                let res = conn
                    .execute(
                        "INSERT INTO conflicts (
                            file_id, root_uuid, rel_path, local_sha256, remote_sha256,
                            remote_device, remote_blob_name, remote_blob_id, detected_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            conflict.file_id,
                            conflict.root_uuid,
                            conflict.rel_path,
                            conflict.local_sha256.as_ref().map(|s| s.as_slice()),
                            conflict.remote_sha256.as_slice(),
                            conflict.remote_device,
                            conflict.remote_blob_name,
                            conflict.remote_blob_id,
                            now,
                        ],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::ResolveConflict {
                conflict_id,
                resolution,
                reply,
            } => {
                let res = conn
                    .execute(
                        "UPDATE conflicts SET resolution = ?2 WHERE id = ?1",
                        params![conflict_id, resolution],
                    )
                    .map(|_| ())
                    .map_err(db_err);
                let _ = reply.send(res);
            }
            WriteJob::Shutdown => break,
        }
    }
}

fn handle_add_root(
    conn: &Connection,
    path: &str,
    label: &str,
    exclude_globs: &[String],
) -> Result<RootInfo, AppError> {
    let root_uuid = Uuid::new_v4().to_string();
    let now = now_unix_sec();
    let globs_json = serde_json::to_string(exclude_globs).unwrap_or_else(|_| "[]".into());

    conn.execute(
        "INSERT INTO roots (root_uuid, path, label, enabled, backup, exclude_globs, added_at)
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET label = excluded.label, enabled = 1",
        params![root_uuid, path, label, globs_json, now],
    )
    .map_err(db_err)?;

    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.root_uuid, r.path, r.label, r.enabled, r.backup, r.exclude_globs,
                    (SELECT COUNT(*) FROM documents d WHERE d.root_id = r.id)
             FROM roots r WHERE r.path = ?1",
        )
        .map_err(db_err)?;

    stmt.query_row([path], |row| {
        let globs_raw: String = row.get(6)?;
        Ok(RootInfo {
            id: row.get::<_, i64>(0)? as i32,
            root_uuid: row.get(1)?,
            path: row.get(2)?,
            label: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
            backup: row.get::<_, i64>(5)? != 0,
            exclude_globs: serde_json::from_str(&globs_raw).unwrap_or_default(),
            doc_count: row.get::<_, i64>(7)?.max(0) as u32,
        })
    })
    .map_err(db_err)
}

fn handle_upsert_batch(
    conn: &mut Connection,
    root_id: i64,
    docs: &[IndexableDoc],
) -> Result<u32, AppError> {
    let now = now_unix_sec();
    let mut count = 0u32;

    for chunk in docs.chunks(500) {
        let tx = conn.transaction().map_err(db_err)?;
        {
            let mut upsert_doc = tx
                .prepare_cached(
                    "INSERT INTO documents (
                        root_id, rel_path, file_name, title, tags_text, frontmatter_json,
                        body, size, mtime_ns, sha256, word_count, indexed_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                     ON CONFLICT(root_id, rel_path) DO UPDATE SET
                        file_name = excluded.file_name,
                        title = excluded.title,
                        tags_text = excluded.tags_text,
                        frontmatter_json = excluded.frontmatter_json,
                        body = excluded.body,
                        size = excluded.size,
                        mtime_ns = excluded.mtime_ns,
                        sha256 = excluded.sha256,
                        word_count = excluded.word_count,
                        indexed_at = excluded.indexed_at
                     WHERE documents.size != excluded.size
                        OR documents.mtime_ns != excluded.mtime_ns
                        OR documents.sha256 != excluded.sha256
                        OR documents.file_name != excluded.file_name
                     RETURNING id",
                )
                .map_err(db_err)?;

            let mut clear_tags = tx
                .prepare_cached("DELETE FROM document_tags WHERE doc_id = ?1")
                .map_err(db_err)?;
            let mut insert_tag = tx
                .prepare_cached("INSERT OR IGNORE INTO tags (name) VALUES (?1)")
                .map_err(db_err)?;
            let mut select_tag = tx
                .prepare_cached("SELECT id FROM tags WHERE name = ?1")
                .map_err(db_err)?;
            let mut link_tag = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO document_tags (doc_id, tag_id) VALUES (?1, ?2)",
                )
                .map_err(db_err)?;

            for doc in chunk {
                let Some(doc_id): Option<i64> = upsert_doc
                    .query_row(
                        params![
                            root_id,
                            doc.rel_path,
                            doc.file_name,
                            doc.parsed.title,
                            doc.parsed.tags_text,
                            doc.parsed.frontmatter_json,
                            doc.parsed.body,
                            doc.size,
                            doc.mtime_ns,
                            doc.sha256.as_slice(),
                            i64::from(doc.parsed.word_count),
                            now,
                        ],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db_err)?
                else {
                    continue;
                };

                clear_tags.execute([doc_id]).map_err(db_err)?;
                for tag in &doc.parsed.tags {
                    insert_tag.execute([tag]).map_err(db_err)?;
                    let tag_id: i64 = select_tag.query_row([tag], |r| r.get(0)).map_err(db_err)?;
                    link_tag.execute(params![doc_id, tag_id]).map_err(db_err)?;
                }
                count += 1;
            }
        }
        tx.commit().map_err(db_err)?;
    }

    Ok(count)
}

fn handle_delete_rel_paths(
    conn: &mut Connection,
    root_id: i64,
    rel_paths: &[String],
) -> Result<u32, AppError> {
    let tx = conn.transaction().map_err(db_err)?;
    let mut removed = 0u32;
    {
        let mut del = tx
            .prepare_cached("DELETE FROM documents WHERE root_id = ?1 AND rel_path = ?2")
            .map_err(db_err)?;
        for rp in rel_paths {
            let n = del.execute(params![root_id, rp]).map_err(db_err)?;
            removed += n as u32;
        }
    }
    tx.commit().map_err(db_err)?;
    Ok(removed)
}

fn handle_reconcile_missing(
    conn: &mut Connection,
    root_id: i64,
    present: &HashSet<String>,
) -> Result<u32, AppError> {
    let existing: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT rel_path FROM documents WHERE root_id = ?1")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([root_id], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?
    };

    let missing: Vec<String> = existing
        .into_iter()
        .filter(|rp| !present.contains(rp))
        .collect();
    if missing.is_empty() {
        return Ok(0);
    }
    handle_delete_rel_paths(conn, root_id, &missing)
}

fn load_fuzzy_meta(conn: &Connection) -> Result<Vec<DocMeta>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.root_id, d.file_name, d.title, d.tags_text, d.rel_path, d.mtime_ns
             FROM documents d
             JOIN roots r ON r.id = d.root_id
             WHERE r.enabled = 1",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DocMeta {
                id: row.get(0)?,
                root_id: row.get(1)?,
                file_name: row.get(2)?,
                title: row.get(3)?,
                tags_text: row.get(4)?,
                rel_path: row.get(5)?,
                mtime_ns: row.get(6)?,
            })
        })
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn now_unix_sec() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Db(e.to_string())
}

fn pool_err(e: r2d2::Error) -> AppError {
    AppError::Db(e.to_string())
}
