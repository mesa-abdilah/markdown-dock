use std::collections::HashSet;

use crate::error::AppError;
use crate::gdrive_sync::client::DriveClient;
use crate::gdrive_sync::manifest::Manifest;

/// Deletes unreferenced immutable blobs from `blobs/` on Google Drive once past retention.
pub async fn collect_garbage_blobs(
    client: &DriveClient,
    token: &str,
    blobs_folder_id: &str,
    manifests: &[Manifest],
    retention_days: u32,
) -> Result<u32, AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = now - i64::from(retention_days) * 86_400;

    let mut live_blob_ids = HashSet::<String>::new();
    for m in manifests {
        for entry in m.entries.values() {
            if entry.deleted_at.is_none() || entry.updated_at >= cutoff {
                live_blob_ids.insert(entry.blob_drive_id.clone());
            }
            for hv in &entry.history {
                if hv.at >= cutoff {
                    live_blob_ids.insert(hv.blob_drive_id.clone());
                }
            }
        }
    }

    let remote_blobs = client.list_folder(token, blobs_folder_id).await?;
    let mut deleted = 0u32;
    for bf in remote_blobs {
        if !live_blob_ids.contains(&bf.id) && client.delete_file(token, &bf.id).await.is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}
