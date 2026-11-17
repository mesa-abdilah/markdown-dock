use tauri::{AppHandle, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

use crate::error::AppError;
use crate::window_manager::geometry::PhysicalRect;

/// Configures a macOS floating utility dock window across all Spaces without
/// claiming Dock focus when collapsed.
pub fn configure_dock_window(window: &WebviewWindow) -> Result<(), AppError> {
    if let Err(err) = window.set_always_on_top(true) {
        tracing::warn!("macOS set_always_on_top failed: {err}");
    }
    if let Err(err) = window.set_visible_on_all_workspaces(true) {
        tracing::warn!("macOS set_visible_on_all_workspaces failed: {err}");
    }
    if let Err(err) = window.set_skip_taskbar(true) {
        tracing::debug!("macOS set_skip_taskbar unsupported: {err}");
    }
    if let Err(err) = window.set_focusable(false) {
        tracing::debug!("macOS initial set_focusable(false) unsupported: {err}");
    }
    Ok(())
}

pub fn set_no_activate(window: &WebviewWindow, no_activate: bool) -> Result<(), AppError> {
    if let Err(err) = window.set_focusable(!no_activate) {
        tracing::debug!("macOS set_focusable({}) failed: {err}", !no_activate);
    }
    Ok(())
}

pub fn set_bounds_atomic(window: &WebviewWindow, rect: PhysicalRect) -> Result<(), AppError> {
    window.set_position(Position::Physical(PhysicalPosition::new(rect.x, rect.y)))?;
    window.set_size(Size::Physical(PhysicalSize::new(rect.width, rect.height)))?;
    Ok(())
}

pub fn get_cursor_screen_pos(window: &WebviewWindow) -> Option<(i32, i32)> {
    window
        .cursor_position()
        .ok()
        .map(|pos| (pos.x.round() as i32, pos.y.round() as i32))
}

/// Switches macOS app activation policy to `Accessory` so no Dock icon appears.
pub fn install_display_change_hook(
    app: &AppHandle,
    _window: &WebviewWindow,
) -> Result<(), AppError> {
    if let Err(err) = app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
        tracing::warn!("macOS set_activation_policy(Accessory) failed: {err}");
    }
    Ok(())
}

pub fn set_webview_low_memory(_window: &WebviewWindow, _low: bool) {}
