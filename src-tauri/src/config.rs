use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::error::AppError;

pub const DEFAULT_HOTKEY: &str = "Alt+Space";
pub const FALLBACK_HOTKEY: &str = "Ctrl+Alt+Space";

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
#[serde(tag = "kind", content = "name")]
pub enum MonitorPolicy {
    #[default]
    Primary,
    Specific(String),
    FollowCursor,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct AppSettings {
    pub monitor_policy: MonitorPolicy,
    /// Normalized vertical position along the right edge in [0.0, 1.0].
    pub vertical_ratio: f64,
    pub hotkey: String,
    pub pinned_by_default: bool,
    pub reduced_motion_override: Option<bool>,
    pub autostart: bool,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub sync_retention_days: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            monitor_policy: MonitorPolicy::Primary,
            vertical_ratio: 0.5,
            hotkey: DEFAULT_HOTKEY.to_string(),
            pinned_by_default: false,
            reduced_motion_override: None,
            autostart: false,
            google_client_id: option_env!("DMD_GOOGLE_CLIENT_ID")
                .unwrap_or("")
                .to_string(),
            google_client_secret: option_env!("DMD_GOOGLE_CLIENT_SECRET")
                .unwrap_or("")
                .to_string(),
            sync_retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Type, Default)]
pub struct SettingsPatch {
    pub monitor_policy: Option<MonitorPolicy>,
    pub vertical_ratio: Option<f64>,
    pub hotkey: Option<String>,
    pub pinned_by_default: Option<bool>,
    pub reduced_motion_override: Option<Option<bool>>,
    pub autostart: Option<bool>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub sync_retention_days: Option<u32>,
}

impl AppSettings {
    pub fn apply_patch(&mut self, patch: SettingsPatch) {
        if let Some(policy) = patch.monitor_policy {
            self.monitor_policy = policy;
        }
        if let Some(ratio) = patch.vertical_ratio {
            self.vertical_ratio = ratio.clamp(0.0, 1.0);
        }
        if let Some(hotkey) = patch.hotkey {
            let trimmed = hotkey.trim();
            if !trimmed.is_empty() {
                self.hotkey = trimmed.to_string();
            }
        }
        if let Some(pinned) = patch.pinned_by_default {
            self.pinned_by_default = pinned;
        }
        if let Some(reduced) = patch.reduced_motion_override {
            self.reduced_motion_override = reduced;
        }
        if let Some(autostart) = patch.autostart {
            self.autostart = autostart;
        }
        if let Some(cid) = patch.google_client_id {
            self.google_client_id = cid.trim().to_string();
        }
        if let Some(csec) = patch.google_client_secret {
            self.google_client_secret = csec.trim().to_string();
        }
        if let Some(days) = patch.sync_retention_days {
            self.sync_retention_days = days.clamp(1, 3650);
        }
    }
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(e.to_string()))?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings(path: &Path) -> AppSettings {
    match std::fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<AppSettings>(&raw) {
            Ok(mut parsed) => {
                parsed.vertical_ratio = parsed.vertical_ratio.clamp(0.0, 1.0);
                if parsed.google_client_id.trim().is_empty()
                    && let Some(env_id) = option_env!("DMD_GOOGLE_CLIENT_ID")
                {
                    parsed.google_client_id = env_id.to_string();
                }
                if parsed.google_client_secret.trim().is_empty()
                    && let Some(env_secret) = option_env!("DMD_GOOGLE_CLIENT_SECRET")
                {
                    parsed.google_client_secret = env_secret.to_string();
                }
                parsed
            }
            Err(err) => {
                tracing::warn!("Failed to parse settings.json, using defaults: {err}");
                AppSettings::default()
            }
        },
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Config(format!("serialize failed: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
