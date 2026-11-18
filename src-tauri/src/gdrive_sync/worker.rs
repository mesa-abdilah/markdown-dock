use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use md5::{Digest as Md5Digest, Md5};
use serde::Serialize;
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::{Semaphore, mpsc};

use crate::crypto::aead::encrypt_blob;
use crate::crypto::keys::VaultKeys;
use crate::crypto::keystore::VaultManager;
use crate::crypto::names::{blob_filename, derive_file_id, manifest_filename};
use crate::crypto::vault_format::AeadAlg;
use crate::error::AppError;
use crate::events::{DriveConnectionEvent, SyncStatusEvent, SyncWorkerPhase};
use crate::gdrive_sync::client::DriveClient;
use crate::gdrive_sync::conflict::detect_conflicts;
use crate::gdrive_sync::gc::collect_garbage_blobs;
use crate::gdrive_sync::manifest::{
    HistoryVersion, MAX_HISTORY_VERSIONS, Manifest, ManifestEntry, ManifestRoot, RestoreCatalogItem,
};
use crate::gdrive_sync::oauth::OAuthManager;
use crate::gdrive_sync::restore::{
    PullContext, PullReport, RestoreReport, RestoreSelection, fetch_all_manifests,
    list_restore_catalog, pull_remote_to_local_roots, restore_to_directory,
};
use crate::indexer::db::{BackupLocalDoc, IndexDb, SyncStateRow};

pub const VAULT_DRIVE_FOLDER_NAME: &str = "Dynamic Markdown Dock Vault";
pub const BLOBS_DRIVE_FOLDER_NAME: &str = "blobs";

#[derive(Debug, Clone, Serialize, Type)]
pub struct SyncStatusSnapshot {
    pub connected: bool,
    pub phase: SyncWorkerPhase,
    pub queued: u32,
    pub synced: u32,
    pub conflicts: u32,
    pub last_ok_sec: Option<i32>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SyncRunReport {
    pub uploaded: u32,
    pub skipped: u32,
    pub tombstoned: u32,
    pub conflicts_detected: u32,
    pub errors: u32,
}

enum WorkerMsg {
    DocsChanged,
    SyncNow(tokio::sync::oneshot::Sender<Result<SyncRunReport, AppError>>),
}

struct DriveLayout {
    vault_folder_id: String,
    blobs_folder_id: String,
}

struct UploadTaskInput {
    doc: BackupLocalDoc,
    file_id_bytes: [u8; 32],
    file_id_hex: String,
    prev_entry: Option<ManifestEntry>,
}

pub struct SyncManager {
    db: IndexDb,
    vault: Arc<VaultManager>,
    pub oauth: Arc<OAuthManager>,
    client: DriveClient,
    status: Mutex<SyncStatusSnapshot>,
    worker_tx: Mutex<Option<mpsc::Sender<WorkerMsg>>>,
    client_id: Mutex<String>,
    client_secret: Mutex<String>,
    retention_days: Mutex<u32>,
}

impl SyncManager {
    pub fn new(
        db: IndexDb,
        vault: Arc<VaultManager>,
        oauth: Arc<OAuthManager>,
        client: DriveClient,
        client_id: String,
        client_secret: String,
        retention_days: u32,
    ) -> Self {
        let connected = oauth.is_connected();
        let (queued, synced, conflicts) = db.sync_counts().unwrap_or((0, 0, 0));
        Self {
            db,
            vault,
            oauth,
            client,
            status: Mutex::new(SyncStatusSnapshot {
                connected,
                phase: SyncWorkerPhase::Idle,
                queued,
                synced,
                conflicts,
                last_ok_sec: None,
                last_error: None,
            }),
            worker_tx: Mutex::new(None),
            client_id: Mutex::new(client_id),
            client_secret: Mutex::new(client_secret),
            retention_days: Mutex::new(retention_days),
        }
    }

    pub fn update_oauth_config(
        &self,
        client_id: String,
        client_secret: String,
        retention_days: u32,
    ) {
        *self.client_id.lock().unwrap() = client_id;
        *self.client_secret.lock().unwrap() = client_secret;
        *self.retention_days.lock().unwrap() = retention_days;
    }

