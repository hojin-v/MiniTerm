#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The right and bottom edges stay fixed for every frame, including rounding.
pub fn interpolate_anchored(from: Rect, to: Rect, progress: f64) -> Rect {
    let eased = 1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3);
    let width = (from.width as f64 + (to.width as f64 - from.width as f64) * eased).round() as u32;
    let height =
        (from.height as f64 + (to.height as f64 - from.height as f64) * eased).round() as u32;
    Rect {
        x: to.x + to.width as i32 - width as i32,
        y: to.y + to.height as i32 - height as i32,
        width,
        height,
    }
}

pub fn hud_bounds(work: Rect, scale: f64, expanded: bool) -> Rect {
    configured_bounds(work, scale, expanded, &WindowPreferences::default())
}

pub fn configured_bounds(
    work: Rect,
    scale: f64,
    expanded: bool,
    preferences: &WindowPreferences,
) -> Rect {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let right = ((preferences.right as f64 * scale).round() as u32).min(work.width / 4);
    let bottom = ((preferences.bottom as f64 * scale).round() as u32).min(work.height / 4);
    let available_width = work.width.saturating_sub(right * 2).max(1);
    let available_height = work.height.saturating_sub(bottom * 2).max(1);
    let width = preferences.mini_width as f64 * scale;
    let height = if expanded {
        preferences
            .expanded_height
            .map_or(
                work.height as f64 * preferences.expanded_height_percent as f64 / 100.0,
                |height| height as f64 * scale,
            )
            .max(preferences.mini_height as f64 * scale)
    } else {
        preferences.mini_height as f64 * scale
    };
    let width = (width.round() as u32).min(available_width).max(1);
    let height = (height.round() as u32).min(available_height).max(1);
    let (right, bottom) = preferences
        .anchor
        .as_ref()
        .map_or((right, bottom), |anchor| {
            (
                work.width * anchor.right as u32 / 10_000,
                work.height * anchor.bottom as u32 / 10_000,
            )
        });
    Rect {
        x: work.x + work.width.saturating_sub(right + width) as i32,
        y: work.y + work.height.saturating_sub(bottom + height) as i32,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animation_stays_anchored_and_finishes_at_the_exact_target() {
        let work = Rect {
            x: -1920,
            y: 100,
            width: 1920,
            height: 1040,
        };
        let from = hud_bounds(work, 1.25, false);
        let to = hud_bounds(work, 1.25, true);
        for frame in 0..=18 {
            let bounds = interpolate_anchored(from, to, frame as f64 / 18.0);
            assert_eq!(bounds.x + bounds.width as i32, to.x + to.width as i32);
            assert_eq!(bounds.y + bounds.height as i32, to.y + to.height as i32);
            assert!(bounds.width >= from.width && bounds.width <= to.width);
        }
        assert_eq!(interpolate_anchored(from, to, 1.0), to);
        assert_eq!(interpolate_anchored(to, from, 1.0), from);
    }
    #[test]
    fn custom_sizes_and_offsets_fit_and_keep_the_anchor_at_extremes() {
        for (width, height) in [(320, 200), (1920, 1040), (3840, 2120)] {
            for scale in [1.0, 1.25, 2.0, 3.0] {
                for gap in [0, 8, 400] {
                    let preferences = WindowPreferences {
                        mini_width: 1000,
                        mini_height: 320,
                        right: gap,
                        bottom: gap,
                        expanded_width_percent: 90,
                        ..Default::default()
                    };
                    let work = Rect {
                        x: -3840,
                        y: -180,
                        width,
                        height,
                    };
                    let mini = configured_bounds(work, scale, false, &preferences);
                    let expanded = configured_bounds(work, scale, true, &preferences);
                    assert_eq!(
                        mini.x + mini.width as i32,
                        expanded.x + expanded.width as i32
                    );
                    assert_eq!(
                        mini.y + mini.height as i32,
                        expanded.y + expanded.height as i32
                    );
                    for bounds in [mini, expanded] {
                        assert!(bounds.x >= work.x && bounds.y >= work.y);
                        assert!(bounds.x + bounds.width as i32 <= work.x + width as i32);
                        assert!(bounds.y + bounds.height as i32 <= work.y + height as i32);
                    }
                }
            }
        }
    }
    #[test]
    fn expansion_keeps_bottom_and_right_on_negative_coordinate_monitor() {
        let work = Rect {
            x: -2560,
            y: -180,
            width: 2560,
            height: 1400,
        };
        let mini = hud_bounds(work, 1.5, false);
        let large = hud_bounds(work, 1.5, true);
        assert_eq!(mini.x + mini.width as i32, large.x + large.width as i32);
        assert_eq!(mini.y + mini.height as i32, large.y + large.height as i32);
        assert!(large.x >= work.x && large.y >= work.y);
    }
    #[test]
    fn small_work_areas_stay_inside_screen() {
        for scale in [1.0, 2.0, f64::NAN] {
            let work = Rect {
                x: 50,
                y: 30,
                width: 320,
                height: 200,
            };
            let rect = hud_bounds(work, scale, true);
            assert!(rect.x >= work.x && rect.y >= work.y);
            assert!(rect.x + rect.width as i32 <= work.x + work.width as i32);
            assert!(rect.y + rect.height as i32 <= work.y + work.height as i32);
        }
    }
    #[test]
    fn dragged_anchors_survive_dpi_and_expansion_is_vertical_and_clamped() {
        let work = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 1040,
        };
        let moved = Rect {
            x: -1700,
            y: 300,
            width: 560,
            height: 112,
        };
        let preferences = WindowPreferences {
            anchor: Some(WindowAnchor::from_bounds(work, moved)),
            ..Default::default()
        };
        let mini = configured_bounds(work, 1.0, false, &preferences);
        assert!((mini.x - moved.x).abs() <= 1 && (mini.y - moved.y).abs() <= 1);
        let expanded = configured_bounds(work, 1.0, true, &preferences);
        assert_eq!(mini.width, expanded.width);
        assert!(expanded.x >= work.x && expanded.y >= work.y);
        for scale in [1.25, 2.0] {
            let moved = configured_bounds(work, scale, true, &preferences);
            assert!(moved.x >= work.x && moved.y >= work.y);
            assert!(moved.x + moved.width as i32 <= work.x + work.width as i32);
        }
        let short = WindowPreferences {
            expanded_height: Some(96),
            ..preferences
        };
        let expanded = configured_bounds(work, 1.0, true, &short);
        assert!(expanded.height >= mini.height);
    }
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowPreferences {
    pub monitor: String,
    pub anchor: Option<WindowAnchor>,
    pub mini_view: MiniView,
    pub mini_show_status: bool,
    pub mini_show_title: bool,
    pub mini_show_output: bool,
    pub mini_show_tasks: bool,
    pub mini_show_battery: bool,
    pub mini_width: u16,
    pub mini_height: u16,
    pub right: u16,
    pub bottom: u16,
    pub expanded_width_percent: u16,
    pub expanded_height_percent: u16,
    pub expanded_height: Option<u16>,
    pub auto_collapse: bool,
    pub reduced_motion: bool,
    pub transparency: u16,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            monitor: String::new(),
            anchor: None,
            mini_view: MiniView::Input,
            mini_show_status: true,
            mini_show_title: true,
            mini_show_output: true,
            mini_show_tasks: true,
            mini_show_battery: true,
            mini_width: 560,
            mini_height: 112,
            right: 8,
            bottom: 8,
            expanded_width_percent: 55,
            expanded_height_percent: 70,
            expanded_height: None,
            auto_collapse: true,
            reduced_motion: false,
            transparency: 0,
        }
    }
}

