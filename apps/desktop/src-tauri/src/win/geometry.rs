//! Pure overlay geometry: corner anchoring and hit-testing, in physical
//! (device) pixels. No OS calls, so it is unit-tested on every platform.

use serde::{Deserialize, Serialize};

/// Screen corner the buddies live in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

/// Axis-aligned rectangle in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        let (left, top) = (i64::from(self.x), i64::from(self.y));
        x >= left
            && y >= top
            && x < left + i64::from(self.width)
            && y < top + i64::from(self.height)
    }
}

/// A rectangle in CSS (logical) pixels relative to the overlay's top-left
/// corner, as the frontend reports buddy and hover-card bounds.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Where the overlay goes: `size` logical pixels, `margin` logical pixels
/// from the edges of `work_area` (the monitor minus the taskbar), at `scale`
/// (the monitor's DPI scale). The result never exceeds the work area.
pub fn anchor(work_area: Rect, size: (f64, f64), scale: f64, corner: Corner, margin: f64) -> Rect {
    let to_px = |logical: f64| (logical * scale).round().max(0.0) as u32;
    let width = to_px(size.0).min(work_area.width);
    let height = to_px(size.1).min(work_area.height);
    let margin_x = to_px(margin).min(work_area.width - width);
    let margin_y = to_px(margin).min(work_area.height - height);

    let left = work_area.x + margin_x as i32;
    let right = work_area.x + (work_area.width - width - margin_x) as i32;
    let top = work_area.y + margin_y as i32;
    let bottom = work_area.y + (work_area.height - height - margin_y) as i32;
    let (x, y) = match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Where the anchored rectangle `home` sits inside `outer` (the expanded
/// overlay), in logical pixels relative to `outer`'s top-left corner. The
/// frontend keeps the buddies there while the window is expanded, so the
/// group does not jump when the window grows.
pub fn inner_rect(outer: Rect, home: Rect, scale: f64) -> LogicalRect {
    LogicalRect {
        x: f64::from(home.x - outer.x) / scale,
        y: f64::from(home.y - outer.y) / scale,
        width: f64::from(home.width) / scale,
        height: f64::from(home.height) / scale,
    }
}

/// Converts a frontend region to screen pixels, given the overlay's
/// top-left corner on screen and its scale factor.
pub fn to_screen(region: LogicalRect, origin: (i32, i32), scale: f64) -> Rect {
    let x0 = (region.x * scale).floor();
    let y0 = (region.y * scale).floor();
    let x1 = ((region.x + region.width.max(0.0)) * scale).ceil();
    let y1 = ((region.y + region.height.max(0.0)) * scale).ceil();
    Rect {
        x: origin.0 + x0 as i32,
        y: origin.1 + y0 as i32,
        width: (x1 - x0).max(0.0) as u32,
        height: (y1 - y0).max(0.0) as u32,
    }
}

/// Whether the screen point lies in any region.
pub fn hit(regions: &[Rect], x: i32, y: i32) -> bool {
    regions.iter().any(|r| r.contains(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920x1080 monitor with a 48 px taskbar at the bottom.
    const WORK: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1032,
    };

    #[test]
    fn inner_rect_places_home_inside_the_expanded_overlay() {
        // Second monitor to the left, 150 % scale, taskbar on top.
        let work = Rect {
            x: -2880,
            y: 72,
            width: 2880,
            height: 1548,
        };
        let home = anchor(work, (200.0, 100.0), 1.5, Corner::BottomRight, 8.0);
        assert_eq!(
            inner_rect(work, home, 1.5),
            LogicalRect {
                x: 1920.0 - 200.0 - 8.0,
                y: 1032.0 - 100.0 - 8.0,
                width: 200.0,
                height: 100.0
            }
        );
    }

    #[test]
    fn anchors_each_corner_inside_the_work_area() {
        let at = |corner| anchor(WORK, (480.0, 160.0), 1.0, corner, 8.0);
        assert_eq!(
            at(Corner::TopLeft),
            Rect {
                x: 8,
                y: 8,
                width: 480,
                height: 160
            }
        );
        assert_eq!(
            at(Corner::TopRight),
            Rect {
                x: 1432,
                y: 8,
                width: 480,
                height: 160
            }
        );
        assert_eq!(
            at(Corner::BottomLeft),
            Rect {
                x: 8,
                y: 864,
                width: 480,
                height: 160
            }
        );
        assert_eq!(
            at(Corner::BottomRight),
            Rect {
                x: 1432,
                y: 864,
                width: 480,
                height: 160
            }
        );
    }

    #[test]
    fn bottom_anchor_stays_above_the_taskbar() {
        let rect = anchor(WORK, (480.0, 160.0), 1.0, Corner::BottomRight, 0.0);
        assert_eq!(rect.y + rect.height as i32, 1032);
    }

    #[test]
    fn scales_size_and_margin_by_dpi() {
        let rect = anchor(WORK, (480.0, 160.0), 1.5, Corner::BottomRight, 8.0);
        assert_eq!((rect.width, rect.height), (720, 240));
        assert_eq!(rect.x, 1920 - 720 - 12);
        assert_eq!(rect.y, 1032 - 240 - 12);
    }

    #[test]
    fn second_monitor_with_offset_and_top_taskbar() {
        // Monitor to the left of the primary, taskbar at its top.
        let work = Rect {
            x: -2560,
            y: 40,
            width: 2560,
            height: 1400,
        };
        let rect = anchor(work, (480.0, 160.0), 2.0, Corner::TopLeft, 8.0);
        assert_eq!(
            rect,
            Rect {
                x: -2544,
                y: 56,
                width: 960,
                height: 320
            }
        );
    }

    #[test]
    fn never_larger_than_the_work_area() {
        let tiny = Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 100,
        };
        let rect = anchor(tiny, (480.0, 160.0), 1.0, Corner::BottomRight, 8.0);
        assert_eq!(rect, tiny);
    }

    #[test]
    fn regions_convert_to_screen_pixels() {
        let region = LogicalRect {
            x: 10.0,
            y: 20.0,
            width: 32.0,
            height: 32.0,
        };
        assert_eq!(
            to_screen(region, (1432, 864), 1.0),
            Rect {
                x: 1442,
                y: 884,
                width: 32,
                height: 32
            }
        );
        // Fractional scale rounds outward, so the region never shrinks.
        let rect = to_screen(region, (0, 0), 1.25);
        assert_eq!(
            rect,
            Rect {
                x: 12,
                y: 25,
                width: 41,
                height: 40
            }
        );
    }

    #[test]
    fn hit_testing_uses_half_open_bounds() {
        let regions = [Rect {
            x: 100,
            y: 100,
            width: 10,
            height: 10,
        }];
        assert!(hit(&regions, 100, 100));
        assert!(hit(&regions, 109, 109));
        assert!(!hit(&regions, 110, 105));
        assert!(!hit(&regions, 99, 105));
        assert!(!hit(&[], 0, 0));
    }

    #[test]
    fn corners_use_kebab_case_names() {
        assert_eq!(
            serde_json::to_string(&Corner::BottomRight).unwrap(),
            "\"bottom-right\""
        );
        assert_eq!(
            serde_json::from_str::<Corner>("\"top-left\"").unwrap(),
            Corner::TopLeft
        );
    }
}
