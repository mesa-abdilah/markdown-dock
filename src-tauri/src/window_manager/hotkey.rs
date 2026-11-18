use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_specta::Event;

use crate::config::{DEFAULT_HOTKEY, FALLBACK_HOTKEY};
use crate::error::AppError;
use crate::events::{HotkeyFallbackNoticeEvent, HotkeyTriggeredEvent};

const TRIGGER_DEBOUNCE: Duration = Duration::from_millis(200);

#[cfg(target_os = "linux")]
const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
#[cfg(target_os = "linux")]
const GNOME_CUSTOM_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/dynamic-markdown-dock/";
#[cfg(target_os = "linux")]
const GNOME_CUSTOM_ITEM_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/dynamic-markdown-dock/";
#[cfg(target_os = "linux")]
const GNOME_TOGGLE_CMD: &str = "gdbus call --session --dest com.dynamicmarkdowndock.app.SingleInstance --object-path /com/dynamicmarkdowndock/app/SingleInstance --method org.SingleInstance.DBus.ExecuteCallback \"['dynamic-markdown-dock', '--toggle']\" \"\"";

#[derive(Default)]
pub struct HotkeyManager {
    active_shortcut: Mutex<Option<String>>,
    last_triggered: Mutex<Option<Instant>>,
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

        match Self::register_platform_shortcut(app, parsed, trimmed) {
            Ok(()) => {
                *self.active_shortcut.lock().unwrap() = Some(trimmed.to_string());
                Ok(trimmed.to_string())
            }
            Err(err) => {
                if let Some(ref old_str) = prev
                    && let Ok(old_parsed) = old_str.parse::<Shortcut>()
                {
                    let _ = Self::register_platform_shortcut(app, old_parsed, old_str);
                }
                Err(err)
            }
        }
    }

    pub fn active(&self) -> Option<String> {
        self.active_shortcut.lock().unwrap().clone()
    }

    /// Triggers a dock hotkey toggle with a short debounce so dual X11 + D-Bus/Wayland
    /// listeners never double-fire on a single physical key press.
    pub fn trigger(&self, app: &AppHandle) {
        {
            let mut last = self.last_triggered.lock().unwrap();
            if let Some(prev) = *last
                && prev.elapsed() < TRIGGER_DEBOUNCE
            {
                return;
            }
            *last = Some(Instant::now());
        }

        let shortcut_label = self.active().unwrap_or_default();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
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

    fn try_register(&self, app: &AppHandle, shortcut_str: &str) -> Result<(), AppError> {
        let parsed: Shortcut = shortcut_str
            .parse()
            .map_err(|e| AppError::Hotkey(format!("Invalid shortcut '{shortcut_str}': {e}")))?;
        Self::register_platform_shortcut(app, parsed, shortcut_str)?;
        *self.active_shortcut.lock().unwrap() = Some(shortcut_str.to_string());
        Ok(())
    }

    fn register_platform_shortcut(
        app: &AppHandle,
        shortcut: Shortcut,
        shortcut_str: &str,
    ) -> Result<(), AppError> {
        #[cfg(target_os = "linux")]
        let gnome_ok = match register_gnome_custom_shortcut(shortcut_str) {
            Ok(()) => true,
            Err(err) => {
                tracing::debug!("GNOME custom shortcut sync skipped: {err}");
                false
            }
        };

        let x11_res = Self::register_parsed(app, shortcut);

        #[cfg(target_os = "linux")]
        if gnome_ok {
            // When GNOME's compositor binds the shortcut via `media-keys`, XWayland may
            // report `AlreadyRegistered` on `XGrabKey`. Since GNOME will forward the shortcut
            // over D-Bus regardless of whether a Wayland or X11 window is focused, succeed.
            if let Err(ref err) = x11_res {
                tracing::debug!(
                    "X11 global_shortcut grab skipped because GNOME compositor owns '{}': {}",
                    shortcut_str,
                    err
                );
            }
            return Ok(());
        }

        let _ = shortcut_str;
        x11_res
    }

    fn register_parsed(app: &AppHandle, shortcut: Shortcut) -> Result<(), AppError> {
        app.global_shortcut()
            .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
                if event.state == ShortcutState::Pressed
                    && let Some(state) = app_handle.try_state::<crate::app_state::AppState>()
                {
                    state.hotkey.trigger(app_handle);
                }
            })
            .map_err(|e| AppError::Hotkey(format!("Shortcut registration failed: {e}")))
    }
}

