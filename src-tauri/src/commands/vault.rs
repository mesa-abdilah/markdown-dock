use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::crypto::kdf::{PasswordStrength, assess_password_strength};
use crate::crypto::keystore::{VaultSetupPending, VaultStatus};
use crate::crypto::vault_format::AeadAlg;
use crate::error::AppError;

#[tauri::command]
#[specta::specta]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    state.vault.status()
}

#[tauri::command]
#[specta::specta]
pub fn vault_check_password_strength(password: String) -> PasswordStrength {
    assess_password_strength(&password)
}

#[tauri::command]
#[specta::specta]
pub async fn vault_begin_setup(
    state: State<'_, AppState>,
    password: String,
    aead: AeadAlg,
    remember_in_keyring: bool,
) -> Result<VaultSetupPending, AppError> {
    let vault = state.vault.clone();
    tokio::task::spawn_blocking(move || vault.begin_setup(&password, aead, remember_in_keyring))
        .await
        .map_err(|e| AppError::Crypto(format!("Setup task failed: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub fn vault_confirm_setup(
    app: AppHandle,
    state: State<'_, AppState>,
    suffix_input: String,
) -> Result<VaultStatus, AppError> {
    state.vault.confirm_setup(Some(&app), &suffix_input)
}

#[tauri::command]
#[specta::specta]
pub fn vault_cancel_setup(state: State<'_, AppState>) {
    state.vault.cancel_setup();
}

#[tauri::command]
#[specta::specta]
pub async fn vault_unlock_password(
    app: AppHandle,
    state: State<'_, AppState>,
    password: String,
    remember_in_keyring: bool,
) -> Result<VaultStatus, AppError> {
    let vault = state.vault.clone();
    tokio::task::spawn_blocking(move || {
        vault.unlock_with_password(Some(&app), &password, remember_in_keyring)
    })
    .await
    .map_err(|e| AppError::Crypto(format!("Unlock task failed: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn vault_unlock_recovery(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery_key: String,
    new_password: Option<String>,
    remember_in_keyring: bool,
) -> Result<VaultStatus, AppError> {
    let vault = state.vault.clone();
    tokio::task::spawn_blocking(move || {
        vault.unlock_with_recovery(
            Some(&app),
            &recovery_key,
            new_password.as_deref(),
            remember_in_keyring,
        )
    })
    .await
    .map_err(|e| AppError::Crypto(format!("Recovery unlock failed: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn vault_change_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), AppError> {
    let vault = state.vault.clone();
    tokio::task::spawn_blocking(move || vault.change_password(&current_password, &new_password))
        .await
        .map_err(|e| AppError::Crypto(format!("Password change failed: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub fn vault_lock(app: AppHandle, state: State<'_, AppState>, forget_keyring: bool) -> VaultStatus {
    state.vault.lock(Some(&app), forget_keyring)
}
