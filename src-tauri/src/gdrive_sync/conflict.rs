use std::collections::HashMap;

use crate::gdrive_sync::manifest::Manifest;
use crate::indexer::db::{IndexDb, NewConflict, SyncStateRow};

/// Checks remote manifests against local state using 3-way comparison:
/// conflict if `local != base && remote != base && local != remote`.
pub fn detect_conflicts(
    db: &IndexDb,
    own_device_id: &str,
    remote_manifests: &[Manifest],
    sync_states: &HashMap<String, SyncStateRow>,
    local_shas: &HashMap<String, [u8; 32]>,
) -> usize {
    let existing = db.list_conflicts(true).unwrap_or_default();
    let mut already_recorded: HashMap<(String, String), bool> = HashMap::new();
    for c in existing {
        already_recorded.insert((c.file_id, c.remote_sha256_hex), true);
    }

    let mut detected = 0usize;
    for m in remote_manifests {
        if m.device_id == own_device_id {
            continue;
        }
        for (file_id, entry) in &m.entries {
            if entry.deleted_at.is_some() {
                continue;
            }
            let Some(&local_sha) = local_shas.get(file_id) else {
                continue;
            };
            let base_sha = sync_states.get(file_id).and_then(|s| s.last_synced_sha256);

            let local_changed = base_sha != Some(local_sha);
            let remote_changed = base_sha != Some(entry.sha256);
            let diverged = local_sha != entry.sha256;

            if local_changed && remote_changed && diverged {
                let remote_hex = hex::encode(entry.sha256);
                if already_recorded.contains_key(&(file_id.clone(), remote_hex.clone())) {
                    continue;
                }
                let _ = db.insert_conflict(NewConflict {
                    file_id: file_id.clone(),
                    root_uuid: entry.root_uuid.clone(),
                    rel_path: entry.rel_path.clone(),
                    local_sha256: Some(local_sha),
                    remote_sha256: entry.sha256,
                    remote_device: m.device_name.clone(),
                    remote_blob_name: entry.blob_name.clone(),
                    remote_blob_id: entry.blob_drive_id.clone(),
                });
                already_recorded.insert((file_id.clone(), remote_hex), true);
                detected += 1;
            }
        }
    }
    detected
}