    pub fn spawn_background_worker(self: &Arc<Self>, app: AppHandle) {
        let (tx, mut rx) = mpsc::channel::<WorkerMsg>(32);
        *self.worker_tx.lock().unwrap() = Some(tx);

        let mgr = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut periodic = tokio::time::interval(Duration::from_secs(15 * 60));
            periodic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // Skip immediate initial tick to let startup settle; user/watcher triggers sync cleanly.
            periodic.tick().await;

            let mut coalesce_deadline: Option<tokio::time::Instant> = None;

            loop {
                tokio::select! {
                    maybe_msg = rx.recv() => {
                        match maybe_msg {
                            Some(WorkerMsg::DocsChanged) => {
                                coalesce_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(5));
                            }
                            Some(WorkerMsg::SyncNow(reply)) => {
                                coalesce_deadline = None;
                                let res = mgr.run_delta_backup(Some(&app)).await;
                                let _ = reply.send(res);
                            }
                            None => break,
                        }
                    }
                    _ = periodic.tick() => {
                        let _ = mgr.run_delta_backup(Some(&app)).await;
                    }
                    _ = async {
                        match coalesce_deadline {
                            Some(dl) => tokio::time::sleep_until(dl).await,
                            None => std::future::pending::<()>().await,
                        }
                    } => {
                        coalesce_deadline = None;
                        let _ = mgr.run_delta_backup(Some(&app)).await;
                    }
                }
            }
        });
    }

    pub fn notify_docs_changed(&self) {
        if let Some(ref tx) = *self.worker_tx.lock().unwrap() {
            let _ = tx.try_send(WorkerMsg::DocsChanged);
        }
    }

    pub fn status(&self) -> SyncStatusSnapshot {
        let (queued, synced, conflicts) = self.db.sync_counts().unwrap_or((0, 0, 0));
        let mut st = self.status.lock().unwrap();
        st.connected = self.oauth.is_connected();
        st.queued = queued;
        st.synced = synced;
        st.conflicts = conflicts;
        st.clone()
    }

    pub async fn connect_drive<F>(
        &self,
        app: Option<&AppHandle>,
        open_browser: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&str) -> Result<(), AppError>,
    {
        let (cid, csec) = {
            (
                self.client_id.lock().unwrap().clone(),
                self.client_secret.lock().unwrap().clone(),
            )
        };
        self.oauth
            .connect_interactive(&cid, &csec, open_browser)
            .await?;
        {
            let mut st = self.status.lock().unwrap();
            st.connected = true;
            st.last_error = None;
        }
        if let Some(h) = app {
            let _ = DriveConnectionEvent { connected: true }.emit(h);
        }
        Ok(())
    }

    pub async fn disconnect_drive(&self, app: Option<&AppHandle>) -> Result<(), AppError> {
        self.oauth.disconnect().await?;
        {
            let mut st = self.status.lock().unwrap();
            st.connected = false;
            st.phase = SyncWorkerPhase::Idle;
        }
        if let Some(h) = app {
            let _ = DriveConnectionEvent { connected: false }.emit(h);
        }
        Ok(())
    }

    /// Checks whether an existing `keyfile.vault` already exists on Google Drive (for connecting a new device).
    pub async fn check_remote_keyfile(&self) -> Result<bool, AppError> {
        let token = self.access_token().await?;
        let layout = self.ensure_drive_layout(&token).await?;
        let remote = self
            .client
            .find_in_parent(&token, Some(&layout.vault_folder_id), "keyfile.vault", None)
            .await?;
        Ok(remote.is_some())
    }

    /// Downloads `keyfile.vault` from Google Drive into local app data so a new device can unlock the vault.
    pub async fn import_remote_keyfile(&self) -> Result<(), AppError> {
        let token = self.access_token().await?;
        let layout = self.ensure_drive_layout(&token).await?;
        let remote = self
            .client
            .find_in_parent(&token, Some(&layout.vault_folder_id), "keyfile.vault", None)
            .await?
            .ok_or_else(|| AppError::NotFound("No keyfile.vault found on Google Drive".into()))?;
        let bytes = self.client.download_file(&token, &remote.id).await?;
        // Validate header before writing locally.
        crate::crypto::aead::read_keyfile_vault_id(&bytes)?;
        let local_kf = self.vault.keyfile_path();
        if let Some(parent) = local_kf.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = local_kf.with_extension("vault.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, local_kf)?;
        Ok(())
    }

    pub async fn sync_now(&self, app: Option<&AppHandle>) -> Result<SyncRunReport, AppError> {
        let maybe_tx = { self.worker_tx.lock().unwrap().clone() };
        if let Some(tx) = maybe_tx {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if tx.send(WorkerMsg::SyncNow(reply_tx)).await.is_ok() {
                return reply_rx
                    .await
                    .map_err(|_| AppError::Sync("Sync worker dropped reply".into()))?;
            }
        }
        self.run_delta_backup(app).await
    }

    pub async fn run_delta_backup(
        &self,
        app: Option<&AppHandle>,
    ) -> Result<SyncRunReport, AppError> {
        if !self.oauth.is_connected() {
            return Err(AppError::Auth("Google Drive is not connected".into()));
        }

        let (keys, vault_id, key_epoch, aead) = self
            .vault
            .with_unlocked(|k, vid, ep, aead| Ok((k.clone(), vid, ep, aead)))?;

        self.set_phase(app, SyncWorkerPhase::Syncing, None);

        let result = self
            .execute_delta_backup(&keys, vault_id, key_epoch, aead)
            .await;

        match result {
            Ok(report) => {
                let now = now_unix_sec() as i32;
                let mut st = self.status.lock().unwrap();
                st.last_ok_sec = Some(now);
                st.last_error = None;
                st.phase = SyncWorkerPhase::Idle;
                drop(st);
                self.emit_status(app);
                Ok(report)
            }
            Err(err) => {
                let msg = err.to_string();
                let phase = if msg.contains("network") || msg.contains("dns") {
                    SyncWorkerPhase::Offline
                } else {
                    SyncWorkerPhase::Error
                };
                self.set_phase(app, phase, Some(msg));
                Err(err)
            }
        }
    }

    async fn execute_delta_backup(
        &self,
        keys: &VaultKeys,
        vault_id: [u8; 16],
        key_epoch: u32,
        aead: AeadAlg,
    ) -> Result<SyncRunReport, AppError> {
        let token = self.access_token().await?;
        let layout = self.ensure_drive_layout(&token).await?;
        self.ensure_remote_keyfile(&token, &layout.vault_folder_id)
            .await?;

        let device_id = self.db.ensure_device_id()?;
        let device_name = detect_device_name();
        let own_manifest_name = manifest_filename(&keys.k_name, &device_id);

        // Fetch existing manifests & check Drive changes
        let all_manifests = fetch_all_manifests(
            &self.client,
            &token,
            &layout.vault_folder_id,
            keys,
            &vault_id,
        )
        .await?;
        self.advance_changes_token(&token).await?;

        let mut own_manifest = all_manifests
            .iter()
            .find(|m| m.device_id == device_id)
            .cloned()
            .unwrap_or_else(|| Manifest::empty(vault_id, device_id.clone(), device_name.clone()));

        let sync_states: HashMap<String, SyncStateRow> = self
            .db
            .list_sync_states()?
            .into_iter()
            .map(|s| (s.file_id.clone(), s))
            .collect();

        let backup_docs = self.db.list_backup_docs()?;
        let mut local_shas: HashMap<String, [u8; 32]> = HashMap::new();
        let mut present_ids = Vec::<String>::with_capacity(backup_docs.len());
        let mut dirty_tasks = Vec::<UploadTaskInput>::new();
        let mut skipped = 0u32;

        for doc in backup_docs {
            let fid_bytes = derive_file_id(&keys.k_name, &doc.root_uuid, &doc.rel_path);
            let fid_hex = hex::encode(fid_bytes);
            local_shas.insert(fid_hex.clone(), doc.sha256);
            present_ids.push(fid_hex.clone());

            let already_synced = sync_states
                .get(&fid_hex)
                .is_some_and(|s| s.status == "synced" && s.last_synced_sha256 == Some(doc.sha256));

            if already_synced {
                skipped += 1;
            } else {
                let prev_entry = own_manifest.entries.get(&fid_hex).cloned();
                dirty_tasks.push(UploadTaskInput {
                    doc,
                    file_id_bytes: fid_bytes,
                    file_id_hex: fid_hex,
                    prev_entry,
                });
            }
        }

        // 3-way conflict detection against other devices' manifests
        let conflicts_detected = detect_conflicts(
            &self.db,
            &device_id,
            &all_manifests,
            &sync_states,
            &local_shas,
        ) as u32;

        // Upload dirty blobs in parallel (max 3 concurrent)
        let sem = Arc::new(Semaphore::new(3));
        let mut join_set = tokio::task::JoinSet::new();

        for task in dirty_tasks {
            let permit = sem.clone().acquire_owned().await.unwrap();
            let client = self.client.clone();
            let token_c = token.clone();
            let blobs_id = layout.blobs_folder_id.clone();
            let keys_c = keys.clone();

            join_set.spawn(async move {
                let _permit = permit;
                upload_single_doc(&client, &token_c, &blobs_id, &keys_c, key_epoch, aead, task)
                    .await
            });
        }

        let mut uploaded = 0u32;
        let mut errors = 0u32;
        let mut manifest_dirty = false;
        let now = now_unix_sec();

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok((fid_hex, new_state, new_entry))) => {
                    self.db.upsert_sync_state(new_state)?;
                    own_manifest.entries.insert(fid_hex, new_entry);
                    uploaded += 1;
                    manifest_dirty = true;
                }
                Ok(Err((fid_hex, root_uuid, rel_path, err_msg))) => {
                    errors += 1;
                    let prev_state = sync_states.get(&fid_hex);
                    let prev_attempts = prev_state.map_or(0, |s| s.attempts);
                    let prev_synced = prev_state.and_then(|s| s.last_synced_sha256);
                    let _ = self.db.upsert_sync_state(SyncStateRow {
                        file_id: fid_hex,
                        root_uuid,
                        rel_path,
                        local_sha256: None,
                        last_synced_sha256: prev_synced,
                        blob_name: None,
                        blob_drive_id: None,
                        remote_md5: None,
                        status: "error".into(),
                        attempts: prev_attempts + 1,
                        next_attempt_at: Some(now + 60),
                        last_error: Some(err_msg),
                        updated_at: now,
                    });
                }
                Err(_) => {
                    errors += 1;
                }
            }
        }

        // Mark deleted local files as tombstones in own_manifest
        let present_set: std::collections::HashSet<String> = present_ids.into_iter().collect();
        let mut tombstoned = 0u32;
        for (fid, entry) in own_manifest.entries.iter_mut() {
            if entry.deleted_at.is_none() && !present_set.contains(fid) {
                entry.deleted_at = Some(now);
                entry.updated_at = now;
                tombstoned += 1;
                manifest_dirty = true;
                let _ = self.db.upsert_sync_state(SyncStateRow {
                    file_id: fid.clone(),
                    root_uuid: entry.root_uuid.clone(),
                    rel_path: entry.rel_path.clone(),
                    local_sha256: None,
                    last_synced_sha256: Some(entry.sha256),
                    blob_name: Some(entry.blob_name.clone()),
                    blob_drive_id: Some(entry.blob_drive_id.clone()),
                    remote_md5: None,
                    status: "tombstoned".into(),
                    attempts: 0,
                    next_attempt_at: None,
                    last_error: None,
                    updated_at: now,
                });
            }
        }

        // Update manifest roots & flush to Drive if changed (or never uploaded yet)
        let manifest_drive_id = self.db.sync_meta_get("manifest_drive_id")?;
        if manifest_dirty || manifest_drive_id.is_none() {
            own_manifest.generation += 1;
            own_manifest.updated_at = now;
            own_manifest.roots = self
                .db
                .list_roots()?
                .into_iter()
                .filter(|r| r.enabled && r.backup)
                .map(|r| ManifestRoot {
                    root_uuid: r.root_uuid,
                    label: r.label,
                })
                .collect();

            let enc_manifest = own_manifest.encode_encrypted(keys, key_epoch, aead)?;
            let remote_file = match manifest_drive_id {
                Some(ref mid) => {
                    self.client
                        .update_file_bytes(&token, mid, &enc_manifest)
                        .await?
                }
                None => {
                    if let Some(existing) = self
                        .client
                        .find_in_parent(
                            &token,
                            Some(&layout.vault_folder_id),
                            &own_manifest_name,
                            None,
                        )
                        .await?
                    {
                        self.client
                            .update_file_bytes(&token, &existing.id, &enc_manifest)
                            .await?
                    } else {
                        self.client
                            .upload_blob(
                                &token,
                                &layout.vault_folder_id,
                                &own_manifest_name,
                                &enc_manifest,
                            )
                            .await?
                    }
                }
            };
            self.db.sync_meta_set("manifest_drive_id", remote_file.id)?;
        }

        // Periodic GC (once per 24h)
        let last_gc = self
            .db
            .sync_meta_get("last_gc_at")?
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        if now - last_gc > 86_400 {
            let retention = *self.retention_days.lock().unwrap();
            let mut manifests_for_gc = all_manifests;
            if let Some(pos) = manifests_for_gc
                .iter()
                .position(|m| m.device_id == own_manifest.device_id)
            {
                manifests_for_gc[pos] = own_manifest;
            } else {
                manifests_for_gc.push(own_manifest);
            }
            let _ = collect_garbage_blobs(
                &self.client,
                &token,
                &layout.blobs_folder_id,
                &manifests_for_gc,
                retention,
            )
            .await;
            let _ = self.db.sync_meta_set("last_gc_at", now.to_string());
        }

        Ok(SyncRunReport {
            uploaded,
            skipped,
            tombstoned,
            conflicts_detected,
            errors,
        })
    }

    pub async fn pull_remote_changes(
        &self,
        app: Option<&AppHandle>,
    ) -> Result<PullReport, AppError> {
        if !self.oauth.is_connected() {
            return Err(AppError::Auth("Google Drive is not connected".into()));
        }
        let (keys, vault_id) = self
            .vault
            .with_unlocked(|k, vid, _, _| Ok((k.clone(), vid)))?;
        self.set_phase(app, SyncWorkerPhase::Pulling, None);

        let token = self.access_token().await?;
        let layout = self.ensure_drive_layout(&token).await?;
        let own_device_id = self.db.ensure_device_id()?;

        let library_dir = app
            .and_then(|h| crate::commands::roots::managed_library_dir(h).ok())
            .or_else(|| {
                self.db.list_roots().ok().and_then(|rs| {
                    rs.into_iter()
                        .filter_map(|r| PathBuf::from(r.path).parent().map(|p| p.to_path_buf()))
                        .next()
                })
            });

        let res = pull_remote_to_local_roots(PullContext {
            db: &self.db,
            client: &self.client,
            token: &token,
            vault_folder_id: &layout.vault_folder_id,
            keys: &keys,
            vault_id: &vault_id,
            own_device_id: &own_device_id,
            library_dir: library_dir.as_deref(),
        })
        .await;

        match res {
            Ok(report) => {
                self.set_phase(app, SyncWorkerPhase::Idle, None);
                Ok(report)
            }
            Err(err) => {
                self.set_phase(app, SyncWorkerPhase::Error, Some(err.to_string()));
                Err(err)
            }
        }
    }

    pub async fn list_restore_items(&self) -> Result<Vec<RestoreCatalogItem>, AppError> {
        let (keys, vault_id) = self
            .vault
            .with_unlocked(|k, vid, _, _| Ok((k.clone(), vid)))?;
        let token = self.access_token().await?;
        let layout = self.ensure_drive_layout(&token).await?;
        list_restore_catalog(
            &self.client,
            &token,
            &layout.vault_folder_id,
            &keys,
            &vault_id,
        )
        .await
    }

    pub async fn run_restore(
        &self,
        selections: &[RestoreSelection],
        target_dir: PathBuf,
        overwrite: bool,
    ) -> Result<RestoreReport, AppError> {
        let keys = self.vault.with_unlocked(|k, _, _, _| Ok(k.clone()))?;
        let token = self.access_token().await?;
        restore_to_directory(
            &self.client,
            &token,
            &keys,
            selections,
            &target_dir,
            overwrite,
        )
        .await
    }

    async fn access_token(&self) -> Result<String, AppError> {
        let (cid, csec) = {
            (
                self.client_id.lock().unwrap().clone(),
                self.client_secret.lock().unwrap().clone(),
            )
        };
        let tok = self.oauth.get_access_token(&cid, &csec).await?;
        Ok(tok.to_string())
    }

    async fn ensure_drive_layout(&self, token: &str) -> Result<DriveLayout, AppError> {
        if let (Some(vf), Some(bf)) = (
            self.db.sync_meta_get("drive_folder_id")?,
            self.db.sync_meta_get("drive_blobs_folder_id")?,
        ) {
            return Ok(DriveLayout {
                vault_folder_id: vf,
                blobs_folder_id: bf,
            });
        }

        let vault_folder = match self
            .client
            .find_in_parent(
                token,
                None,
                VAULT_DRIVE_FOLDER_NAME,
                Some("application/vnd.google-apps.folder"),
            )
            .await?
        {
            Some(f) => f,
            None => {
                self.client
                    .create_folder(token, None, VAULT_DRIVE_FOLDER_NAME)
                    .await?
            }
        };

        let blobs_folder = match self
            .client
            .find_in_parent(
                token,
                Some(&vault_folder.id),
                BLOBS_DRIVE_FOLDER_NAME,
                Some("application/vnd.google-apps.folder"),
            )
            .await?
        {
            Some(f) => f,
            None => {
                self.client
                    .create_folder(token, Some(&vault_folder.id), BLOBS_DRIVE_FOLDER_NAME)
                    .await?
            }
        };

        self.db.sync_meta_set("drive_folder_id", &vault_folder.id)?;
        self.db
            .sync_meta_set("drive_blobs_folder_id", &blobs_folder.id)?;

        Ok(DriveLayout {
            vault_folder_id: vault_folder.id,
            blobs_folder_id: blobs_folder.id,
        })
    }

    async fn ensure_remote_keyfile(
        &self,
        token: &str,
        vault_folder_id: &str,
    ) -> Result<(), AppError> {
        if self.db.sync_meta_get("keyfile_drive_id")?.is_some() {
            return Ok(());
        }
        if let Some(existing) = self
            .client
            .find_in_parent(token, Some(vault_folder_id), "keyfile.vault", None)
            .await?
        {
            self.db.sync_meta_set("keyfile_drive_id", existing.id)?;
            return Ok(());
        }
        let local_kf = self.vault.keyfile_path();
        if local_kf.exists() {
            let bytes = std::fs::read(local_kf)?;
            let uploaded = self
                .client
                .upload_blob(token, vault_folder_id, "keyfile.vault", &bytes)
                .await?;
            self.db.sync_meta_set("keyfile_drive_id", uploaded.id)?;
        }
        Ok(())
    }

    async fn advance_changes_token(&self, token: &str) -> Result<(), AppError> {
        match self.db.sync_meta_get("changes_page_token")? {
            None => {
                let start = self.client.get_start_page_token(token).await?;
                self.db.sync_meta_set("changes_page_token", start)?;
            }
            Some(mut page_tok) => loop {
                let page = self.client.list_changes(token, &page_tok).await?;
                if let Some(next) = page.next_page_token {
                    page_tok = next;
                } else if let Some(new_start) = page.new_start_page_token {
                    self.db.sync_meta_set("changes_page_token", new_start)?;
                    break;
                } else {
                    break;
                }
            },
        }
        Ok(())
    }

    fn set_phase(&self, app: Option<&AppHandle>, phase: SyncWorkerPhase, error: Option<String>) {
        {
            let mut st = self.status.lock().unwrap();
            st.phase = phase;
            st.last_error = error;
        }
        self.emit_status(app);
    }

    fn emit_status(&self, app: Option<&AppHandle>) {
        if let Some(handle) = app {
            let snap = self.status();
            let _ = SyncStatusEvent {
                phase: snap.phase,
                queued: snap.queued,
                synced: snap.synced,
                conflicts: snap.conflicts,
                last_ok_sec: snap.last_ok_sec,
                last_error: snap.last_error,
            }
            .emit(handle);
        }
    }
}

