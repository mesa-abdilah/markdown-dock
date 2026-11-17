use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::crypto::aead::decrypt_blob;
use crate::crypto::keys::VaultKeys;
use crate::error::AppError;
use crate::gdrive_sync::client::DriveClient;
use crate::gdrive_sync::manifest::{Manifest, RestoreCatalogItem, merge_manifests};
use crate::indexer::db::{IndexDb, SyncStateRow};

#[derive(Debug, Clone, Deserialize, Type)]
pub struct RestoreSelection {
    pub file_id: String,
    pub rel_path: String,
    pub blob_drive_id: String,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct RestoreReport {
    pub restored: u32,
    pub skipped_existing: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct PullReport {
    pub pulled: u32,
    pub conflict_copies: u32,
    pub skipped: u32,
}

pub async fn fetch_all_manifests(
    client: &DriveClient,
    token: &str,
    vault_folder_id: &str,
    keys: &VaultKeys,
    vault_id: &[u8; 16],
) -> Result<Vec<Manifest>, AppError> {
    let files = client.list_folder(token, vault_folder_id).await?;
    let mut manifests = Vec::new();
    for f in files {
        if f.name.starts_with("m-") && f.name.ends_with(".vault") {
            let bytes = client.download_file(token, &f.id).await?;
            match Manifest::decode_encrypted(keys, vault_id, &bytes) {
                Ok(m) => manifests.push(m),
                Err(err) => {
                    tracing::warn!("Skipping undecryptable manifest '{}': {err}", f.name);
                }
            }
        }
    }
    Ok(manifests)
}

pub async fn list_restore_catalog(
    client: &DriveClient,
    token: &str,
    vault_folder_id: &str,
    keys: &VaultKeys,
    vault_id: &[u8; 16],
) -> Result<Vec<RestoreCatalogItem>, AppError> {
    let manifests = fetch_all_manifests(client, token, vault_folder_id, keys, vault_id).await?;
    Ok(merge_manifests(&manifests))
}

/// Restores selected remote versions into `target_dir` atomically, verifying AEAD + SHA-256.
pub async fn restore_to_directory(
    client: &DriveClient,
    token: &str,
    keys: &VaultKeys,
    selections: &[RestoreSelection],
    target_dir: &Path,
    overwrite: bool,
) -> Result<RestoreReport, AppError> {
    if !target_dir.exists() {
        std::fs::create_dir_all(target_dir)?;
    }
    let target_canon = std::fs::canonicalize(target_dir)?;

    let mut report = RestoreReport {
        restored: 0,
        skipped_existing: 0,
        failed: 0,
    };

    for sel in selections {
        let Ok(fid_bytes) = hex_to_32(&sel.file_id) else {
            report.failed += 1;
            continue;
        };
        let Ok(sha_bytes) = hex_to_32(&sel.sha256_hex) else {
            report.failed += 1;
            continue;
        };
        let Some(dest) = safe_join_rel(&target_canon, &sel.rel_path) else {
            report.failed += 1;
            continue;
        };

        if dest.exists() && !overwrite {
            report.skipped_existing += 1;
            continue;
        }

        match download_and_decrypt(
            client,
            token,
            keys,
            &sel.blob_drive_id,
            &fid_bytes,
            &sha_bytes,
        )
        .await
        {
            Ok(plaintext) => {
                atomic_write_file(&dest, &plaintext)?;
                report.restored += 1;
            }
            Err(err) => {
                tracing::error!("Restore failed for '{}': {err}", sel.rel_path);
                report.failed += 1;
            }
        }
    }

    Ok(report)
}

/// Explicit Sync / Pull: pulls newer files from remote device manifests into mapped local roots.
/// Divergent conflicts are saved side-by-side (`<stem>.conflict-<device>-<ts>.md`) instead of overwriting.
pub async fn pull_remote_to_local_roots(
    db: &IndexDb,
    client: &DriveClient,
    token: &str,
    vault_folder_id: &str,
    keys: &VaultKeys,
    vault_id: &[u8; 16],
    own_device_id: &str,
) -> Result<PullReport, AppError> {
    let manifests = fetch_all_manifests(client, token, vault_folder_id, keys, vault_id).await?;
    let roots = db.list_roots()?;
    let root_paths: HashMap<String, PathBuf> = roots
        .into_iter()
        .filter(|r| r.enabled)
        .filter_map(|r| {
            std::fs::canonicalize(&r.path)
                .ok()
                .map(|p| (r.root_uuid, p))
        })
        .collect();

    let sync_map: HashMap<String, SyncStateRow> = db
        .list_sync_states()?
        .into_iter()
        .map(|s| (s.file_id.clone(), s))
        .collect();
    let local_docs = db.list_backup_docs()?;
    let mut local_shas: HashMap<String, [u8; 32]> = HashMap::new();
    for d in local_docs {
        let fid = crate::crypto::names::derive_file_id(&keys.k_name, &d.root_uuid, &d.rel_path);
        local_shas.insert(hex::encode(fid), d.sha256);
    }

    // Pick newest remote entry per file_id across other devices
    let mut remote_best: HashMap<String, (crate::gdrive_sync::manifest::ManifestEntry, String)> =
        HashMap::new();
    for m in manifests {
        if m.device_id == own_device_id {
            continue;
        }
        for (fid, entry) in m.entries {
            match remote_best.get(&fid) {
                Some((prev, _)) if prev.updated_at >= entry.updated_at => {}
                _ => {
                    remote_best.insert(fid, (entry, m.device_name.clone()));
                }
            }
        }
    }

    let mut report = PullReport {
        pulled: 0,
        conflict_copies: 0,
        skipped: 0,
    };

    for (file_id_hex, (entry, remote_device)) in remote_best {
        if entry.deleted_at.is_some() {
            report.skipped += 1;
            continue;
        }
        let Some(root_canon) = root_paths.get(&entry.root_uuid) else {
            report.skipped += 1;
            continue;
        };
        let Some(dest) = safe_join_rel(root_canon, &entry.rel_path) else {
            report.skipped += 1;
            continue;
        };

        let local_sha = local_shas.get(&file_id_hex).copied();
        if local_sha == Some(entry.sha256) {
            report.skipped += 1;
            continue;
        }

        let base_sha = sync_map
            .get(&file_id_hex)
            .and_then(|s| s.last_synced_sha256);
        let fid_bytes = hex_to_32(&file_id_hex)?;

        // Fast-forward if file doesn't exist locally or local matches last synced base.
        let can_fast_forward = local_sha.is_none() || local_sha == base_sha;
        let plaintext = download_and_decrypt(
            client,
            token,
            keys,
            &entry.blob_drive_id,
            &fid_bytes,
            &entry.sha256,
        )
        .await?;

        if can_fast_forward {
            atomic_write_file(&dest, &plaintext)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            db.upsert_sync_state(SyncStateRow {
                file_id: file_id_hex,
                root_uuid: entry.root_uuid,
                rel_path: entry.rel_path,
                local_sha256: Some(entry.sha256),
                last_synced_sha256: Some(entry.sha256),
                blob_name: Some(entry.blob_name),
                blob_drive_id: Some(entry.blob_drive_id),
                remote_md5: None,
                status: "synced".into(),
                attempts: 0,
                next_attempt_at: None,
                last_error: None,
                updated_at: now,
            })?;
            report.pulled += 1;
        } else {
            // Conflict: preserve local file and write remote alongside with conflict suffix.
            let conflict_path = make_conflict_path(&dest, &remote_device);
            atomic_write_file(&conflict_path, &plaintext)?;
            report.conflict_copies += 1;
        }
    }

    Ok(report)
}

async fn download_and_decrypt(
    client: &DriveClient,
    token: &str,
    keys: &VaultKeys,
    blob_drive_id: &str,
    file_id: &[u8; 32],
    expected_sha: &[u8; 32],
) -> Result<Vec<u8>, AppError> {
    let enc = client.download_file(token, blob_drive_id).await?;
    decrypt_blob(keys, file_id, expected_sha, &enc)
}

pub fn atomic_write_file(dest: &Path, bytes: &[u8]) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_name = format!(
        ".{}.dmd-tmp",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    );
    let tmp = dest.with_file_name(tmp_name);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

fn make_conflict_path(orig: &Path, device: &str) -> PathBuf {
    let stem = orig.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let ext = orig.extension().and_then(|e| e.to_str()).unwrap_or("md");
    let safe_dev: String = device
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(16)
        .collect();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    orig.with_file_name(format!("{stem}.conflict-{safe_dev}-{ts}.{ext}"))
}

fn safe_join_rel(root_canon: &Path, rel_path: &str) -> Option<PathBuf> {
    let mut out = root_canon.to_path_buf();
    for part in rel_path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains(':')
        {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

fn hex_to_32(s: &str) -> Result<[u8; 32], AppError> {
    let bytes = hex::decode(s).map_err(|_| AppError::Crypto("Invalid hex identifier".into()))?;
    bytes
        .try_into()
        .map_err(|_| AppError::Crypto("Expected 32-byte hex value".into()))
}
