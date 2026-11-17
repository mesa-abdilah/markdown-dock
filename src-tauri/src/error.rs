use serde::Serialize;
use specta::Type;

#[derive(Debug, thiserror::Error, Serialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Window operation failed: {0}")]
    Window(String),

    #[error("Monitor not found: {0}")]
    Monitor(String),

    #[error("Stale transition plan sequence {provided} (current: {current})")]
    StalePlan { provided: u32, current: u32 },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Hotkey error: {0}")]
    Hotkey(String),

    #[error("Database error: {0}")]
    Db(String),

    #[error("Watcher error: {0}")]
    Watcher(String),

    #[error("Document or root not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    Security(String),

    #[error("Cryptography error: {0}")]
    Crypto(String),

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Google Drive sync error: {0}")]
    Sync(String),

    #[error("OAuth authentication error: {0}")]
    Auth(String),

    #[error("IO error: {0}")]
    Io(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        Self::Window(err.to_string())
    }
}
