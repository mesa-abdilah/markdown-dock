use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use subtle::ConstantTimeEq;
use tauri::AppHandle;
use tauri_specta::Event;
use zeroize::Zeroizing;

use crate::crypto::aead::{
    create_keyfile, rewrap_keyfile_password, unwrap_keyfile_with_password,
    unwrap_keyfile_with_recovery,
};
use crate::crypto::kdf::assess_password_strength;
use crate::crypto::keys::{SecretKey32, VaultKeys, format_recovery_key};
use crate::crypto::vault_format::AeadAlg;
use crate::error::AppError;
use crate::events::{VaultLockState, VaultStateChangedEvent};

const KEYRING_SERVICE: &str = "com.dynamicmarkdowndock.app";
const KEYRING_USER: &str = "vault-key-v1";
const DEFAULT_IDLE_LOCK_MINUTES: u64 = 15;

#[derive(Debug, Clone, Serialize, Type)]
pub struct VaultStatus {
    pub state: VaultLockState,
    pub remember_in_keyring: bool,
    pub idle_lock_minutes: u32,
    pub key_epoch: Option<u32>,
    pub aead: Option<AeadAlg>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct VaultSetupPending {
    pub recovery_key: String,
    pub recovery_suffix: String,
}

struct UnlockedSession {
    keys: VaultKeys,
    vault_id: [u8; 16],
    key_epoch: u32,
    aead: AeadAlg,
    last_active: Instant,
}

struct PendingSetup {
    keyfile_bytes: Vec<u8>,
    keys: VaultKeys,
    vault_id: [u8; 16],
    key_epoch: u32,
    aead: AeadAlg,
    expected_suffix: String,
    expected_full: Zeroizing<String>,
    remember_in_keyring: bool,
}

pub struct VaultManager {
    keyfile_path: PathBuf,
    idle_lock_minutes: Mutex<u64>,
    remember_in_keyring: Mutex<bool>,
    session: Mutex<Option<UnlockedSession>>,
    pending_setup: Mutex<Option<PendingSetup>>,
}

impl VaultManager {
    pub fn init(keyfile_path: PathBuf) -> Self {
        let mut remember = false;
        let mut initial_session: Option<UnlockedSession> = None;

        if keyfile_path.exists()
            && let Ok(raw_kf) = std::fs::read(&keyfile_path)
            && let Ok((vault_id, header)) = crate::crypto::aead::read_keyfile_vault_id(&raw_kf)
            && let Ok(vk_bytes) = load_vault_key_from_keyring()
            && let Ok(keys) = VaultKeys::derive_from_vault_key(vk_bytes)
        {
            remember = true;
            initial_session = Some(UnlockedSession {
                keys,
                vault_id,
                key_epoch: header.key_epoch,
                aead: header.aead,
                last_active: Instant::now(),
            });
        }

        Self {
            keyfile_path,
            idle_lock_minutes: Mutex::new(DEFAULT_IDLE_LOCK_MINUTES),
            remember_in_keyring: Mutex::new(remember),
            session: Mutex::new(initial_session),
            pending_setup: Mutex::new(None),
        }
    }

    pub fn keyfile_path(&self) -> &Path {
        &self.keyfile_path
    }

    pub fn status(&self) -> VaultStatus {
        self.check_idle_timeout();
        let remember = *self.remember_in_keyring.lock().unwrap();
        let idle_mins = *self.idle_lock_minutes.lock().unwrap() as u32;
        let sess = self.session.lock().unwrap();

        if let Some(ref s) = *sess {
            VaultStatus {
                state: VaultLockState::Unlocked,
                remember_in_keyring: remember,
                idle_lock_minutes: idle_mins,
                key_epoch: Some(s.key_epoch),
                aead: Some(s.aead),
            }
        } else if self.keyfile_path.exists() {
            VaultStatus {
                state: VaultLockState::Locked,
                remember_in_keyring: remember,
                idle_lock_minutes: idle_mins,
                key_epoch: None,
                aead: None,
            }
        } else {
            VaultStatus {
                state: VaultLockState::Uninitialized,
                remember_in_keyring: false,
                idle_lock_minutes: idle_mins,
                key_epoch: None,
                aead: None,
            }
        }
    }

