use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

use md5::{Digest, Md5};
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use dynamic_markdown_dock_lib::crypto::keystore::VaultManager;
use dynamic_markdown_dock_lib::crypto::vault_format::AeadAlg;
use dynamic_markdown_dock_lib::gdrive_sync::client::DriveClient;
use dynamic_markdown_dock_lib::gdrive_sync::oauth::OAuthManager;
use dynamic_markdown_dock_lib::gdrive_sync::restore::RestoreSelection;
use dynamic_markdown_dock_lib::gdrive_sync::worker::SyncManager;
use dynamic_markdown_dock_lib::indexer::db::IndexDb;
use dynamic_markdown_dock_lib::watcher::scanner::scan_root;

#[derive(Clone, Default)]
struct FakeDriveState {
    next_id: u64,
    files: HashMap<String, FakeDriveEntry>,
    raw_upload_bodies: Vec<Vec<u8>>,
}

#[derive(Clone)]
struct FakeDriveEntry {
    id: String,
    name: String,
    mime_type: String,
    parent: String,
    bytes: Vec<u8>,
}

struct FakeDriveResponder {
    state: Arc<Mutex<FakeDriveState>>,
}

impl Respond for FakeDriveResponder {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let path = req.url.path();
        let method = req.method.as_str();
        let mut st = self.state.lock().unwrap();

        if method == "GET" && path == "/drive/v3/changes/startPageToken" {
            return ResponseTemplate::new(200).set_body_json(json!({ "startPageToken": "1" }));
        }
        if method == "GET" && path == "/drive/v3/changes" {
            return ResponseTemplate::new(200)
                .set_body_json(json!({ "newStartPageToken": "2", "changes": [] }));
        }

        if method == "GET" && path == "/drive/v3/files" {
            let q = req
                .url
                .query_pairs()
                .find(|(k, _)| k == "q")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default();

            let matched: Vec<_> = st
                .files
                .values()
                .filter(|f| {
                    let parent_ok = q.split("'").nth(1).is_none_or(|_| {
                        if let Some(idx) = q.find(" in parents") {
                            let before = &q[..idx];
                            let pid = before
                                .trim_end_matches('\'')
                                .rsplit('\'')
                                .next()
                                .unwrap_or("");
                            pid == "root" || f.parent == pid
                        } else {
                            true
                        }
                    });
                    let name_ok = if let Some(rest) = q.strip_prefix("name = '") {
                        let target = rest.split('\'').next().unwrap_or("");
                        f.name == target
                    } else {
                        true
                    };
                    parent_ok && name_ok
                })
                .map(|f| {
                    let md5 = hex::encode(Md5::digest(&f.bytes));
                    json!({
                        "id": f.id,
                        "name": f.name,
                        "mimeType": f.mime_type,
                        "md5Checksum": md5,
                    })
                })
                .collect();

            return ResponseTemplate::new(200).set_body_json(json!({ "files": matched }));
        }

        if method == "POST" && path == "/drive/v3/files" {
            let meta: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            st.next_id += 1;
            let id = format!("drv_{}", st.next_id);
            let name = meta["name"].as_str().unwrap().to_string();
            let mime = meta["mimeType"].as_str().unwrap().to_string();
            let parent = meta["parents"][0].as_str().unwrap_or("root").to_string();
            st.files.insert(
                id.clone(),
                FakeDriveEntry {
                    id: id.clone(),
                    name: name.clone(),
                    mime_type: mime.clone(),
                    parent,
                    bytes: Vec::new(),
                },
            );
            return ResponseTemplate::new(200).set_body_json(json!({
                "id": id,
                "name": name,
                "mimeType": mime,
            }));
        }

        if method == "POST" && path == "/upload/drive/v3/files" {
            st.raw_upload_bodies.push(req.body.clone());
            let body = &req.body;
            // Parse multipart parts by finding \r\n\r\n separators
            let text_lossy = String::from_utf8_lossy(body);
            let json_start = text_lossy.find('{').unwrap();
            let json_end = text_lossy[json_start..].find("\r\n--").unwrap() + json_start;
            let meta: serde_json::Value =
                serde_json::from_str(&text_lossy[json_start..json_end]).unwrap();

            let oct_marker = b"Content-Type: application/octet-stream\r\n\r\n";
            let pos = body
                .windows(oct_marker.len())
                .position(|w| w == oct_marker)
                .unwrap()
                + oct_marker.len();
            let end_marker = b"\r\n--dmd_vault_multipart_boundary_7f3a9c--";
            let end_pos = body[pos..]
                .windows(end_marker.len())
                .position(|w| w == end_marker)
                .unwrap()
                + pos;
            let payload = body[pos..end_pos].to_vec();
            let md5 = hex::encode(Md5::digest(&payload));

            st.next_id += 1;
            let id = format!("drv_{}", st.next_id);
            let name = meta["name"].as_str().unwrap().to_string();
            let mime = meta["mimeType"].as_str().unwrap().to_string();
            let parent = meta["parents"][0].as_str().unwrap_or("root").to_string();
            st.files.insert(
                id.clone(),
                FakeDriveEntry {
                    id: id.clone(),
                    name: name.clone(),
                    mime_type: mime.clone(),
                    parent,
                    bytes: payload,
                },
            );
            return ResponseTemplate::new(200).set_body_json(json!({
                "id": id,
                "name": name,
                "mimeType": mime,
                "md5Checksum": md5,
            }));
        }

