use std::sync::Mutex;
use std::time::{Duration, Instant};

use data_encoding::BASE64URL_NOPAD;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;
use zeroize::Zeroizing;

use crate::error::AppError;

pub const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const KEYRING_SERVICE: &str = "com.dynamicmarkdowndock.app";
const KEYRING_REFRESH_USER: &str = "gdrive-refresh-token-v1";

#[derive(Clone)]
struct CachedAccessToken {
    token: Zeroizing<String>,
    expires_at: Instant,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default = "default_expires")]
    expires_in: u64,
    refresh_token: Option<String>,
}

fn default_expires() -> u64 {
    3600
}

pub struct OAuthManager {
    http: Client,
    token_url: String,
    revoke_url: String,
    cached_access: Mutex<Option<CachedAccessToken>>,
    /// Test-only in-memory override so unit/wiremock tests never touch OS keyring.
    override_refresh: Mutex<Option<Zeroizing<String>>>,
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new(GOOGLE_TOKEN_URL.to_string(), GOOGLE_REVOKE_URL.to_string())
    }
}

impl OAuthManager {
    pub fn new(token_url: String, revoke_url: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build");
        Self {
            http,
            token_url,
            revoke_url,
            cached_access: Mutex::new(None),
            override_refresh: Mutex::new(None),
        }
    }

    pub fn set_test_token(&self, access_token: String, refresh_token: Option<String>) {
        *self.cached_access.lock().unwrap() = Some(CachedAccessToken {
            token: Zeroizing::new(access_token),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });
        if let Some(rt) = refresh_token {
            *self.override_refresh.lock().unwrap() = Some(Zeroizing::new(rt));
        }
    }

    pub fn is_connected(&self) -> bool {
        if self.override_refresh.lock().unwrap().is_some()
            || self.cached_access.lock().unwrap().is_some()
        {
            return true;
        }
        load_refresh_token().is_ok()
    }

    pub fn clear_cached_access(&self) {
        *self.cached_access.lock().unwrap() = None;
    }

    /// Runs the full Authorization Code + PKCE S256 loopback flow on `127.0.0.1:0`.
    pub async fn connect_interactive<F>(
        &self,
        client_id: &str,
        client_secret: &str,
        open_browser: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&str) -> Result<(), AppError>,
    {
        if client_id.trim().is_empty() {
            return Err(AppError::Auth(
                "Google OAuth Client ID is required. Configure it in Settings → Sync.".into(),
            ));
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| AppError::Auth(format!("Loopback bind failed: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| AppError::Auth(e.to_string()))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");

        let mut verifier_raw = [0u8; 48];
        getrandom::fill(&mut verifier_raw).map_err(|e| AppError::Auth(e.to_string()))?;
        let code_verifier = BASE64URL_NOPAD.encode(&verifier_raw);
        let challenge_hash = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = BASE64URL_NOPAD.encode(&challenge_hash);

        let mut state_raw = [0u8; 24];
        getrandom::fill(&mut state_raw).map_err(|e| AppError::Auth(e.to_string()))?;
        let expected_state = BASE64URL_NOPAD.encode(&state_raw);

        let mut auth_url =
            Url::parse(GOOGLE_AUTH_URL).map_err(|e| AppError::Auth(e.to_string()))?;
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", client_id.trim())
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", DRIVE_FILE_SCOPE)
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &expected_state)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");

        open_browser(auth_url.as_str())?;

        // Wait up to 3 minutes for user to finish OAuth consent in browser.
        let code = tokio::time::timeout(
            Duration::from_secs(180),
            receive_loopback_code(&listener, &expected_state),
        )
        .await
        .map_err(|_| AppError::Auth("OAuth login timed out after 3 minutes".into()))??;

        let mut form = vec![
            ("code", code.as_str()),
            ("client_id", client_id.trim()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str()),
        ];
        let trimmed_secret = client_secret.trim();
        if !trimmed_secret.is_empty() {
            form.push(("client_secret", trimmed_secret));
        }

        let resp = self
            .http
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| AppError::Auth(format!("Token exchange request failed: {e}")))?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!("Token exchange failed: {txt}")));
        }

        let parsed: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Auth(format!("Invalid token JSON: {e}")))?;

        if let Some(rt) = parsed.refresh_token {
            save_refresh_token(&rt)?;
        } else {
            return Err(AppError::Auth(
                "Google did not return a refresh_token; please retry consent.".into(),
            ));
        }

        let ttl = parsed.expires_in.saturating_sub(60).max(30);
        *self.cached_access.lock().unwrap() = Some(CachedAccessToken {
            token: Zeroizing::new(parsed.access_token),
            expires_at: Instant::now() + Duration::from_secs(ttl),
        });

        Ok(())
    }

    pub async fn get_access_token(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Zeroizing<String>, AppError> {
        {
            let guard = self.cached_access.lock().unwrap();
            if let Some(ref cached) = *guard
                && Instant::now() < cached.expires_at
            {
                return Ok(cached.token.clone());
            }
        }

        let refresh = {
            let ov = self.override_refresh.lock().unwrap();
            if let Some(ref r) = *ov {
                r.clone()
            } else {
                load_refresh_token()?
            }
        };

        if client_id.trim().is_empty() {
            return Err(AppError::Auth(
                "Google OAuth Client ID is not configured".into(),
            ));
        }

        let mut form = vec![
            ("refresh_token", refresh.as_str()),
            ("client_id", client_id.trim()),
            ("grant_type", "refresh_token"),
        ];
        let trimmed_secret = client_secret.trim();
        if !trimmed_secret.is_empty() {
            form.push(("client_secret", trimmed_secret));
        }

        let resp = self
            .http
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| AppError::Auth(format!("Token refresh network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::BAD_REQUEST && txt.contains("invalid_grant") {
                return Err(AppError::Auth(
                    "Google Drive session expired or revoked. Please reconnect.".into(),
                ));
            }
            return Err(AppError::Auth(format!(
                "Token refresh failed ({status}): {txt}"
            )));
        }

        let parsed: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Auth(format!("Invalid refresh JSON: {e}")))?;

        if let Some(new_rt) = parsed.refresh_token {
            let _ = save_refresh_token(&new_rt);
        }

        let token = Zeroizing::new(parsed.access_token);
        let ttl = parsed.expires_in.saturating_sub(60).max(30);
        *self.cached_access.lock().unwrap() = Some(CachedAccessToken {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(ttl),
        });

        Ok(token)
    }

    pub async fn disconnect(&self) -> Result<(), AppError> {
        *self.cached_access.lock().unwrap() = None;
        *self.override_refresh.lock().unwrap() = None;
        if let Ok(rt) = load_refresh_token() {
            let _ = self
                .http
                .post(&self.revoke_url)
                .form(&[("token", rt.as_str())])
                .send()
                .await;
        }
        delete_refresh_token();
        Ok(())
    }
}