    /// Step 1 of mandatory Recovery Key setup: validates strength >= 3, creates keyfile in memory,
    /// and returns the 52-char Recovery Key. Vault is NOT written/activated until `confirm_setup` runs.
    pub fn begin_setup(
        &self,
        password: &str,
        aead: AeadAlg,
        remember_in_keyring: bool,
    ) -> Result<VaultSetupPending, AppError> {
        let strength = assess_password_strength(password);
        if strength.score < 3 {
            return Err(AppError::Crypto(strength.warning.unwrap_or_else(|| {
                "Master password is too weak (minimum score 3/4 required)".into()
            })));
        }

        let (keyfile_bytes, keys, recovery_key, info) = create_keyfile(password, aead)?;
        let formatted = format_recovery_key(&recovery_key);
        let full_clean =
            Zeroizing::new(formatted.chars().filter(|c| *c != '-').collect::<String>());
        let suffix: String = full_clean
            .chars()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        *self.pending_setup.lock().unwrap() = Some(PendingSetup {
            keyfile_bytes,
            keys,
            vault_id: info.vault_id,
            key_epoch: info.key_epoch,
            aead: info.aead,
            expected_suffix: suffix.clone(),
            expected_full: full_clean,
            remember_in_keyring,
        });

        Ok(VaultSetupPending {
            recovery_key: formatted.to_string(),
            recovery_suffix: suffix,
        })
    }

    /// Step 2 of mandatory Recovery Key setup: verifies the user recorded either the last 6 chars
    /// or the full 52-char Recovery Key, writes `keyfile.vault` atomically, and unlocks the session.
    pub fn confirm_setup(
        &self,
        app: Option<&AppHandle>,
        suffix_input: &str,
    ) -> Result<VaultStatus, AppError> {
        let pending = self
            .pending_setup
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| AppError::Crypto("No pending vault setup found".into()))?;

        let cleaned = Zeroizing::new(
            suffix_input
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '-')
                .map(|c| c.to_ascii_uppercase())
                .collect::<String>(),
        );

        let matches_suffix = cleaned.len() == pending.expected_suffix.len()
            && bool::from(cleaned.as_bytes().ct_eq(pending.expected_suffix.as_bytes()));
        let matches_full = cleaned.len() == pending.expected_full.len()
            && bool::from(cleaned.as_bytes().ct_eq(pending.expected_full.as_bytes()));
        if !matches_suffix && !matches_full {
            // Put pending setup back so user can retry without regenerating keys
            let got_len = cleaned.len();
            let suffix_len = pending.expected_suffix.len();
            let full_len = pending.expected_full.len();
            *self.pending_setup.lock().unwrap() = Some(pending);
            return Err(AppError::Crypto(format!(
                "Recovery Key verification did not match (received {got_len} chars without '-'). Type the last {suffix_len} characters or paste the full {full_len}-character Recovery Key.",
            )));
        }

        if let Some(parent) = self.keyfile_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.keyfile_path.with_extension("vault.tmp");
        std::fs::write(&tmp, &pending.keyfile_bytes)?;
        std::fs::rename(&tmp, &self.keyfile_path)?;

        if pending.remember_in_keyring {
            let _ = save_vault_key_to_keyring(&pending.keys.vault_key);
        } else {
            let _ = delete_vault_key_from_keyring();
        }

        *self.remember_in_keyring.lock().unwrap() = pending.remember_in_keyring;
        *self.session.lock().unwrap() = Some(UnlockedSession {
            keys: pending.keys,
            vault_id: pending.vault_id,
            key_epoch: pending.key_epoch,
            aead: pending.aead,
            last_active: Instant::now(),
        });

