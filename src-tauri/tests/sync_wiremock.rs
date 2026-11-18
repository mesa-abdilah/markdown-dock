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

#[tokio::test]
async fn cross_device_pull_maps_by_label_and_creates_missing_folders() {
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

    let base_url = format!("{}/drive/v3", server.uri());
    let upload_url = format!("{}/upload/drive/v3", server.uri());

    // --- Device A: sets up vault, creates "General" + "Work", backs up to Drive ---
    let tmp_a = TempDir::new().unwrap();
    let lib_a = tmp_a.path().join("library");
    let gen_a = lib_a.join("General");
    let work_a = lib_a.join("Work");
    fs::create_dir_all(&gen_a).unwrap();
    fs::create_dir_all(&work_a).unwrap();
    fs::write(gen_a.join("welcome.md"), "# Welcome from Windows").unwrap();
    fs::write(work_a.join("tasks.md"), "- [ ] Ship Ubuntu sync").unwrap();

    let db_a = IndexDb::open(&tmp_a.path().join("index.db")).unwrap();
    let root_gen_a = db_a
        .add_root(
            fs::canonicalize(&gen_a).unwrap().to_string_lossy().into_owned(),
            "General".into(),
            Vec::new(),
        )
        .unwrap();
    let root_work_a = db_a
        .add_root(
            fs::canonicalize(&work_a).unwrap().to_string_lossy().into_owned(),
            "Work".into(),
            Vec::new(),
        )
        .unwrap();
    db_a.set_root_backup(i64::from(root_gen_a.id), true).unwrap();
    db_a.set_root_backup(i64::from(root_work_a.id), true).unwrap();
    scan_root(&db_a, &db_a.get_root(i64::from(root_gen_a.id)).unwrap(), None).unwrap();
    scan_root(&db_a, &db_a.get_root(i64::from(root_work_a.id)).unwrap(), None).unwrap();

    let vault_a = Arc::new(VaultManager::init(tmp_a.path().join("keyfile.vault")));
    let pending = vault_a
        .begin_setup("super-strong-passphrase-2026!", AeadAlg::Aes256Gcm, false)
        .unwrap();
    vault_a.confirm_setup(None, &pending.recovery_suffix).unwrap();

    let oauth_a = Arc::new(OAuthManager::default());
    oauth_a.set_test_token("fake-access-token".into(), Some("fake-refresh".into()));
    let sync_a = SyncManager::new(
        db_a.clone(),
        Arc::clone(&vault_a),
        Arc::clone(&oauth_a),
        DriveClient::new(base_url.clone(), upload_url.clone()),
        "cid".into(),
        "csec".into(),
        30,
    );
    let up_report = sync_a.run_delta_backup(None).await.unwrap();
    assert_eq!(up_report.uploaded, 2);

    // --- Device B: fresh device with its OWN "General" root_uuid, imports remote keyfile ---
    let tmp_b = TempDir::new().unwrap();
    let lib_b = tmp_b.path().join("library");
    let gen_b = lib_b.join("General");
    fs::create_dir_all(&gen_b).unwrap();

    let db_b = IndexDb::open(&tmp_b.path().join("index.db")).unwrap();
    let root_gen_b = db_b
        .add_root(
            fs::canonicalize(&gen_b).unwrap().to_string_lossy().into_owned(),
            "General".into(),
            Vec::new(),
        )
        .unwrap();
    db_b.set_root_backup(i64::from(root_gen_b.id), true).unwrap();
    assert_ne!(root_gen_a.root_uuid, root_gen_b.root_uuid);

    let vault_b = Arc::new(VaultManager::init(tmp_b.path().join("keyfile.vault")));
    let oauth_b = Arc::new(OAuthManager::default());
    oauth_b.set_test_token("fake-access-token".into(), Some("fake-refresh".into()));
    let sync_b = SyncManager::new(
        db_b.clone(),
        Arc::clone(&vault_b),
        Arc::clone(&oauth_b),
        DriveClient::new(base_url, upload_url),
        "cid".into(),
        "csec".into(),
        30,
    );
    sync_b.import_remote_keyfile().await.unwrap();
    vault_b
        .unlock_with_password(None, "super-strong-passphrase-2026!", false)
        .unwrap();

    let pull1 = sync_b.pull_remote_changes(None).await.unwrap();
    assert_eq!(pull1.pulled, 2);
    assert_eq!(pull1.conflict_copies, 0);

    assert_eq!(
        fs::read_to_string(gen_b.join("welcome.md")).unwrap(),
        "# Welcome from Windows"
    );
    assert_eq!(
        fs::read_to_string(lib_b.join("Work").join("tasks.md")).unwrap(),
        "- [ ] Ship Ubuntu sync"
    );

    // Rescan Device B and verify idempotency on second pull & delta backup
    for r in db_b.list_roots().unwrap() {
        scan_root(&db_b, &r, None).unwrap();
    }
    let pull2 = sync_b.pull_remote_changes(None).await.unwrap();
    assert_eq!(pull2.pulled, 0);
    assert_eq!(pull2.skipped, 2);

    let backup_b = sync_b.run_delta_backup(None).await.unwrap();
    assert_eq!(backup_b.uploaded, 0);
    assert_eq!(backup_b.skipped, 2);

    db_a.shutdown();
    db_b.shutdown();
}