async fn receive_loopback_code(
    listener: &TcpListener,
    expected_state: &str,
) -> Result<String, AppError> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| AppError::Auth(format!("Loopback accept failed: {e}")))?;

        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| AppError::Auth(e.to_string()))?;
        let req = String::from_utf8_lossy(&buf[..n]);
        let first_line = req.lines().next().unwrap_or("");
        let path_part = first_line.split_whitespace().nth(1).unwrap_or("/");

        if path_part == "/favicon.ico" {
            let _ = stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await;
            continue;
        }

        let url = Url::parse(&format!("http://127.0.0.1{path_part}"))
            .map_err(|e| AppError::Auth(format!("Malformed callback URL: {e}")))?;
        let mut code: Option<String> = None;
        let mut state: Option<String> = None;
        let mut oauth_error: Option<String> = None;

        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                "error" => oauth_error = Some(v.into_owned()),
                _ => {}
            }
        }

        let html = if oauth_error.is_some()
            || state.as_deref() != Some(expected_state)
            || code.is_none()
        {
            "<html><body style='font-family:sans-serif;background:#111;color:#eee;padding:2rem'><h2>Authentication failed</h2><p>You can close this window and retry in Dynamic Markdown Dock.</p></body></html>"
        } else {
            "<html><body style='font-family:sans-serif;background:#111;color:#eee;padding:2rem'><h2>Connected to Google Drive</h2><p>You can close this browser tab and return to Dynamic Markdown Dock.</p></body></html>"
        };
        let http_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(http_resp.as_bytes()).await;

        if let Some(err) = oauth_error {
            return Err(AppError::Auth(format!("Google OAuth denied: {err}")));
        }
        if state.as_deref() != Some(expected_state) {
            return Err(AppError::Auth(
                "OAuth state mismatch (CSRF check failed)".into(),
            ));
        }
        if let Some(c) = code {
            return Ok(c);
        }
        return Err(AppError::Auth("Missing authorization code".into()));
    }
}

fn save_refresh_token(token: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_USER)
        .map_err(|e| AppError::Auth(format!("Keyring init failed: {e}")))?;
    entry
        .set_password(token)
        .map_err(|e| AppError::Auth(format!("Keyring save failed: {e}")))?;
    Ok(())
}

fn load_refresh_token() -> Result<Zeroizing<String>, AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_USER)
        .map_err(|e| AppError::Auth(format!("Keyring init failed: {e}")))?;
    let pw = entry
        .get_password()
        .map_err(|_| AppError::Auth("Google Drive is not connected".into()))?;
    Ok(Zeroizing::new(pw))
}

fn delete_refresh_token() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_USER) {
        let _ = entry.delete_credential();
    }
}
