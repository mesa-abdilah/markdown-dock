pub mod geometry;
pub mod hotkey;
pub mod monitor;
pub mod platform;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_specta::Event;

use crate::config::MonitorPolicy;
use crate::error::AppError;
use crate::events::{DockFocusRequestEvent, DockModeChangedEvent};
use crate::window_manager::geometry::{
    AnchorEdge, DockMode, DockPlan, compute_dock_plan, vertical_ratio_from_physical_y,
};

pub const DOCK_WINDOW_LABEL: &str = "dock";

#[derive(Debug, Clone, Serialize, Type)]
pub struct DockStateSnapshot {
    pub mode: DockMode,
    pub pinned: bool,
    pub anchor: AnchorEdge,
    pub seq: u32,
    pub vertical_ratio: f64,
    pub plan: DockPlan,
}

struct DockInner {
    mode: DockMode,
    pinned: bool,
    vertical_ratio: f64,
    monitor_policy: MonitorPolicy,
    seq: u32,
    pending_plan: Option<DockPlan>,
    active_plan: Option<DockPlan>,
}

pub struct DockController {
    inner: Mutex<DockInner>,
    low_mem_gen: AtomicU64,
}

impl DockController {
    pub fn new(vertical_ratio: f64, monitor_policy: MonitorPolicy, pinned: bool) -> Self {
        Self {
            inner: Mutex::new(DockInner {
                mode: DockMode::Collapsed,
                pinned,
                vertical_ratio: vertical_ratio.clamp(0.0, 1.0),
                monitor_policy,
                seq: 0,
                pending_plan: None,
                active_plan: None,
            }),
            low_mem_gen: AtomicU64::new(0),
        }
    }

    pub fn dock_window(app: &AppHandle) -> Result<WebviewWindow, AppError> {
        app.get_webview_window(DOCK_WINDOW_LABEL)
            .ok_or_else(|| AppError::Window("Dock window not found".into()))
    }

    pub fn init_window(&self, app: &AppHandle) -> Result<DockStateSnapshot, AppError> {
        let win = Self::dock_window(app)?;
        platform::configure_dock_window(&win)?;
        platform::set_no_activate(&win, true)?;
        platform::install_display_change_hook(app, &win)?;

        let plan = self.plan_transition(app, DockMode::Collapsed)?;
        let snapshot = self.apply_plan(app, plan.seq, false)?;
        Ok(snapshot)
    }

    pub fn get_snapshot(&self, app: &AppHandle) -> Result<DockStateSnapshot, AppError> {
        let guard = self.inner.lock().unwrap();
        if let Some(ref plan) = guard.active_plan {
            return Ok(DockStateSnapshot {
                mode: guard.mode,
                pinned: guard.pinned,
                anchor: plan.anchor,
                seq: guard.seq,
                vertical_ratio: guard.vertical_ratio,
                plan: plan.clone(),
            });
        }
        let target = guard.mode;
        let ratio = guard.vertical_ratio;
        let policy = guard.monitor_policy.clone();
        drop(guard);

        let win = Self::dock_window(app)?;
        let selected = monitor::select_monitor(&win, &policy)?;
        let mut guard = self.inner.lock().unwrap();
        guard.seq += 1;
        let plan = compute_dock_plan(guard.seq, selected.name, selected.work_area, target, ratio);
        guard.active_plan = Some(plan.clone());
        Ok(DockStateSnapshot {
            mode: guard.mode,
            pinned: guard.pinned,
            anchor: plan.anchor,
            seq: guard.seq,
            vertical_ratio: guard.vertical_ratio,
            plan,
        })
    }

    pub fn plan_transition(&self, app: &AppHandle, target: DockMode) -> Result<DockPlan, AppError> {
        let win = Self::dock_window(app)?;
        let (policy, ratio) = {
            let guard = self.inner.lock().unwrap();
            (guard.monitor_policy.clone(), guard.vertical_ratio)
        };
        let selected = monitor::select_monitor(&win, &policy)?;

        let mut guard = self.inner.lock().unwrap();
        guard.seq += 1;
        let plan = compute_dock_plan(guard.seq, selected.name, selected.work_area, target, ratio);
        guard.pending_plan = Some(plan.clone());
        Ok(plan)
    }

    pub fn apply_plan(
        &self,
        app: &AppHandle,
        plan_seq: u32,
        focus_window: bool,
    ) -> Result<DockStateSnapshot, AppError> {
        let win = Self::dock_window(app)?;
        let (plan, pinned, vertical_ratio) = {
            let mut guard = self.inner.lock().unwrap();
            if plan_seq != guard.seq {
                return Err(AppError::StalePlan {
                    provided: plan_seq,
                    current: guard.seq,
                });
            }
            let plan = guard
                .pending_plan
                .take()
                .or_else(|| guard.active_plan.clone())
                .ok_or_else(|| AppError::Window("No dock plan available".into()))?;
            guard.mode = plan.target_mode;
            guard.active_plan = Some(plan.clone());
            (plan, guard.pinned, guard.vertical_ratio)
        };

        let no_activate = matches!(plan.target_mode, DockMode::Collapsed | DockMode::Peek);
        platform::set_no_activate(&win, no_activate)?;
        platform::set_bounds_atomic(&win, plan.rect_phys)?;

        if focus_window && !no_activate {
            let _ = win.set_focus();
        }
        self.schedule_webview_memory_policy(app, &win, plan.target_mode);

        let _ = DockModeChangedEvent {
            mode: plan.target_mode,
            pinned,
            anchor: plan.anchor,
            seq: plan.seq,
        }
        .emit(app);

        Ok(DockStateSnapshot {
            mode: plan.target_mode,
            pinned,
            anchor: plan.anchor,
            seq: plan.seq,
            vertical_ratio,
            plan,
        })
    }

