use std::sync::Mutex;
use std::sync::mpsc::sync_channel;

use gtk::prelude::*;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::error::AppError;
use crate::window_manager::geometry::PhysicalRect;

struct LinuxDockTarget {
    rect: Option<PhysicalRect>,
    no_activate: bool,
    retries: u8,
}

static DOCK_TARGET: Mutex<LinuxDockTarget> = Mutex::new(LinuxDockTarget {
    rect: None,
    no_activate: true,
    retries: 0,
});

fn with_gtk_window<F>(window: &WebviewWindow, f: F) -> Result<(), AppError>
where
    F: FnOnce(&gtk::ApplicationWindow) + Send + 'static,
{
    if gtk::glib::MainContext::default().is_owner() {
        let gtk_win = window.gtk_window()?;
        f(&gtk_win);
        return Ok(());
    }

    let win = window.clone();
    let (tx, rx) = sync_channel::<Result<(), AppError>>(1);
    window
        .run_on_main_thread(move || {
            let res = match win.gtk_window() {
                Ok(gtk_win) => {
                    f(&gtk_win);
                    Ok(())
                }
                Err(err) => Err(AppError::Window(err.to_string())),
            };
            let _ = tx.send(res);
        })
        .map_err(|e| AppError::Window(e.to_string()))?;

    rx.recv()
        .map_err(|e| AppError::Window(format!("GTK main thread channel closed: {e}")))?
}

fn logical_bounds(gtk_win: &gtk::ApplicationWindow, rect: PhysicalRect) -> (i32, i32, i32, i32) {
    let scale = f64::from(gtk_win.scale_factor().max(1));
    let x_l = (f64::from(rect.x) / scale).round() as i32;
    let y_l = (f64::from(rect.y) / scale).round() as i32;
    let w_l = ((f64::from(rect.width) / scale).round() as i32).max(1);
    let h_l = ((f64::from(rect.height) / scale).round() as i32).max(1);
    (x_l, y_l, w_l, h_l)
}

fn apply_gtk_window_flags(gtk_win: &gtk::ApplicationWindow, no_activate: bool) {
    gtk_win.set_decorated(false);
    gtk_win.set_keep_above(true);
    gtk_win.set_skip_taskbar_hint(true);
    gtk_win.set_skip_pager_hint(true);
    gtk_win.stick();
    gtk_win.set_gravity(gtk::gdk::Gravity::NorthWest);
    gtk_win.set_accept_focus(!no_activate);
    gtk_win.set_focus_on_map(!no_activate);

    if !gtk_win.is_visible() {
        gtk_win.set_type_hint(gtk::gdk::WindowTypeHint::Utility);
    }

    if let Some(gdk_win) = gtk_win.window() {
        gdk_win.set_type_hint(gtk::gdk::WindowTypeHint::Utility);
        gdk_win.set_keep_above(true);
        gdk_win.set_skip_taskbar_hint(true);
        gdk_win.set_skip_pager_hint(true);
        gdk_win.stick();
        gdk_win.set_accept_focus(!no_activate);
        gdk_win.set_focus_on_map(!no_activate);
    }
}

fn apply_gtk_bounds(gtk_win: &gtk::ApplicationWindow, rect: PhysicalRect) {
    let (x_l, y_l, w_l, h_l) = logical_bounds(gtk_win, rect);

    // Update X11 WM_NORMAL_HINTS so that even while `resizable` is false (which prevents
    // tao from hijacking mouse clicks near the 5px window border for resize-drags),
    // GTK3 and GNOME/Mutter permit resizing between Collapsed (28x96) and Expanded panels.
    let geom = gtk::gdk::Geometry::new(
        w_l,
        h_l,
        w_l,
        h_l,
        w_l,
        h_l,
        0,
        0,
        0.0,
        0.0,
        gtk::gdk::Gravity::NorthWest,
    );
    gtk_win.set_geometry_hints(
        None::<&gtk::Widget>,
        Some(&geom),
        gtk::gdk::WindowHints::MIN_SIZE
            | gtk::gdk::WindowHints::MAX_SIZE
            | gtk::gdk::WindowHints::BASE_SIZE
            | gtk::gdk::WindowHints::WIN_GRAVITY,
    );

    gtk_win.set_size_request(w_l, h_l);
    if let Some(child) = gtk_win.child() {
        child.set_size_request(w_l, h_l);
    }
    gtk_win.set_default_size(w_l, h_l);
    gtk_win.resize(w_l, h_l);
    gtk_win.move_(x_l, y_l);

    if let Some(gdk_win) = gtk_win.window() {
        gdk_win.move_resize(x_l, y_l, w_l, h_l);
    }
}

/// Configures an X11/XWayland dock window to stay above other windows, omit taskbar entries,
/// span all workspaces, and avoid stealing focus while collapsed.
pub fn configure_dock_window(window: &WebviewWindow) -> Result<(), AppError> {
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_visible_on_all_workspaces(true);
    let _ = window.set_focusable(false);

    with_gtk_window(window, |gtk_win| {
        let no_activate = DOCK_TARGET
            .lock()
            .map(|t| t.no_activate)
            .unwrap_or(true);
        apply_gtk_window_flags(gtk_win, no_activate);
    })
}

/// Reveals the dock window only after window type hints, flags, and edge bounds are set,
/// preventing GNOME Mutter from attaching `mutter-x11-frames` at startup.
pub fn show_dock_window(window: &WebviewWindow) -> Result<(), AppError> {
    with_gtk_window(window, |gtk_win| {
        let (rect_opt, no_activate) = DOCK_TARGET
            .lock()
            .map(|t| (t.rect, t.no_activate))
            .unwrap_or((None, true));
        apply_gtk_window_flags(gtk_win, no_activate);
        if let Some(rect) = rect_opt {
            apply_gtk_bounds(gtk_win, rect);
        }
        gtk_win.show_all();
        apply_gtk_window_flags(gtk_win, no_activate);
        if let Some(rect) = rect_opt {
            apply_gtk_bounds(gtk_win, rect);
        }
    })
}

