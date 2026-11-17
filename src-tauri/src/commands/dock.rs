use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::config::save_settings;
use crate::error::AppError;
use crate::window_manager::DockStateSnapshot;
use crate::window_manager::geometry::{DockMode, DockPlan};

#[tauri::command]
#[specta::specta]
pub fn dock_get_state(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DockStateSnapshot, AppError> {
    state.dock.get_snapshot(&app)
}

#[tauri::command]
#[specta::specta]
pub fn dock_plan_transition(
    app: AppHandle,
    state: State<'_, AppState>,
    target: DockMode,
) -> Result<DockPlan, AppError> {
    state.dock.plan_transition(&app, target)
}

#[tauri::command]
#[specta::specta]
pub fn dock_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_seq: u32,
    focus_window: bool,
) -> Result<DockStateSnapshot, AppError> {
    state.dock.apply_plan(&app, plan_seq, focus_window)
}

#[tauri::command]
#[specta::specta]
pub fn dock_set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    pinned: bool,
) -> Result<DockStateSnapshot, AppError> {
    state.dock.set_pinned(&app, pinned)
}

#[tauri::command]
#[specta::specta]
pub fn dock_set_vertical_ratio(
    app: AppHandle,
    state: State<'_, AppState>,
    ratio: f64,
    persist: bool,
) -> Result<DockPlan, AppError> {
    let plan = state.dock.set_vertical_ratio(&app, ratio)?;
    if persist {
        let mut settings = state.settings.lock().unwrap();
        settings.vertical_ratio = ratio.clamp(0.0, 1.0);
        save_settings(&state.settings_path, &settings)?;
    }
    Ok(plan)
}

#[tauri::command]
#[specta::specta]
pub fn dock_nudge_vertical_by_screen_y(
    app: AppHandle,
    state: State<'_, AppState>,
    center_phys_y: f64,
    persist: bool,
) -> Result<DockPlan, AppError> {
    let plan = state.dock.nudge_vertical_by_screen_y(&app, center_phys_y)?;
    if persist {
        let snap = state.dock.get_snapshot(&app)?;
        let mut settings = state.settings.lock().unwrap();
        settings.vertical_ratio = snap.vertical_ratio;
        save_settings(&state.settings_path, &settings)?;
    }
    Ok(plan)
}

#[tauri::command]
#[specta::specta]
pub fn dock_redock(app: AppHandle, state: State<'_, AppState>) -> Result<DockPlan, AppError> {
    state.dock.redock(&app)
}
