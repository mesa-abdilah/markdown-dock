use tauri::{AppHandle, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

use crate::error::AppError;
use crate::window_manager::geometry::PhysicalRect;

/// Configures an X11 dock window to stay above other windows, omit taskbar entries,
/// span all workspaces, and avoid stealing focus while collapsed.
///
/// Wayland is intentionally excluded in v1 because global screen positioning and
/// cursor probing are compositor-restricted on Wayland.
pub fn configure_dock_window(window: &WebviewWindow) -> Result<(), AppError> {
    if let Err(err) = window.set_always_on_top(true) {
        tracing::warn!("X11 set_always_on_top failed: {err}");
    }
    if let Err(err) = window.set_skip_taskbar(true) {
        tracing::warn!("X11 set_skip_taskbar failed: {err}");
    }
    if let Err(err) = window.set_visible_on_all_workspaces(true) {
        tracing::warn!("X11 set_visible_on_all_workspaces failed: {err}");
    }
    if let Err(err) = window.set_focusable(false) {
        tracing::debug!("X11 initial set_focusable(false) unsupported: {err}");
    }
    Ok(())
}

/// Toggles X11 focusability so Peek/Collapsed do not steal focus from the user's app.
pub fn set_no_activate(window: &WebviewWindow, no_activate: bool) -> Result<(), AppError> {
    if let Err(err) = window.set_focusable(!no_activate) {
        tracing::debug!("X11 set_focusable({}) failed: {err}", !no_activate);
    }
    Ok(())
}

/// Updates X11 window position and size in physical coordinates.
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

pub fn install_display_change_hook(
    _app: &AppHandle,
    _window: &WebviewWindow,
) -> Result<(), AppError> {
    Ok(())
}

pub fn set_webview_low_memory(_window: &WebviewWindow, _low: bool) {}