/// Focuses and raises the expanded dock window after bounds have been applied.
pub fn focus_dock_window(window: &WebviewWindow) {
    let _ = window.set_focus();
    let _ = with_gtk_window(window, |gtk_win| {
        gtk_win.set_accept_focus(true);
        gtk_win.set_focus_on_map(true);
        if let Some(gdk_win) = gtk_win.window() {
            gdk_win.set_accept_focus(true);
            gdk_win.raise();
            gdk_win.focus(gtk::gdk::ffi::GDK_CURRENT_TIME as u32);
        }
        gtk_win.present_with_time(gtk::gdk::ffi::GDK_CURRENT_TIME as u32);
    });
}

/// Toggles X11 focusability so Peek/Collapsed do not steal focus from the user's app.
pub fn set_no_activate(window: &WebviewWindow, no_activate: bool) -> Result<(), AppError> {
    if let Ok(mut guard) = DOCK_TARGET.lock() {
        guard.no_activate = no_activate;
    }
    let _ = window.set_focusable(!no_activate);

    with_gtk_window(window, move |gtk_win| {
        gtk_win.set_accept_focus(!no_activate);
        gtk_win.set_focus_on_map(!no_activate);
        if let Some(gdk_win) = gtk_win.window() {
            gdk_win.set_accept_focus(!no_activate);
            gdk_win.set_focus_on_map(!no_activate);
        }
    })
}

/// Atomically updates X11 window position and size in physical coordinates.
pub fn set_bounds_atomic(window: &WebviewWindow, rect: PhysicalRect) -> Result<(), AppError> {
    let no_activate = if let Ok(mut guard) = DOCK_TARGET.lock() {
        guard.rect = Some(rect);
        guard.retries = 0;
        guard.no_activate
    } else {
        true
    };

    with_gtk_window(window, move |gtk_win| {
        apply_gtk_window_flags(gtk_win, no_activate);
        apply_gtk_bounds(gtk_win, rect);
    })
}

pub fn get_cursor_screen_pos(window: &WebviewWindow) -> Option<(i32, i32)> {
    window
        .cursor_position()
        .ok()
        .map(|pos| (pos.x.round() as i32, pos.y.round() as i32))
}

pub fn install_display_change_hook(
    app: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), AppError> {
    let app_handle = app.clone();
    with_gtk_window(window, move |gtk_win| {
        // Re-assert position and dock hints when X11/Mutter first maps the window so
        // the window manager's initial center placement never overrides the right-edge dock.
        gtk_win.connect_map_event(|win, _| {
            let state = DOCK_TARGET
                .lock()
                .ok()
                .map(|t| (t.rect, t.no_activate));
            if let Some((rect_opt, no_activate)) = state {
                apply_gtk_window_flags(win, no_activate);
                if let Some(rect) = rect_opt {
                    apply_gtk_bounds(win, rect);
                }
            }
            gtk::glib::Propagation::Proceed
        });

        // tao schedules a one-shot draw callback that resets `accept_focus` to `true` when
        // `focus: false` is configured; restore our target `no_activate` state after draw.
        gtk_win.connect_draw(|win, _| {
            if let Ok(guard) = DOCK_TARGET.lock()
                && win.accepts_focus() != !guard.no_activate
            {
                let no_activate = guard.no_activate;
                win.set_accept_focus(!no_activate);
                if let Some(gdk_win) = win.window() {
                    gdk_win.set_accept_focus(!no_activate);
                }
            }
            gtk::glib::Propagation::Proceed
        });

        // If Mutter or tao's internal configure callback resets position (e.g. centering on map)
        // or size constraints, snap the window back to the active dock plan.
        gtk_win.connect_configure_event(|win, event| {
            let should_snap = if let Ok(mut guard) = DOCK_TARGET.lock() {
                if let Some(rect) = guard.rect {
                    let (target_x, target_y, target_w, target_h) = logical_bounds(win, rect);
                    let (cur_x, cur_y) = event.position();
                    let (cur_w, cur_h) = event.size();
                    let pos_mismatch =
                        (cur_x - target_x).abs() > 2 || (cur_y - target_y).abs() > 2;
                    let size_mismatch =
                        (cur_w as i32 - target_w).abs() > 2 || (cur_h as i32 - target_h).abs() > 2;

                    if (pos_mismatch || size_mismatch) && guard.retries < 4 {
                        guard.retries += 1;
                        Some((rect, guard.no_activate))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some((rect, no_activate)) = should_snap {
                let win_clone = win.clone();
                gtk::glib::idle_add_local_once(move || {
                    apply_gtk_window_flags(&win_clone, no_activate);
                    apply_gtk_bounds(&win_clone, rect);
                });
            }
            false
        });

        if let Some(screen) = GtkWindowExt::screen(gtk_win) {
            let app_for_monitors = app_handle.clone();
            screen.connect_monitors_changed(move |_| {
                if let Some(state) = app_for_monitors.try_state::<crate::app_state::AppState>() {
                    let _ = state.dock.redock(&app_for_monitors);
                }
            });

            let app_for_size = app_handle.clone();
            screen.connect_size_changed(move |_| {
                if let Some(state) = app_for_size.try_state::<crate::app_state::AppState>() {
                    let _ = state.dock.redock(&app_for_size);
                }
            });
        }
    })
}

pub fn set_webview_low_memory(_window: &WebviewWindow, _low: bool) {}
