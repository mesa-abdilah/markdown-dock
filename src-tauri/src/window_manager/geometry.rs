use serde::{Deserialize, Serialize};
use specta::Type;

pub const EDGE_MARGIN_LOGICAL: f64 = 8.0;
pub const SHADOW_PAD_LOGICAL: f64 = 8.0;
pub const CAPSULE_W_LOGICAL: f64 = 12.0;
pub const CAPSULE_H_LOGICAL: f64 = 80.0;
pub const PEEK_W_LOGICAL: f64 = 58.0;
pub const PEEK_H_LOGICAL: f64 = 96.0;
pub const EXPANDED_W_LOGICAL: f64 = 460.0;
pub const EXPANDED_MAX_H_LOGICAL: f64 = 680.0;
pub const WIDE_W_LOGICAL: f64 = 1060.0;
pub const WIDE_MAX_H_LOGICAL: f64 = 760.0;
pub const DROP_W_LOGICAL: f64 = 240.0;
pub const DROP_H_LOGICAL: f64 = 320.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DockMode {
    Collapsed,
    Peek,
    Expanded,
    Wide,
    Fullscreen,
    DropTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum AnchorEdge {
    TopRight,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct MonitorWorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct LogicalSizeF {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct DockPlan {
    pub seq: u32,
    pub target_mode: DockMode,
    pub anchor: AnchorEdge,
    pub monitor_name: String,
    pub scale_factor: f64,
    pub rect_phys: PhysicalRect,
    pub window_logical: LogicalSizeF,
    pub visual_logical: LogicalSizeF,
    pub shadow_pad: f64,
}

/// Visual content dimensions (excluding shadow padding) in logical pixels.
pub fn visual_dimensions(mode: DockMode, work_area_logical_h: f64) -> LogicalSizeF {
    let safe_max_h = (work_area_logical_h - 2.0 * EDGE_MARGIN_LOGICAL).max(CAPSULE_H_LOGICAL);
    match mode {
        DockMode::Collapsed => LogicalSizeF {
            width: CAPSULE_W_LOGICAL,
            height: CAPSULE_H_LOGICAL,
        },
        DockMode::Peek => LogicalSizeF {
            width: PEEK_W_LOGICAL,
            height: PEEK_H_LOGICAL,
        },
        DockMode::Expanded => LogicalSizeF {
            width: EXPANDED_W_LOGICAL,
            height: EXPANDED_MAX_H_LOGICAL
                .min(0.80 * work_area_logical_h)
                .min(safe_max_h)
                .max(CAPSULE_H_LOGICAL),
        },
        DockMode::Wide => LogicalSizeF {
            width: WIDE_W_LOGICAL,
            height: WIDE_MAX_H_LOGICAL
                .min(0.85 * work_area_logical_h)
                .min(safe_max_h)
                .max(CAPSULE_H_LOGICAL),
        },
        DockMode::Fullscreen => LogicalSizeF {
            width: WIDE_W_LOGICAL,
            height: safe_max_h,
        },
        DockMode::DropTarget => LogicalSizeF {
            width: DROP_W_LOGICAL,
            height: DROP_H_LOGICAL
                .min(0.60 * work_area_logical_h)
                .min(safe_max_h)
                .max(CAPSULE_H_LOGICAL),
        },
    }
}

/// Computes the exact window bounds in physical pixels for the target dock mode.
///
/// The window bounds include `SHADOW_PAD_LOGICAL` on all four sides around the visual shell
/// so CSS shadows and borders are never clipped by OS window boundaries.
pub fn compute_dock_plan(
    seq: u32,
    monitor_name: String,
    wa: MonitorWorkArea,
    target_mode: DockMode,
    vertical_ratio: f64,
) -> DockPlan {
    let scale = if wa.scale_factor > 0.0 {
        wa.scale_factor
    } else {
        1.0
    };
    let ratio = vertical_ratio.clamp(0.0, 1.0);

    let wa_x_l = f64::from(wa.x) / scale;
    let wa_y_l = f64::from(wa.y) / scale;
    let wa_w_l = f64::from(wa.width) / scale;
    let wa_h_l = f64::from(wa.height) / scale;

    let pad = SHADOW_PAD_LOGICAL;
    let margin = EDGE_MARGIN_LOGICAL;

    let cap_vis = visual_dimensions(DockMode::Collapsed, wa_h_l);
    let cap_win_h = cap_vis.height + 2.0 * pad;

    // Visual capsule vertical range clamped within work area minus margin.
    let cap_min_y = wa_y_l + margin - pad;
    let cap_max_y = (wa_y_l + wa_h_l - margin - cap_win_h + pad).max(cap_min_y);
    let raw_cap_y = wa_y_l + ratio * wa_h_l - cap_win_h / 2.0;
    let y_cap = raw_cap_y.clamp(cap_min_y, cap_max_y);

    let mut target_vis = visual_dimensions(target_mode, wa_h_l);
    let (win_w, win_h, x_win, anchor, y_win) = if target_mode == DockMode::Fullscreen {
        let full_w = (wa_w_l - 2.0 * margin).max(cap_vis.width);
        let full_h = (wa_h_l - 2.0 * margin).max(cap_vis.height);
        target_vis = LogicalSizeF {
            width: full_w,
            height: full_h,
        };
        (
            full_w + 2.0 * pad,
            full_h + 2.0 * pad,
            wa_x_l + margin - pad,
            AnchorEdge::TopRight,
            wa_y_l + margin - pad,
        )
    } else {
        let win_w =
            (target_vis.width + 2.0 * pad).min((wa_w_l - margin).max(cap_vis.width + 2.0 * pad));
        let win_h = target_vis.height + 2.0 * pad;

        // Right edge of the visual shell stays flush at (wa_x_l + wa_w_l - margin).
        let x_win = (wa_x_l + wa_w_l - margin + pad - win_w).max(wa_x_l);

        // For Peek (same height as Collapsed), look ahead using Expanded height so
        // the CSS anchor stays identical across Collapsed -> Peek -> Expanded.
        let anchor_check_h = if target_mode == DockMode::Peek {
            visual_dimensions(DockMode::Expanded, wa_h_l).height + 2.0 * pad
        } else {
            win_h
        };

        // Choose anchor so the capsule corner stays stationary across Collapsed <-> Expanded.
        let fits_downward = (y_cap + anchor_check_h - pad) <= (wa_y_l + wa_h_l - margin + 0.5);
        let (anchor, y_win) = if fits_downward {
            (AnchorEdge::TopRight, y_cap)
        } else {
            let bottom_aligned_y = y_cap + cap_win_h - win_h;
            let min_y = wa_y_l + margin - pad;
            (AnchorEdge::BottomRight, bottom_aligned_y.max(min_y))
        };
        (win_w, win_h, x_win, anchor, y_win)
    };

    let rect_phys = PhysicalRect {
        x: (x_win * scale).round() as i32,
        y: (y_win * scale).round() as i32,
        width: (win_w * scale).round().max(1.0) as u32,
        height: (win_h * scale).round().max(1.0) as u32,
    };

    DockPlan {
        seq,
        target_mode,
        anchor,
        monitor_name,
        scale_factor: scale,
        rect_phys,
        window_logical: LogicalSizeF {
            width: win_w,
            height: win_h,
        },
        visual_logical: target_vis,
        shadow_pad: pad,
    }
}

/// Converts a drag-move screen Y (physical) into a clamped vertical ratio in [0.0, 1.0].
pub fn vertical_ratio_from_physical_y(wa: MonitorWorkArea, center_phys_y: f64) -> f64 {
    let scale = if wa.scale_factor > 0.0 {
        wa.scale_factor
    } else {
        1.0
    };
    let wa_y_l = f64::from(wa.y) / scale;
    let wa_h_l = (f64::from(wa.height) / scale).max(1.0);
    let center_l = center_phys_y / scale;
    ((center_l - wa_y_l) / wa_h_l).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_wa(scale: f64) -> MonitorWorkArea {
        MonitorWorkArea {
            x: 0,
            y: 0,
            width: (1920.0 * scale) as u32,
            height: (1032.0 * scale) as u32, // 1080 minus 48px taskbar
            scale_factor: scale,
        }
    }

    #[test]
    fn collapsed_sticks_to_right_edge_across_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let wa = std_wa(scale);
            let plan = compute_dock_plan(1, "Primary".into(), wa, DockMode::Collapsed, 0.5);
            let expected_right = wa.x + wa.width as i32
                - ((EDGE_MARGIN_LOGICAL - SHADOW_PAD_LOGICAL) * scale).round() as i32;
            let actual_right = plan.rect_phys.x + plan.rect_phys.width as i32;
            assert!(
                (actual_right - expected_right).abs() <= 1,
                "scale={scale}: actual_right={actual_right}, expected={expected_right}"
            );
        }
    }

    #[test]
    fn expands_downward_when_near_top_and_upward_when_near_bottom() {
        let wa = std_wa(1.0);
        let top_cap = compute_dock_plan(1, "M".into(), wa, DockMode::Collapsed, 0.1);
        let top_exp = compute_dock_plan(2, "M".into(), wa, DockMode::Expanded, 0.1);
        assert_eq!(top_exp.anchor, AnchorEdge::TopRight);
        assert_eq!(top_exp.rect_phys.y, top_cap.rect_phys.y);

        let bot_cap = compute_dock_plan(3, "M".into(), wa, DockMode::Collapsed, 0.95);
        let bot_exp = compute_dock_plan(4, "M".into(), wa, DockMode::Expanded, 0.95);
        assert_eq!(bot_exp.anchor, AnchorEdge::BottomRight);
        assert_eq!(
            bot_exp.rect_phys.y + bot_exp.rect_phys.height as i32,
            bot_cap.rect_phys.y + bot_cap.rect_phys.height as i32
        );
    }

    #[test]
    fn respects_right_side_taskbar_and_secondary_monitor_offset() {
        // Secondary monitor to the right of primary, with right-side taskbar reducing width.
        let wa = MonitorWorkArea {
            x: 2560,
            y: 120,
            width: 1840, // 1920 - 80 taskbar
            height: 1080,
            scale_factor: 1.5,
        };
        let plan = compute_dock_plan(10, "DP-2".into(), wa, DockMode::Wide, 0.5);
        assert!(plan.rect_phys.x >= wa.x);
        assert!(plan.rect_phys.x + plan.rect_phys.width as i32 <= wa.x + wa.width as i32);
        assert!(plan.rect_phys.y >= wa.y - (SHADOW_PAD_LOGICAL * 1.5).round() as i32);
        assert!(
            plan.rect_phys.y + plan.rect_phys.height as i32
                <= wa.y + wa.height as i32 + (SHADOW_PAD_LOGICAL * 1.5).round() as i32
        );
    }

    #[test]
    fn peek_preserves_y_and_anchor_for_seamless_expand() {
        let wa = std_wa(1.25);
        for ratio in [0.1, 0.5, 0.92] {
            let cap = compute_dock_plan(1, "M".into(), wa, DockMode::Collapsed, ratio);
            let peek = compute_dock_plan(2, "M".into(), wa, DockMode::Peek, ratio);
            let exp = compute_dock_plan(3, "M".into(), wa, DockMode::Expanded, ratio);
            assert_eq!(peek.anchor, exp.anchor);
            if peek.anchor == AnchorEdge::TopRight {
                assert_eq!(peek.rect_phys.y, cap.rect_phys.y);
            } else {
                let peek_bottom = peek.rect_phys.y + peek.rect_phys.height as i32;
                let cap_bottom = cap.rect_phys.y + cap.rect_phys.height as i32;
                assert!(
                    (peek_bottom - cap_bottom).abs() <= 1,
                    "peek_bottom={peek_bottom}, cap_bottom={cap_bottom}"
                );
            }
        }
    }

    #[test]
    fn handles_small_screen_height_without_overflow() {
        let wa = MonitorWorkArea {
            x: 0,
            y: 0,
            width: 1024,
            height: 600,
            scale_factor: 1.0,
        };
        let plan = compute_dock_plan(1, "Small".into(), wa, DockMode::Wide, 0.9);
        assert!(plan.rect_phys.y >= 0);
        assert!(plan.rect_phys.y + plan.rect_phys.height as i32 <= 600 + SHADOW_PAD_LOGICAL as i32);
    }

    #[test]
    fn fullscreen_fills_work_area_across_dpi() {
        for scale in [1.0, 1.5] {
            let wa = std_wa(scale);
            let plan = compute_dock_plan(1, "Primary".into(), wa, DockMode::Fullscreen, 0.5);
            assert_eq!(plan.anchor, AnchorEdge::TopRight);
            assert!((plan.visual_logical.width - (1920.0 - 16.0)).abs() < 0.01);
            assert!((plan.visual_logical.height - (1032.0 - 16.0)).abs() < 0.01);
            assert_eq!(plan.rect_phys.x, 0);
            assert_eq!(plan.rect_phys.y, 0);
        }
    }
}