        let st = self.status();
        emit_vault_state(app, &st);
        Ok(st)
    }

    pub fn cancel_setup(&self) {
        *self.pending_setup.lock().unwrap() = None;
    }

    pub fn unlock_with_password(
        &self,
        app: Option<&AppHandle>,
        password: &str,
        remember_in_keyring: bool,
    ) -> Result<VaultStatus, AppError> {
        let kf = std::fs::read(&self.keyfile_path)
            .map_err(|_| AppError::NotFound("Vault keyfile not initialized".into()))?;
        let (keys, info) = unwrap_keyfile_with_password(&kf, password)?;

        if remember_in_keyring {
            let _ = save_vault_key_to_keyring(&keys.vault_key);
        } else {
            let _ = delete_vault_key_from_keyring();
        }
        *self.remember_in_keyring.lock().unwrap() = remember_in_keyring;
        *self.session.lock().unwrap() = Some(UnlockedSession {
            keys,
            vault_id: info.vault_id,
            key_epoch: info.key_epoch,
            aead: info.aead,
            last_active: Instant::now(),
        });

        let st = self.status();
        emit_vault_state(app, &st);
        Ok(st)
    }

    pub fn unlock_with_recovery(
        &self,
        app: Option<&AppHandle>,
        recovery_key: &str,
        new_password: Option<&str>,
        remember_in_keyring: bool,
    ) -> Result<VaultStatus, AppError> {
        let kf = std::fs::read(&self.keyfile_path)
            .map_err(|_| AppError::NotFound("Vault keyfile not initialized".into()))?;
        let (keys, info) = unwrap_keyfile_with_recovery(&kf, recovery_key)?;

        if let Some(npw) = new_password {
            let trimmed = npw.trim();
            if !trimmed.is_empty() {
                let strength = assess_password_strength(trimmed);
                if strength.score < 3 {
                    return Err(AppError::Crypto(
                        strength
                            .warning
                            .unwrap_or_else(|| "New password is too weak".into()),
                    ));
                }
                let updated = rewrap_keyfile_password(&kf, &keys, trimmed)?;
                let tmp = self.keyfile_path.with_extension("vault.tmp");
                std::fs::write(&tmp, &updated)?;
                std::fs::rename(&tmp, &self.keyfile_path)?;
            }
        }

        if remember_in_keyring {
            let _ = save_vault_key_to_keyring(&keys.vault_key);
        } else {
            let _ = delete_vault_key_from_keyring();
        }
        *self.remember_in_keyring.lock().unwrap() = remember_in_keyring;
        *self.session.lock().unwrap() = Some(UnlockedSession {
            keys,
            vault_id: info.vault_id,
            key_epoch: info.key_epoch,
            aead: info.aead,
            last_active: Instant::now(),
        });

        let st = self.status();
        emit_vault_state(app, &st);
        Ok(st)
    }

    pub fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        let strength = assess_password_strength(new_password);
        if strength.score < 3 {
            return Err(AppError::Crypto(strength.warning.unwrap_or_else(|| {
                "New password is too weak (minimum score 3/4)".into()
            })));
        }
        let kf = std::fs::read(&self.keyfile_path)
            .map_err(|_| AppError::NotFound("Vault keyfile not initialized".into()))?;
        let (keys, _) = unwrap_keyfile_with_password(&kf, current_password)?;
        let updated = rewrap_keyfile_password(&kf, &keys, new_password)?;
        let tmp = self.keyfile_path.with_extension("vault.tmp");
        std::fs::write(&tmp, &updated)?;
        std::fs::rename(&tmp, &self.keyfile_path)?;
        Ok(())
    }

    pub fn lock(&self, app: Option<&AppHandle>, forget_keyring: bool) -> VaultStatus {
        *self.session.lock().unwrap() = None;
        if forget_keyring {
            let _ = delete_vault_key_from_keyring();
            *self.remember_in_keyring.lock().unwrap() = false;
        }
        let st = self.status();
        emit_vault_state(app, &st);
        st
    }

    pub fn with_unlocked<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&VaultKeys, [u8; 16], u32, AeadAlg) -> Result<T, AppError>,
    {
        self.check_idle_timeout();
        let mut guard = self.session.lock().unwrap();
        let sess = guard.as_mut().ok_or(AppError::VaultLocked)?;
        sess.last_active = Instant::now();
        f(&sess.keys, sess.vault_id, sess.key_epoch, sess.aead)
    }

    pub fn check_idle_timeout(&self) -> bool {
        let mins = *self.idle_lock_minutes.lock().unwrap();
        if mins == 0 {
            return false;
        }
        let mut guard = self.session.lock().unwrap();
        if let Some(ref s) = *guard
            && s.last_active.elapsed() > Duration::from_secs(mins * 60)
        {
            *guard = None;
            return true;
        }
        false
    }
}

fn emit_vault_state(app: Option<&AppHandle>, st: &VaultStatus) {
    if let Some(handle) = app {
        let _ = VaultStateChangedEvent {
            state: st.state,
            remember_in_keyring: st.remember_in_keyring,
        }
        .emit(handle);
    }
}

fn save_vault_key_to_keyring(vk: &SecretKey32) -> Result<(), AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Crypto(format!("Keyring init failed: {e}")))?;
    entry
        .set_secret(vk.expose())
        .map_err(|e| AppError::Crypto(format!("Keyring write failed: {e}")))?;
    Ok(())
}

fn load_vault_key_from_keyring() -> Result<SecretKey32, AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Crypto(format!("Keyring init failed: {e}")))?;
    let secret = Zeroizing::new(
        entry
            .get_secret()
            .map_err(|e| AppError::Crypto(format!("Keyring read failed: {e}")))?,
    );
    if secret.len() != 32 {
        return Err(AppError::Crypto("Invalid key length in keyring".into()));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&secret);
    Ok(SecretKey32::from_bytes(raw))
}

fn delete_vault_key_from_keyring() -> Result<(), AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Crypto(format!("Keyring init failed: {e}")))?;
    let _ = entry.delete_credential();
    Ok(())
}