impl WindowPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !(280..=1000).contains(&self.mini_width)
            || !(96..=320).contains(&self.mini_height)
            || self.right > 400
            || self.bottom > 400
            || !(40..=90).contains(&self.expanded_width_percent)
            || !(40..=90).contains(&self.expanded_height_percent)
            || self.monitor.len() > 128
            || self.monitor.chars().any(char::is_control)
            || self
                .expanded_height
                .is_some_and(|n| !(96..=10_000).contains(&n))
            || self.transparency > 80
            || self
                .anchor
                .as_ref()
                .is_some_and(|a| a.right > 10_000 || a.bottom > 10_000)
        {
            return Err("창 크기·간격·모니터·모양 설정의 범위를 확인하세요.".into());
        }
        Ok(())
    }
}

/// Fractions of the monitor work area, so a saved drag position survives DPI changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowAnchor {
    pub right: u16,
    pub bottom: u16,
}

impl WindowAnchor {
    pub fn from_bounds(work: Rect, bounds: Rect) -> Self {
        Self {
            right: ((work.x + work.width as i32 - bounds.x - bounds.width as i32).max(0) as u64
                * 10_000
                / work.width.max(1) as u64)
                .min(10_000) as u16,
            bottom: ((work.y + work.height as i32 - bounds.y - bounds.height as i32).max(0) as u64
                * 10_000
                / work.height.max(1) as u64)
                .min(10_000) as u16,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniView {
    #[default]
    Input,
    Summary,
}