#[cfg(target_os = "linux")]
fn to_gnome_accelerator(shortcut_str: &str) -> Option<String> {
    let mut mods = String::new();
    let mut key_part: Option<String> = None;

    for raw_token in shortcut_str
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match raw_token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => {
                if !mods.contains("<Control>") {
                    mods.push_str("<Control>");
                }
            }
            "alt" | "option" => {
                if !mods.contains("<Alt>") {
                    mods.push_str("<Alt>");
                }
            }
            "shift" => {
                if !mods.contains("<Shift>") {
                    mods.push_str("<Shift>");
                }
            }
            "super" | "meta" | "command" | "cmd" | "win" => {
                if !mods.contains("<Super>") {
                    mods.push_str("<Super>");
                }
            }
            other => {
                let mapped = match other {
                    "space" => "space".to_string(),
                    "enter" | "return" => "Return".to_string(),
                    "esc" | "escape" => "Escape".to_string(),
                    "tab" => "Tab".to_string(),
                    "backspace" => "BackSpace".to_string(),
                    "delete" => "Delete".to_string(),
                    "insert" => "Insert".to_string(),
                    "home" => "Home".to_string(),
                    "end" => "End".to_string(),
                    "pageup" => "Page_Up".to_string(),
                    "pagedown" => "Page_Down".to_string(),
                    "up" | "arrowup" => "Up".to_string(),
                    "down" | "arrowdown" => "Down".to_string(),
                    "left" | "arrowleft" => "Left".to_string(),
                    "right" | "arrowright" => "Right".to_string(),
                    "backquote" | "`" => "grave".to_string(),
                    "minus" | "-" => "minus".to_string(),
                    "equal" | "=" => "equal".to_string(),
                    "bracketleft" | "[" => "bracketleft".to_string(),
                    "bracketright" | "]" => "bracketright".to_string(),
                    "backslash" | "\\" => "backslash".to_string(),
                    "semicolon" | ";" => "semicolon".to_string(),
                    "quote" | "'" => "apostrophe".to_string(),
                    "comma" | "," => "comma".to_string(),
                    "period" | "." => "period".to_string(),
                    "slash" | "/" => "slash".to_string(),
                    s if s.starts_with("key") && s.len() == 4 => s[3..].to_string(),
                    s if s.starts_with("digit") && s.len() == 6 => s[5..].to_string(),
                    s if s.starts_with('f') && s[1..].parse::<u8>().is_ok() => {
                        s.to_ascii_uppercase()
                    }
                    s if s.len() == 1 => s.to_string(),
                    _ => return None,
                };
                key_part = Some(mapped);
            }
        }
    }

    let key = key_part?;
    Some(format!("{mods}{key}"))
}

#[cfg(target_os = "linux")]
fn run_gsettings(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("gsettings spawn error: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "linux")]
fn register_gnome_custom_shortcut(shortcut_str: &str) -> Result<(), String> {
    let accel = to_gnome_accelerator(shortcut_str)
        .ok_or_else(|| format!("Cannot convert '{shortcut_str}' to GNOME accelerator"))?;

    let current_list = run_gsettings(&["get", GNOME_MEDIA_KEYS_SCHEMA, "custom-keybindings"])?;
    let mut paths: Vec<String> = Vec::new();
    let mut rest = current_list.as_str();
    while let Some(start) = rest.find('\'') {
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('\'') {
            let item = &after_start[..end];
            if !item.is_empty() {
                paths.push(item.to_string());
            }
            rest = &after_start[end + 1..];
        } else {
            break;
        }
    }

    if !paths.iter().any(|p| p == GNOME_CUSTOM_PATH) {
        paths.push(GNOME_CUSTOM_PATH.to_string());
        let formatted = format!(
            "[{}]",
            paths
                .iter()
                .map(|p| format!("'{p}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        run_gsettings(&[
            "set",
            GNOME_MEDIA_KEYS_SCHEMA,
            "custom-keybindings",
            &formatted,
        ])?;
    }

    run_gsettings(&[
        "set",
        GNOME_CUSTOM_ITEM_SCHEMA,
        "name",
        "Dynamic Markdown Dock",
    ])?;
    run_gsettings(&["set", GNOME_CUSTOM_ITEM_SCHEMA, "command", GNOME_TOGGLE_CMD])?;
    run_gsettings(&["set", GNOME_CUSTOM_ITEM_SCHEMA, "binding", &accel])?;

    // On Ubuntu GNOME, `Alt+Space` is bound by default to `activate-window-menu`, which
    // intercepts `Alt+Space` before custom keybindings can fire. Free it when requested.
    if accel.eq_ignore_ascii_case("<Alt>space")
        && let Ok(wm_val) = run_gsettings(&[
            "get",
            "org.gnome.desktop.wm.keybindings",
            "activate-window-menu",
        ])
        && wm_val.to_ascii_lowercase().contains("<alt>space")
    {
        let _ = run_gsettings(&[
            "set",
            "org.gnome.desktop.wm.keybindings",
            "activate-window-menu",
            "[]",
        ]);
    }

    Ok(())
}
