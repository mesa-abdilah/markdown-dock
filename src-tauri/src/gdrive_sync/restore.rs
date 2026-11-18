use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;

use crate::commands::roots::create_managed_root_in_dir;
use crate::crypto::aead::decrypt_blob;
use crate::crypto::keys::VaultKeys;
use crate::crypto::names::derive_file_id;
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

pub struct PullContext<'a> {
    pub db: &'a IndexDb,
    pub client: &'a DriveClient,
    pub token: &'a str,
    pub vault_folder_id: &'a str,
    pub keys: &'a VaultKeys,
    pub vault_id: &'a [u8; 16],
    pub own_device_id: &'a str,
    pub library_dir: Option<&'a Path>,
}

/// Explicit Sync / Pull: pulls newer files from remote device manifests into mapped local roots.
/// Matches remote folders by `root_uuid`, then by folder label (case-insensitive), and creates
/// missing managed folders inside `library_dir` when pulling from another device.
/// Divergent conflicts are saved side-by-side (`<stem>.conflict-<device>-<ts>.md`) instead of overwriting.
pub async fn pull_remote_to_local_roots(ctx: PullContext<'_>) -> Result<PullReport, AppError> {
    let PullContext {
        db,
        client,
        token,
        vault_folder_id,
        keys,
        vault_id,
        own_device_id,
        library_dir,
    } = ctx;
    let manifests = fetch_all_manifests(client, token, vault_folder_id, keys, vault_id).await?;
    let roots = db.list_roots()?;

    let mut by_uuid: HashMap<String, (String, PathBuf)> = HashMap::new();
    let mut by_label_lower: HashMap<String, (String, PathBuf)> = HashMap::new();
    let mut disabled_labels_lower: HashSet<String> = HashSet::new();

    for r in roots {
        if !r.enabled {
            disabled_labels_lower.insert(r.label.to_ascii_lowercase());
            continue;
        }
        if let Ok(canon) = std::fs::canonicalize(&r.path) {
            by_uuid.insert(r.root_uuid.clone(), (r.root_uuid.clone(), canon.clone()));
            by_label_lower
                .entry(r.label.to_ascii_lowercase())
                .or_insert((r.root_uuid, canon));
        }
    }

    let sync_map: HashMap<String, SyncStateRow> = db
        .list_sync_states()?
        .into_iter()
        .map(|s| (s.file_id.clone(), s))
        .collect();
    let local_docs = db.list_backup_docs()?;
    let mut local_shas: HashMap<String, [u8; 32]> = HashMap::new();
    for d in local_docs {
        let fid = derive_file_id(&keys.k_name, &d.root_uuid, &d.rel_path);
        local_shas.insert(hex::encode(fid), d.sha256);
    }

    // Pick newest remote entry per file_id across other devices and record remote root labels.
    let mut remote_root_labels: HashMap<String, String> = HashMap::new();
    let mut remote_best: HashMap<String, (crate::gdrive_sync::manifest::ManifestEntry, String)> =
        HashMap::new();
    for m in manifests {
        if m.device_id == own_device_id {
            continue;
        }
        for r in m.roots {
            remote_root_labels
                .entry(r.root_uuid)
                .or_insert(r.label);
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

    for (remote_file_id_hex, (entry, remote_device)) in remote_best {
        if entry.deleted_at.is_some() {
            report.skipped += 1;
            continue;
        }

        let target_root = if let Some(found) = by_uuid.get(&entry.root_uuid) {
            found.clone()
        } else {
            let remote_label = remote_root_labels
                .get(&entry.root_uuid)
                .map(String::as_str)
                .unwrap_or("General");
            let label_key = remote_label.to_ascii_lowercase();

            if disabled_labels_lower.contains(&label_key) {
                report.skipped += 1;
                continue;
            }

            if let Some(found_by_label) = by_label_lower.get(&label_key) {
                let cloned = found_by_label.clone();
                by_uuid.insert(entry.root_uuid.clone(), cloned.clone());
                cloned
            } else if let Some(lib_dir) = library_dir {
                match create_managed_root_in_dir(db, lib_dir, remote_label) {
                    Ok(created) => {
                        let canon = std::fs::canonicalize(&created.path)
                            .unwrap_or_else(|_| PathBuf::from(&created.path));
                        let pair = (created.root_uuid.clone(), canon);
                        by_uuid.insert(entry.root_uuid.clone(), pair.clone());
                        by_uuid.insert(created.root_uuid, pair.clone());
                        by_label_lower.insert(label_key, pair.clone());
                        pair
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Could not create local folder '{}' during pull: {err}",
                            remote_label
                        );
                        report.skipped += 1;
                        continue;
                    }
                }
            } else {
                report.skipped += 1;
                continue;
            }
        };

        let (local_root_uuid, root_canon) = target_root;
        let Some(dest) = safe_join_rel(&root_canon, &entry.rel_path) else {
            report.skipped += 1;
            continue;
        };

        let local_fid_bytes = derive_file_id(&keys.k_name, &local_root_uuid, &entry.rel_path);
        let local_fid_hex = hex::encode(local_fid_bytes);

        let local_sha = local_shas
            .get(&local_fid_hex)
            .copied()
            .or_else(|| sha256_if_exists(&dest));
        if local_sha == Some(entry.sha256) {
            report.skipped += 1;
            continue;
        }

        let base_sha = sync_map
            .get(&local_fid_hex)
            .or_else(|| sync_map.get(&remote_file_id_hex))
            .and_then(|s| s.last_synced_sha256);
        let remote_fid_bytes = hex_to_32(&remote_file_id_hex)?;

        // Fast-forward if file doesn't exist locally or local matches last synced base.
        let can_fast_forward = local_sha.is_none() || local_sha == base_sha;
        let plaintext = download_and_decrypt(
            client,
            token,
            keys,
            &entry.blob_drive_id,
            &remote_fid_bytes,
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
                file_id: local_fid_hex,
                root_uuid: local_root_uuid,
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

fn sha256_if_exists(path: &Path) -> Option<[u8; 32]> {
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Some(out)
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
