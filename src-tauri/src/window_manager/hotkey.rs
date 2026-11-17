use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_specta::Event;

use crate::config::{DEFAULT_HOTKEY, FALLBACK_HOTKEY};
use crate::error::AppError;
use crate::events::{HotkeyFallbackNoticeEvent, HotkeyTriggeredEvent};

#[derive(Default)]
pub struct HotkeyManager {
    active_shortcut: Mutex<Option<String>>,
}

impl HotkeyManager {
    pub fn register_initial(&self, app: &AppHandle, requested: &str) -> Result<String, AppError> {
        let requested_trimmed = if requested.trim().is_empty() {
            DEFAULT_HOTKEY
        } else {
            requested.trim()
        };

        match self.try_register(app, requested_trimmed) {
            Ok(()) => Ok(requested_trimmed.to_string()),
            Err(primary_err) => {
                tracing::warn!(
                    "Failed to register hotkey '{}': {}. Trying fallback '{}'",
                    requested_trimmed,
                    primary_err,
                    FALLBACK_HOTKEY
                );
                if requested_trimmed != FALLBACK_HOTKEY
                    && self.try_register(app, FALLBACK_HOTKEY).is_ok()
                {
                    let _ = HotkeyFallbackNoticeEvent {
                        requested: requested_trimmed.to_string(),
                        active: FALLBACK_HOTKEY.to_string(),
                        reason: primary_err.to_string(),
                    }
                    .emit(app);
                    return Ok(FALLBACK_HOTKEY.to_string());
                }
                Err(primary_err)
            }
        }
    }

    pub fn update_shortcut(&self, app: &AppHandle, new_shortcut: &str) -> Result<String, AppError> {
        let trimmed = new_shortcut.trim();
        let parsed: Shortcut = trimmed
            .parse()
            .map_err(|e| AppError::Hotkey(format!("Invalid shortcut '{trimmed}': {e}")))?;

        let prev = self.active_shortcut.lock().unwrap().clone();
        if prev.as_deref() == Some(trimmed) {
            return Ok(trimmed.to_string());
        }

        // Unregister previous before registering new (restore previous if new fails).
        if let Some(ref old_str) = prev
            && let Ok(old_parsed) = old_str.parse::<Shortcut>()
        {
            let _ = app.global_shortcut().unregister(old_parsed);
        }

        match Self::register_parsed(app, parsed, trimmed.to_string()) {
            Ok(()) => {
                *self.active_shortcut.lock().unwrap() = Some(trimmed.to_string());
                Ok(trimmed.to_string())
            }
            Err(err) => {
                if let Some(ref old_str) = prev
                    && let Ok(old_parsed) = old_str.parse::<Shortcut>()
                {
                    let _ = Self::register_parsed(app, old_parsed, old_str.clone());
                }
                Err(err)
            }
        }
    }

    pub fn active(&self) -> Option<String> {
        self.active_shortcut.lock().unwrap().clone()
    }

    fn try_register(&self, app: &AppHandle, shortcut_str: &str) -> Result<(), AppError> {
        let parsed: Shortcut = shortcut_str
            .parse()
            .map_err(|e| AppError::Hotkey(format!("Invalid shortcut '{shortcut_str}': {e}")))?;
        Self::register_parsed(app, parsed, shortcut_str.to_string())?;
        *self.active_shortcut.lock().unwrap() = Some(shortcut_str.to_string());
        Ok(())
    }

    fn register_parsed(app: &AppHandle, shortcut: Shortcut, label: String) -> Result<(), AppError> {
        app.global_shortcut()
            .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let shortcut_label = label.clone();
                    let handle = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        if let Some(state) = handle.try_state::<crate::app_state::AppState>() {
                            match state.dock.toggle_from_hotkey(&handle) {
                                Ok(target_mode) => {
                                    let _ = HotkeyTriggeredEvent {
                                        shortcut: shortcut_label,
                                        target_mode,
                                    }
                                    .emit(&handle);
                                }
                                Err(err) => {
                                    tracing::error!("Hotkey toggle failed: {err}");
                                }
                            }
                        }
                    });
                }
            })
            .map_err(|e| AppError::Hotkey(format!("Shortcut registration failed: {e}")))
    }
}