async fn upload_single_doc(
    client: &DriveClient,
    token: &str,
    blobs_folder_id: &str,
    keys: &VaultKeys,
    key_epoch: u32,
    aead: AeadAlg,
    task: UploadTaskInput,
) -> Result<(String, SyncStateRow, ManifestEntry), (String, String, String, String)> {
    let doc = task.doc;
    let abs_path = PathBuf::from(&doc.root_path)
        .join(doc.rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));

    let plaintext = match std::fs::read(&abs_path) {
        Ok(b) => b,
        Err(e) => {
            return Err((
                task.file_id_hex,
                doc.root_uuid,
                doc.rel_path,
                format!("Read error: {e}"),
            ));
        }
    };

    let (ciphertext, pt_sha) =
        match encrypt_blob(keys, key_epoch, aead, &task.file_id_bytes, &plaintext, true) {
            Ok(v) => v,
            Err(e) => {
                return Err((task.file_id_hex, doc.root_uuid, doc.rel_path, e.to_string()));
            }
        };

    let bname = blob_filename(&keys.k_name, &task.file_id_bytes, &pt_sha);
    let local_md5 = hex::encode(Md5::digest(&ciphertext));

    // Idempotency check: skip re-upload if blob already exists in blobs/
    let remote_file = match client
        .find_in_parent(token, Some(blobs_folder_id), &bname, None)
        .await
    {
        Ok(Some(existing)) => existing,
        Ok(None) => match client
            .upload_blob(token, blobs_folder_id, &bname, &ciphertext)
            .await
        {
            Ok(created) => {
                if let Some(ref rmd5) = created.md5_checksum
                    && !rmd5.eq_ignore_ascii_case(&local_md5)
                {
                    return Err((
                        task.file_id_hex,
                        doc.root_uuid,
                        doc.rel_path,
                        format!("Uploaded ciphertext MD5 mismatch ({rmd5} != {local_md5})"),
                    ));
                }
                created
            }
            Err(e) => {
                return Err((task.file_id_hex, doc.root_uuid, doc.rel_path, e.to_string()));
            }
        },
        Err(e) => {
            return Err((task.file_id_hex, doc.root_uuid, doc.rel_path, e.to_string()));
        }
    };

    let now = now_unix_sec();
    let mut history = task
        .prev_entry
        .as_ref()
        .map(|p| p.history.clone())
        .unwrap_or_default();
    if let Some(ref prev) = task.prev_entry
        && prev.sha256 != pt_sha
    {
        history.push(HistoryVersion {
            sha256: prev.sha256,
            blob_name: prev.blob_name.clone(),
            blob_drive_id: prev.blob_drive_id.clone(),
            at: prev.updated_at,
        });
        if history.len() > MAX_HISTORY_VERSIONS {
            let overflow = history.len() - MAX_HISTORY_VERSIONS;
            history.drain(0..overflow);
        }
    }

    let sync_row = SyncStateRow {
        file_id: task.file_id_hex.clone(),
        root_uuid: doc.root_uuid.clone(),
        rel_path: doc.rel_path.clone(),
        local_sha256: Some(pt_sha),
        last_synced_sha256: Some(pt_sha),
        blob_name: Some(bname.clone()),
        blob_drive_id: Some(remote_file.id.clone()),
        remote_md5: Some(local_md5),
        status: "synced".into(),
        attempts: 0,
        next_attempt_at: None,
        last_error: None,
        updated_at: now,
    };

    let entry = ManifestEntry {
        root_uuid: doc.root_uuid,
        rel_path: doc.rel_path,
        size: plaintext.len() as u64,
        mtime_ns: doc.mtime_ns,
        sha256: pt_sha,
        base_sha256: task.prev_entry.map(|p| p.sha256),
        blob_name: bname,
        blob_drive_id: remote_file.id,
        deleted_at: None,
        updated_at: now,
        history,
    };

    Ok((task.file_id_hex, sync_row, entry))
}

fn detect_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Desktop".into())
}

fn now_unix_sec() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