    pub fn set_pinned(&self, app: &AppHandle, pinned: bool) -> Result<DockStateSnapshot, AppError> {
        let snapshot = {
            let mut guard = self.inner.lock().unwrap();
            guard.pinned = pinned;
            let plan = guard
                .active_plan
                .clone()
                .ok_or_else(|| AppError::Window("Dock not initialized".into()))?;
            DockStateSnapshot {
                mode: guard.mode,
                pinned,
                anchor: plan.anchor,
                seq: guard.seq,
                vertical_ratio: guard.vertical_ratio,
                plan,
            }
        };

        let _ = DockModeChangedEvent {
            mode: snapshot.mode,
            pinned: snapshot.pinned,
            anchor: snapshot.anchor,
            seq: snapshot.seq,
        }
        .emit(app);

        Ok(snapshot)
    }

    pub fn set_vertical_ratio(&self, app: &AppHandle, ratio: f64) -> Result<DockPlan, AppError> {
        let clamped = ratio.clamp(0.0, 1.0);
        let mode = {
            let mut guard = self.inner.lock().unwrap();
            guard.vertical_ratio = clamped;
            guard.mode
        };
        let plan = self.plan_transition(app, mode)?;
        let _ = self.apply_plan(app, plan.seq, false)?;
        Ok(plan)
    }

    pub fn nudge_vertical_by_screen_y(
        &self,
        app: &AppHandle,
        center_phys_y: f64,
    ) -> Result<DockPlan, AppError> {
        let win = Self::dock_window(app)?;
        let policy = {
            let guard = self.inner.lock().unwrap();
            guard.monitor_policy.clone()
        };
        let selected = monitor::select_monitor(&win, &policy)?;
        let ratio = vertical_ratio_from_physical_y(selected.work_area, center_phys_y);
        self.set_vertical_ratio(app, ratio)
    }

    pub fn update_monitor_policy(
        &self,
        app: &AppHandle,
        policy: MonitorPolicy,
    ) -> Result<DockPlan, AppError> {
        {
            let mut guard = self.inner.lock().unwrap();
            guard.monitor_policy = policy;
        }
        self.redock(app)
    }

    pub fn redock(&self, app: &AppHandle) -> Result<DockPlan, AppError> {
        let mode = {
            let guard = self.inner.lock().unwrap();
            guard.mode
        };
        let plan = self.plan_transition(app, mode)?;
        let _ = self.apply_plan(app, plan.seq, false)?;
        Ok(plan)
    }

    pub fn toggle_from_hotkey(&self, app: &AppHandle) -> Result<DockMode, AppError> {
        let (current_mode, pinned) = {
            let guard = self.inner.lock().unwrap();
            (guard.mode, guard.pinned)
        };
        let target = match current_mode {
            DockMode::Collapsed | DockMode::Peek | DockMode::DropTarget => DockMode::Expanded,
            DockMode::Expanded | DockMode::Wide | DockMode::Fullscreen => {
                if pinned {
                    // When pinned, hotkey brings focus back rather than hiding unexpectedly.
                    let win = Self::dock_window(app)?;
                    let _ = win.set_focus();
                    let _ = DockFocusRequestEvent { focus_search: true }.emit(app);
                    return Ok(current_mode);
                }
                DockMode::Collapsed
            }
        };

        if target == DockMode::Expanded {
            let plan = self.plan_transition(app, DockMode::Expanded)?;
            let _ = self.apply_plan(app, plan.seq, true)?;
            let _ = DockFocusRequestEvent { focus_search: true }.emit(app);
        }
        // For Collapsed, frontend receives HotkeyTriggeredEvent and runs spring close -> shrink.
        Ok(target)
    }

    pub(crate) fn low_mem_gen_current(&self) -> u64 {
        self.low_mem_gen.load(Ordering::Acquire)
    }

    /// Immediately restores normal memory target on expand/peek, and defers
    /// `Low` memory target until 30s after remaining `Collapsed` so collapse
    /// springs finish before WebView2 triggers background GC/trim.
    fn schedule_webview_memory_policy(
        &self,
        app: &AppHandle,
        window: &WebviewWindow,
        mode: DockMode,
    ) {
        let epoch = self.low_mem_gen.fetch_add(1, Ordering::AcqRel) + 1;
        if mode != DockMode::Collapsed {
            platform::set_webview_low_memory(window, false);
            return;
        }

        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let main_handle = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                let Some(state) = main_handle.try_state::<crate::app_state::AppState>() else {
                    return;
                };
                if state.dock.low_mem_gen_current() != epoch {
                    return;
                }
                if let Ok(win) = DockController::dock_window(&main_handle) {
                    platform::set_webview_low_memory(&win, true);
                }
            });
        });
    }
}
