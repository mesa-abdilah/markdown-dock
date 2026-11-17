use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::app_state::AppState;
use crate::config::{AppSettings, SettingsPatch, save_settings};
use crate::error::AppError;
use crate::window_manager::DockController;
use crate::window_manager::monitor::{self, MonitorInfo};

#[tauri::command]
#[specta::specta]
pub fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
#[specta::specta]
pub fn settings_update(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings, AppError> {
    let (prev_policy, prev_hotkey, prev_autostart) = {
        let s = state.settings.lock().unwrap();
        (s.monitor_policy.clone(), s.hotkey.clone(), s.autostart)
    };

    let mut next = {
        let s = state.settings.lock().unwrap();
        let mut clone = s.clone();
        clone.apply_patch(patch);
        clone
    };

    if next.hotkey != prev_hotkey {
        let actual = state.hotkey.update_shortcut(&app, &next.hotkey)?;
        next.hotkey = actual;
    }

    if next.autostart != prev_autostart {
        let autostart = app.autolaunch();
        let res = if next.autostart {
            autostart.enable()
        } else {
            autostart.disable()
        };
        if let Err(err) = res {
            tracing::warn!("Autostart update failed: {err}");
        }
    }

    if next.monitor_policy != prev_policy {
        let _ = state
            .dock
            .update_monitor_policy(&app, next.monitor_policy.clone());
    }

    if (next.vertical_ratio - state.settings.lock().unwrap().vertical_ratio).abs() > f64::EPSILON {
        let _ = state.dock.set_vertical_ratio(&app, next.vertical_ratio);
    }

    save_settings(&state.settings_path, &next)?;
    state.sync.update_oauth_config(
        next.google_client_id.clone(),
        next.google_client_secret.clone(),
        next.sync_retention_days,
    );
    *state.settings.lock().unwrap() = next.clone();
    Ok(next)
}

#[tauri::command]
#[specta::specta]
pub fn list_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, AppError> {
    let win = DockController::dock_window(&app)?;
    monitor::list_monitors(&win)
}