        if method == "PATCH" && path.starts_with("/upload/drive/v3/files/") {
            st.raw_upload_bodies.push(req.body.clone());
            let fid = path.trim_start_matches("/upload/drive/v3/files/");
            let md5 = hex::encode(Md5::digest(&req.body));
            if let Some(entry) = st.files.get_mut(fid) {
                entry.bytes = req.body.clone();
                return ResponseTemplate::new(200).set_body_json(json!({
                    "id": entry.id,
                    "name": entry.name,
                    "mimeType": entry.mime_type,
                    "md5Checksum": md5,
                }));
            }
            return ResponseTemplate::new(404);
        }

        if method == "GET" && path.starts_with("/drive/v3/files/") {
            let fid = path.trim_start_matches("/drive/v3/files/");
            if let Some(entry) = st.files.get(fid) {
                return ResponseTemplate::new(200).set_body_bytes(entry.bytes.clone());
            }
            return ResponseTemplate::new(404);
        }

        ResponseTemplate::new(404)
    }
}

#[tokio::test]
async fn zero_knowledge_delta_backup_and_restore_over_wiremock() {
    let server = MockServer::start().await;
    let fake_state = Arc::new(Mutex::new(FakeDriveState::default()));

    Mock::given(method("GET"))
        .and(path_regex(".*"))
        .respond_with(FakeDriveResponder {
            state: Arc::clone(&fake_state),
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(".*"))
        .respond_with(FakeDriveResponder {
            state: Arc::clone(&fake_state),
        })
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path_regex(".*"))
        .respond_with(FakeDriveResponder {
            state: Arc::clone(&fake_state),
        })
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let notes_dir = tmp.path().join("notes");
    fs::create_dir_all(&notes_dir).unwrap();

    let canary = "CANARY-ZERO-KNOWLEDGE-991a";
    let file1 = notes_dir.join(format!("{canary}-secret.md"));
    let file2 = notes_dir.join("meeting.md");
    fs::write(
        &file1,
        format!("# Secret\n\nBody contains {canary} plaintext."),
    )
    .unwrap();
    fs::write(&file2, "# Meeting\n\nPlain meeting notes.").unwrap();

    let db = IndexDb::open(&tmp.path().join("index.db")).unwrap();
    let root = db
        .add_root(
            fs::canonicalize(&notes_dir)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "Notes".into(),
            Vec::new(),
        )
        .unwrap();
    db.set_root_backup(i64::from(root.id), true).unwrap();
    let refreshed_root = db.get_root(i64::from(root.id)).unwrap();
    scan_root(&db, &refreshed_root, None).unwrap();

    let vault = Arc::new(VaultManager::init(tmp.path().join("keyfile.vault")));
    let pending = vault
        .begin_setup("super-strong-passphrase-2026!", AeadAlg::Aes256Gcm, false)
        .unwrap();
    vault.confirm_setup(None, &pending.recovery_suffix).unwrap();

    let oauth = Arc::new(OAuthManager::default());
    oauth.set_test_token("fake-access-token".into(), Some("fake-refresh".into()));

    let base_url = format!("{}/drive/v3", server.uri());
    let upload_url = format!("{}/upload/drive/v3", server.uri());
    let client = DriveClient::new(base_url, upload_url);

    let sync = SyncManager::new(
        db.clone(),
        Arc::clone(&vault),
        Arc::clone(&oauth),
        client,
        "cid".into(),
        "csec".into(),
        30,
    );

    // 1. Initial sync uploads 2 blobs + keyfile + manifest
    let r1 = sync.run_delta_backup(None).await.unwrap();
    assert_eq!(r1.uploaded, 2);
    assert_eq!(r1.errors, 0);

    // Verify ZERO plaintext canary leaked in any Drive request or filename, and every binary starts with DMDV
    {
        let st = fake_state.lock().unwrap();
        for body in &st.raw_upload_bodies {
            let as_str = String::from_utf8_lossy(body);
            assert!(
                !as_str.contains(canary),
                "Canary plaintext leaked in Drive upload!"
            );
        }
        for entry in st.files.values() {
            if entry.mime_type != "application/vnd.google-apps.folder" {
                assert_eq!(entry.mime_type, "application/octet-stream");
                assert!(!entry.name.contains(canary));
                assert!(entry.name.ends_with(".vault"));
                assert_eq!(&entry.bytes[..4], b"DMDV");
            }
        }
    }

    // 2. Modify only 1 file -> incremental delta sync uploads ONLY 1 blob
    fs::write(&file2, "# Meeting\n\nUpdated notes v2!").unwrap();
    scan_root(&db, &refreshed_root, None).unwrap();
    let r2 = sync.run_delta_backup(None).await.unwrap();
    assert_eq!(r2.uploaded, 1);
    assert_eq!(r2.skipped, 1);

    // 3. List restore catalog & restore to a clean directory
    let catalog = sync.list_restore_items().await.unwrap();
    assert_eq!(catalog.len(), 2);

    let restore_dir = tmp.path().join("restored");
    let selections: Vec<RestoreSelection> = catalog
        .iter()
        .map(|item| RestoreSelection {
            file_id: item.file_id.clone(),
            rel_path: item.rel_path.clone(),
            blob_drive_id: item.blob_drive_id.clone(),
            sha256_hex: item.sha256_hex.clone(),
        })
        .collect();
    let rest_report = sync
        .run_restore(&selections, restore_dir.clone(), false)
        .await
        .unwrap();
    assert_eq!(rest_report.restored, 2);
    assert_eq!(rest_report.failed, 0);

    assert_eq!(
        fs::read(restore_dir.join(format!("{canary}-secret.md"))).unwrap(),
        fs::read(&file1).unwrap()
    );
    assert_eq!(
        fs::read(restore_dir.join("meeting.md")).unwrap(),
        fs::read(&file2).unwrap()
    );

    db.shutdown();
}
