use std::sync::OnceLock;

use tauri::{AppHandle, Manager, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GWL_EXSTYLE, GWLP_WNDPROC, GetCursorPos, GetWindowLongPtrW, HWND_TOPMOST,
    SPI_SETWORKAREA, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WNDPROC, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::Interface;

use crate::error::AppError;
use crate::window_manager::geometry::PhysicalRect;

static ORIG_WNDPROC: OnceLock<isize> = OnceLock::new();
static HOOK_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn configure_dock_window(window: &WebviewWindow) -> Result<(), AppError> {
    let hwnd = window.hwnd()?;
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = ex_style | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | SWP_NOACTIVATE,
        );
    }
    Ok(())
}

/// Toggles `WS_EX_NOACTIVATE` so Peek mode does not steal focus from the user's active app.
pub fn set_no_activate(window: &WebviewWindow, no_activate: bool) -> Result<(), AppError> {
    let hwnd = window.hwnd()?;
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flag = WS_EX_NOACTIVATE.0 as isize;
        let updated = if no_activate {
            ex_style | flag
        } else {
            ex_style & !flag
        };
        if updated != ex_style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
        }
    }
    Ok(())
}

/// Atomically updates window position and size in a single Win32 `SetWindowPos` call
/// using physical monitor coordinates to eliminate 1-frame flicker or DPI mismatch.
pub fn set_bounds_atomic(window: &WebviewWindow, rect: PhysicalRect) -> Result<(), AppError> {
    let hwnd = window.hwnd()?;
    let w = i32::try_from(rect.width).unwrap_or(i32::MAX);
    let h = i32::try_from(rect.height).unwrap_or(i32::MAX);
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            w,
            h,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(|e| AppError::Window(format!("SetWindowPos failed: {e}")))?;
    }
    Ok(())
}

pub fn get_cursor_screen_pos(_window: &WebviewWindow) -> Option<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

pub fn install_display_change_hook(
    app: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), AppError> {
    let _ = HOOK_APP_HANDLE.set(app.clone());
    let hwnd = window.hwnd()?;
    unsafe {
        let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, dock_wndproc as *const () as isize);
        let _ = ORIG_WNDPROC.set(prev);
    }
    Ok(())
}

unsafe extern "system" fn dock_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let should_redock = match msg {
        WM_DISPLAYCHANGE => true,
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => true,
        _ => false,
    };

    if should_redock && let Some(app) = HOOK_APP_HANDLE.get() {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(state) = handle.try_state::<crate::app_state::AppState>() {
                let _ = state.dock.redock(&handle);
            }
        });
    }

    if let Some(&orig) = ORIG_WNDPROC.get()
        && orig != 0
    {
        let proc_fn: WNDPROC = unsafe { std::mem::transmute(orig) };
        return unsafe { CallWindowProcW(proc_fn, hwnd, msg, wparam, lparam) };
    }
    LRESULT(0)
}

pub fn set_webview_low_memory(window: &WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        ICoreWebView2_19,
    };

    let _ = window.with_webview(move |webview| unsafe {
        if let Ok(core) = webview.controller().CoreWebView2()
            && let Ok(core19) = core.cast::<ICoreWebView2_19>()
        {
            let level = if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = core19.SetMemoryUsageTargetLevel(level);
        }
    });
}
