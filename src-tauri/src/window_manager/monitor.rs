use serde::Serialize;
use specta::Type;
use tauri::{Monitor, WebviewWindow};

use crate::config::MonitorPolicy;
use crate::error::AppError;
use crate::window_manager::geometry::MonitorWorkArea;
use crate::window_manager::platform;

#[derive(Debug, Clone, Serialize, Type)]
pub struct MonitorInfo {
    pub name: String,
    pub is_primary: bool,
    pub scale_factor: f64,
    pub width: u32,
    pub height: u32,
    pub work_area: MonitorWorkArea,
}

pub struct SelectedMonitor {
    pub name: String,
    pub work_area: MonitorWorkArea,
}

fn monitor_name(m: &Monitor, idx: usize) -> String {
    m.name()
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| format!("Monitor-{}", idx + 1))
}

fn to_work_area(m: &Monitor) -> MonitorWorkArea {
    let wa = m.work_area();
    if wa.size.width > 0 && wa.size.height > 0 {
        MonitorWorkArea {
            x: wa.position.x,
            y: wa.position.y,
            width: wa.size.width,
            height: wa.size.height,
            scale_factor: m.scale_factor(),
        }
    } else {
        let pos = m.position();
        let size = m.size();
        MonitorWorkArea {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            scale_factor: m.scale_factor(),
        }
    }
}

pub fn list_monitors(window: &WebviewWindow) -> Result<Vec<MonitorInfo>, AppError> {
    let primary_name = window
        .primary_monitor()?
        .and_then(|m| m.name().map(std::string::ToString::to_string));

    let monitors = window.available_monitors()?;
    Ok(monitors
        .into_iter()
        .enumerate()
        .map(|(idx, m)| {
            let name = monitor_name(&m, idx);
            let is_primary = primary_name.as_deref() == Some(name.as_str())
                || (primary_name.is_none() && idx == 0);
            let size = m.size();
            MonitorInfo {
                name,
                is_primary,
                scale_factor: m.scale_factor(),
                width: size.width,
                height: size.height,
                work_area: to_work_area(&m),
            }
        })
        .collect())
}

pub fn select_monitor(
    window: &WebviewWindow,
    policy: &MonitorPolicy,
) -> Result<SelectedMonitor, AppError> {
    let monitors = window.available_monitors()?;
    if monitors.is_empty() {
        return Err(AppError::Monitor("No monitors reported by OS".into()));
    }

    match policy {
        MonitorPolicy::FollowCursor => {
            if let Some((cx, cy)) = platform::get_cursor_screen_pos(window) {
                for (idx, m) in monitors.iter().enumerate() {
                    let pos = m.position();
                    let size = m.size();
                    let inside_x = cx >= pos.x && cx < pos.x + size.width as i32;
                    let inside_y = cy >= pos.y && cy < pos.y + size.height as i32;
                    if inside_x && inside_y {
                        return Ok(SelectedMonitor {
                            name: monitor_name(m, idx),
                            work_area: to_work_area(m),
                        });
                    }
                }
            }
        }
        MonitorPolicy::Specific(target_name) => {
            for (idx, m) in monitors.iter().enumerate() {
                let name = monitor_name(m, idx);
                if &name == target_name {
                    return Ok(SelectedMonitor {
                        name,
                        work_area: to_work_area(m),
                    });
                }
            }
            tracing::warn!(
                "Configured monitor '{}' not found, falling back to primary",
                target_name
            );
        }
        MonitorPolicy::Primary => {}
    }

    if let Some(primary) = window.primary_monitor()? {
        return Ok(SelectedMonitor {
            name: primary
                .name()
                .map(std::string::ToString::to_string)
                .unwrap_or_else(|| "Primary".to_string()),
            work_area: to_work_area(&primary),
        });
    }

    let first = &monitors[0];
    Ok(SelectedMonitor {
        name: monitor_name(first, 0),
        work_area: to_work_area(first),
    })
}
