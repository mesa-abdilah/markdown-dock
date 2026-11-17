use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::crypto::aead::{decrypt_manifest, encrypt_manifest};
use crate::crypto::keys::VaultKeys;
use crate::crypto::vault_format::AeadAlg;
use crate::error::AppError;

pub const MAX_HISTORY_VERSIONS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRoot {
    pub root_uuid: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryVersion {
    pub sha256: [u8; 32],
    pub blob_name: String,
    pub blob_drive_id: String,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub root_uuid: String,
    pub rel_path: String,
    pub size: u64,
    pub mtime_ns: i64,
    pub sha256: [u8; 32],
    pub base_sha256: Option<[u8; 32]>,
    pub blob_name: String,
    pub blob_drive_id: String,
    pub deleted_at: Option<i64>,
    pub updated_at: i64,
    #[serde(default)]
    pub history: Vec<HistoryVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u8,
    pub vault_id: [u8; 16],
    pub device_id: String,
    pub device_name: String,
    pub generation: u64,
    pub updated_at: i64,
    pub roots: Vec<ManifestRoot>,
    pub entries: BTreeMap<String, ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct RestoreCatalogItem {
    pub file_id: String,
    pub root_uuid: String,
    pub root_label: String,
    pub rel_path: String,
    pub size_bytes: f64,
    pub updated_at_sec: i32,
    pub deleted: bool,
    pub device_name: String,
    pub sha256_hex: String,
    pub blob_drive_id: String,
    pub versions: Vec<RestoreVersionInfo>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct RestoreVersionInfo {
    pub sha256_hex: String,
    pub blob_drive_id: String,
    pub at_sec: i32,
}

impl Manifest {
    pub fn empty(vault_id: [u8; 16], device_id: String, device_name: String) -> Self {
        Self {
            format: 1,
            vault_id,
            device_id,
            device_name,
            generation: 0,
            updated_at: 0,
            roots: Vec::new(),
            entries: BTreeMap::new(),
        }
    }

    pub fn encode_encrypted(
        &self,
        keys: &VaultKeys,
        key_epoch: u32,
        aead: AeadAlg,
    ) -> Result<Vec<u8>, AppError> {
        let mut cbor = Vec::new();
        ciborium::ser::into_writer(self, &mut cbor)
            .map_err(|e| AppError::Sync(format!("Manifest CBOR encode failed: {e}")))?;
        encrypt_manifest(keys, key_epoch, aead, &self.vault_id, &cbor)
    }

    pub fn decode_encrypted(
        keys: &VaultKeys,
        vault_id: &[u8; 16],
        bytes: &[u8],
    ) -> Result<Self, AppError> {
        let cbor = decrypt_manifest(keys, vault_id, bytes)?;
        let m: Self = ciborium::de::from_reader(cbor.as_slice())
            .map_err(|e| AppError::Sync(format!("Manifest CBOR decode failed: {e}")))?;
        if m.format != 1 {
            return Err(AppError::Sync(format!(
                "Unsupported manifest format: {}",
                m.format
            )));
        }
        Ok(m)
    }
}

pub fn merge_manifests(manifests: &[Manifest]) -> Vec<RestoreCatalogItem> {
    let mut root_labels: BTreeMap<String, String> = BTreeMap::new();
    let mut best: BTreeMap<String, (&ManifestEntry, &str)> = BTreeMap::new();

    for m in manifests {
        for r in &m.roots {
            root_labels
                .entry(r.root_uuid.clone())
                .or_insert_with(|| r.label.clone());
        }
        for (fid, entry) in &m.entries {
            match best.get(fid) {
                Some((prev, _)) if prev.updated_at >= entry.updated_at => {}
                _ => {
                    best.insert(fid.clone(), (entry, m.device_name.as_str()));
                }
            }
        }
    }

    let mut items: Vec<RestoreCatalogItem> = best
        .into_iter()
        .map(|(file_id, (entry, device_name))| {
            let mut versions = Vec::with_capacity(1 + entry.history.len());
            versions.push(RestoreVersionInfo {
                sha256_hex: hex::encode(entry.sha256),
                blob_drive_id: entry.blob_drive_id.clone(),
                at_sec: entry.updated_at as i32,
            });
            for hv in entry.history.iter().rev() {
                versions.push(RestoreVersionInfo {
                    sha256_hex: hex::encode(hv.sha256),
                    blob_drive_id: hv.blob_drive_id.clone(),
                    at_sec: hv.at as i32,
                });
            }
            RestoreCatalogItem {
                file_id,
                root_uuid: entry.root_uuid.clone(),
                root_label: root_labels
                    .get(&entry.root_uuid)
                    .cloned()
                    .unwrap_or_else(|| "Folder".into()),
                rel_path: entry.rel_path.clone(),
                size_bytes: entry.size as f64,
                updated_at_sec: entry.updated_at as i32,
                deleted: entry.deleted_at.is_some(),
                device_name: device_name.to_string(),
                sha256_hex: hex::encode(entry.sha256),
                blob_drive_id: entry.blob_drive_id.clone(),
                versions,
            }
        })
        .collect();

    items.sort_by_key(|a| std::cmp::Reverse(a.updated_at_sec));
    items
}
