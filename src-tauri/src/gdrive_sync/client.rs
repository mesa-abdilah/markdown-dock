use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::gdrive_sync::backoff::{is_retryable_status, jittered_backoff, parse_retry_after};

const DEFAULT_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const DEFAULT_UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";
const MULTIPART_THRESHOLD: usize = 5 * 1024 * 1024; // 5 MiB
const MAX_RETRIES: u32 = 5;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub md5_checksum: Option<String>,
    #[serde(default)]
    pub modified_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileListResponse {
    #[serde(default)]
    files: Vec<DriveFile>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPageTokenResponse {
    start_page_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveChange {
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub removed: bool,
    #[serde(default)]
    pub file: Option<DriveFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesPage {
    #[serde(default)]
    pub changes: Vec<DriveChange>,
    pub next_page_token: Option<String>,
    pub new_start_page_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateMetadata<'a> {
    name: &'a str,
    mime_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parents: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_properties: Option<HashMap<&'a str, &'a str>>,
}

#[derive(Clone)]
pub struct DriveClient {
    http: Client,
    api_base: String,
    upload_base: String,
}

impl Default for DriveClient {
    fn default() -> Self {
        Self::new(
            DEFAULT_API_BASE.to_string(),
            DEFAULT_UPLOAD_BASE.to_string(),
        )
    }
}

impl DriveClient {
    pub fn new(api_base: String, upload_base: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client build");
        Self {
            http,
            api_base: api_base.trim_end_matches('/').to_string(),
            upload_base: upload_base.trim_end_matches('/').to_string(),
        }
    }

    pub async fn find_in_parent(
        &self,
        token: &str,
        parent_id: Option<&str>,
        name: &str,
        mime_type: Option<&str>,
    ) -> Result<Option<DriveFile>, AppError> {
        let escaped_name = name.replace('\\', "\\\\").replace('\'', "\\'");
        let mut q = format!("name = '{escaped_name}' and trashed = false");
        if let Some(pid) = parent_id {
            q.push_str(&format!(" and '{pid}' in parents"));
        } else {
            q.push_str(" and 'root' in parents");
        }
        if let Some(mt) = mime_type {
            q.push_str(&format!(" and mimeType = '{mt}'"));
        }

        let resp: FileListResponse = self
            .request_json(
                Method::GET,
                &format!("{}/files", self.api_base),
                token,
                Some(&[
                    ("q", q.as_str()),
                    ("fields", "files(id,name,mimeType,md5Checksum,modifiedTime)"),
                    ("pageSize", "5"),
                ]),
                None,
            )
            .await?;

        Ok(resp.files.into_iter().next())
    }

    pub async fn list_folder(
        &self,
        token: &str,
        parent_id: &str,
    ) -> Result<Vec<DriveFile>, AppError> {
        let mut all = Vec::new();
        let mut page_token: Option<String> = None;
        let q = format!("'{parent_id}' in parents and trashed = false");

        loop {
            let mut query = vec![
                ("q", q.as_str()),
                (
                    "fields",
                    "nextPageToken,files(id,name,mimeType,md5Checksum,modifiedTime)",
                ),
                ("pageSize", "200"),
            ];
            let pt_ref = page_token.clone();
            if let Some(ref pt) = pt_ref {
                query.push(("pageToken", pt.as_str()));
            }
            let page: FileListResponse = self
                .request_json(
                    Method::GET,
                    &format!("{}/files", self.api_base),
                    token,
                    Some(&query),
                    None,
                )
                .await?;
            all.extend(page.files);
            match page.next_page_token {
                Some(next) if !next.is_empty() => page_token = Some(next),
                _ => break,
            }
        }
        Ok(all)
    }

    pub async fn create_folder(
        &self,
        token: &str,
        parent_id: Option<&str>,
        name: &str,
    ) -> Result<DriveFile, AppError> {
        let mut app_props = HashMap::new();
        app_props.insert("dmd", "1");
        let meta = CreateMetadata {
            name,
            mime_type: "application/vnd.google-apps.folder",
            parents: parent_id.map(|p| vec![p]),
            app_properties: Some(app_props),
        };
        let body = serde_json::to_vec(&meta).map_err(|e| AppError::Sync(e.to_string()))?;
        self.request_json(
            Method::POST,
            &format!(
                "{}/files?fields=id,name,mimeType,md5Checksum,modifiedTime",
                self.api_base
            ),
            token,
            None,
            Some((body, "application/json")),
        )
        .await
    }

    /// Uploads an opaque binary `.vault` blob using multipart (<= 5 MiB) or resumable upload (> 5 MiB).
    /// Deliberately sets `mimeType: application/octet-stream` and omits description/properties so Drive cannot preview or index.
    pub async fn upload_blob(
        &self,
        token: &str,
        parent_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<DriveFile, AppError> {
        if bytes.len() <= MULTIPART_THRESHOLD {
            self.upload_multipart_create(token, parent_id, name, bytes)
                .await
        } else {
            self.upload_resumable_create(token, parent_id, name, bytes)
                .await
        }
    }

    async fn upload_multipart_create(
        &self,
        token: &str,
        parent_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<DriveFile, AppError> {
        let mut app_props = HashMap::new();
        app_props.insert("dmd", "1");
        let meta = CreateMetadata {
            name,
            mime_type: "application/octet-stream",
            parents: Some(vec![parent_id]),
            app_properties: Some(app_props),
        };
        let meta_json = serde_json::to_string(&meta).map_err(|e| AppError::Sync(e.to_string()))?;
        let boundary = "dmd_vault_multipart_boundary_7f3a9c";
        let body = build_multipart_body(boundary, &meta_json, bytes);
        let content_type = format!("multipart/related; boundary={boundary}");

        self.request_json(
            Method::POST,
            &format!(
                "{}/files?uploadType=multipart&fields=id,name,mimeType,md5Checksum,modifiedTime",
                self.upload_base
            ),
            token,
            None,
            Some((body, &content_type)),
        )
        .await
    }

    async fn upload_resumable_create(
        &self,
        token: &str,
        parent_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<DriveFile, AppError> {
        let mut app_props = HashMap::new();
        app_props.insert("dmd", "1");
        let meta = CreateMetadata {
            name,
            mime_type: "application/octet-stream",
            parents: Some(vec![parent_id]),
            app_properties: Some(app_props),
        };
        let meta_bytes = serde_json::to_vec(&meta).map_err(|e| AppError::Sync(e.to_string()))?;

        let init_url = format!(
            "{}/files?uploadType=resumable&fields=id,name,mimeType,md5Checksum,modifiedTime",
            self.upload_base
        );
        let init_resp = self
            .http
            .post(&init_url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json; charset=UTF-8")
            .header("X-Upload-Content-Type", "application/octet-stream")
            .header("X-Upload-Content-Length", bytes.len().to_string())
            .body(meta_bytes)
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("Resumable init failed: {e}")))?;

        if !init_resp.status().is_success() {
            let txt = init_resp.text().await.unwrap_or_default();
            return Err(AppError::Sync(format!("Resumable session rejected: {txt}")));
        }

        let session_uri = init_resp
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Sync("Missing resumable Location header".into()))?
            .to_string();

        let total = bytes.len();
        let resp = self
            .http
            .put(&session_uri)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, total.to_string())
            .header(CONTENT_RANGE, format!("bytes 0-{}/{total}", total - 1))
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("Resumable PUT failed: {e}")))?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(AppError::Sync(format!("Resumable upload failed: {txt}")));
        }
        resp.json::<DriveFile>()
            .await
            .map_err(|e| AppError::Sync(format!("Invalid resumable response: {e}")))
    }

    pub async fn update_file_bytes(
        &self,
        token: &str,
        file_id: &str,
        bytes: &[u8],
    ) -> Result<DriveFile, AppError> {
        let url = format!(
            "{}/files/{file_id}?uploadType=media&fields=id,name,mimeType,md5Checksum,modifiedTime",
            self.upload_base
        );
        self.request_json(
            Method::PATCH,
            &url,
            token,
            None,
            Some((bytes.to_vec(), "application/octet-stream")),
        )
        .await
    }

    pub async fn download_file(&self, token: &str, file_id: &str) -> Result<Vec<u8>, AppError> {
        let url = format!("{}/files/{file_id}?alt=media", self.api_base);
        for attempt in 0..=MAX_RETRIES {
            let resp = self
                .http
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| AppError::Sync(format!("Download request failed: {e}")))?;

            let status = resp.status();
            if status.is_success() {
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| AppError::Sync(format!("Download body read failed: {e}")))?;
                return Ok(bytes.to_vec());
            }

            let retry_after = parse_retry_after(resp.headers());
            let body = resp.text().await.unwrap_or_default();
            if attempt < MAX_RETRIES && is_retryable_status(status, &body) {
                let delay = retry_after.unwrap_or_else(|| jittered_backoff(attempt, 400, 16_000));
                tokio::time::sleep(delay).await;
                continue;
            }
            return Err(AppError::Sync(format!(
                "Download failed ({status}): {body}"
            )));
        }
        Err(AppError::Sync("Download exceeded max retries".into()))
    }

    pub async fn delete_file(&self, token: &str, file_id: &str) -> Result<(), AppError> {
        let url = format!("{}/files/{file_id}", self.api_base);
        let resp = self
            .http
            .delete(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("Delete failed: {e}")))?;
        if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            let txt = resp.text().await.unwrap_or_default();
            Err(AppError::Sync(format!("Delete error: {txt}")))
        }
    }

    pub async fn get_start_page_token(&self, token: &str) -> Result<String, AppError> {
        let resp: StartPageTokenResponse = self
            .request_json(
                Method::GET,
                &format!("{}/changes/startPageToken", self.api_base),
                token,
                None,
                None,
            )
            .await?;
        Ok(resp.start_page_token)
    }

    pub async fn list_changes(
        &self,
        token: &str,
        page_token: &str,
    ) -> Result<ChangesPage, AppError> {
        self.request_json(
            Method::GET,
            &format!("{}/changes", self.api_base),
            token,
            Some(&[
                ("pageToken", page_token),
                (
                    "fields",
                    "nextPageToken,newStartPageToken,changes(fileId,removed,file(id,name,mimeType,md5Checksum,modifiedTime))",
                ),
                ("pageSize", "100"),
            ]),
            None,
        )
        .await
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        url: &str,
        token: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<(Vec<u8>, &str)>,
    ) -> Result<T, AppError> {
        for attempt in 0..=MAX_RETRIES {
            let mut req = self
                .http
                .request(method.clone(), url)
                .header(AUTHORIZATION, format!("Bearer {token}"));
            if let Some(q) = query {
                req = req.query(q);
            }
            if let Some((ref bytes, ctype)) = body {
                req = req.header(CONTENT_TYPE, ctype).body(bytes.clone());
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Sync(format!("Drive API network error: {e}")))?;

            let status = resp.status();
            if status.is_success() {
                return resp
                    .json::<T>()
                    .await
                    .map_err(|e| AppError::Sync(format!("Invalid Drive JSON: {e}")));
            }

            let retry_after = parse_retry_after(resp.headers());
            let txt = resp.text().await.unwrap_or_default();
            if attempt < MAX_RETRIES && is_retryable_status(status, &txt) {
                let delay = retry_after.unwrap_or_else(|| jittered_backoff(attempt, 400, 16_000));
                tokio::time::sleep(delay).await;
                continue;
            }
            return Err(AppError::Sync(format!("Drive API error ({status}): {txt}")));
        }
        Err(AppError::Sync("Drive API exceeded max retries".into()))
    }
}

fn build_multipart_body(boundary: &str, meta_json: &str, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(meta_json.len() + bytes.len() + 256);
    out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    out.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    out.extend_from_slice(meta_json.as_bytes());
    out.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
    out.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    out.extend_from_slice(bytes);
    out.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    out
}
