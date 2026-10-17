use serde::{Deserialize, Serialize};

pub const MARGIN_PX: i32 = 16;
pub const COMPACT_LOGICAL: (f64, f64) = (380.0, 200.0);
pub const EXPANDED_LOGICAL: (f64, f64) = (460.0, 380.0);
pub const MIN_LOGICAL: (f64, f64) = (180.0, 90.0);
const MAX_COORD: i32 = 1_000_000;
const MAX_SIDE: i32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height)
    }

    fn is_sane(&self) -> bool {
        self.x.abs() <= MAX_COORD
            && self.y.abs() <= MAX_COORD
            && (1..=MAX_SIDE).contains(&self.width)
            && (1..=MAX_SIDE).contains(&self.height)
    }

    fn contains_center_of(&self, other: &Rect) -> bool {
        let cx2 = 2 * i64::from(other.x) + i64::from(other.width);
        let cy2 = 2 * i64::from(other.y) + i64::from(other.height);
        cx2 >= 2 * i64::from(self.x)
            && cx2 < 2 * i64::from(self.right())
            && cy2 >= 2 * i64::from(self.y)
            && cy2 < 2 * i64::from(self.bottom())
    }

    fn intersection_area(&self, other: &Rect) -> i64 {
        let w = i64::from(self.right().min(other.right())) - i64::from(self.x.max(other.x));
        let h = i64::from(self.bottom().min(other.bottom())) - i64::from(self.y.max(other.y));
        if w > 0 && h > 0 {
            w * h
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SavedSize {
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub scale: Option<f64>,
}

impl SavedSize {
    fn is_sane(&self) -> bool {
        (1..=MAX_SIDE).contains(&self.width)
            && (1..=MAX_SIDE).contains(&self.height)
            && self.scale.is_none_or(|s| s.is_finite() && s > 0.0)
    }

    fn at_scale(&self, scale: f64) -> Size {
        match self.scale {
            Some(saved) if saved > 0.0 && scale > 0.0 && saved != scale => Size {
                width: (f64::from(self.width) * scale / saved).round() as i32,
                height: (f64::from(self.height) * scale / saved).round() as i32,
            },
            _ => Size {
                width: self.width,
                height: self.height,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub visible: bool,
    pub expanded: bool,
    pub hide_translation: bool,
    pub corner: Corner,
    pub position: Option<Rect>,
    pub compact_size: Option<SavedSize>,
    pub expanded_size: Option<SavedSize>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            visible: true,
            expanded: false,
            hide_translation: false,
            corner: Corner::default(),
            position: None,
            compact_size: None,
            expanded_size: None,
        }
    }
}

impl Layout {
    pub fn remember(&mut self, rect: Rect, expanded: bool, scale: f64) {
        self.position = Some(rect);
        let size = Some(SavedSize {
            width: rect.width,
            height: rect.height,
            scale: (scale.is_finite() && scale > 0.0).then_some(scale),
        });
        if expanded {
            self.expanded_size = size;
        } else {
            self.compact_size = size;
        }
    }

    pub fn sanitize(&mut self) -> bool {
        let mut dropped = false;
        if self.position.is_some_and(|r| !r.is_sane()) {
            self.position = None;
            dropped = true;
        }
        for size in [&mut self.compact_size, &mut self.expanded_size] {
            if size.is_some_and(|s| !s.is_sane()) {
                *size = None;
                dropped = true;
            }
        }
        dropped
    }

    pub fn reset_position(&mut self) {
        self.position = None;
        self.compact_size = None;
        self.expanded_size = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorArea {
    pub work_area: Rect,
    pub scale: f64,
}

fn to_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub fn clamp_into(work: Rect, rect: Rect) -> Rect {
    if work.width <= 0 || work.height <= 0 {
        return work;
    }
    let (wx, wy) = (i64::from(work.x), i64::from(work.y));
    let (ww, wh) = (i64::from(work.width), i64::from(work.height));
    let width = i64::from(rect.width).clamp(1, ww);
    let height = i64::from(rect.height).clamp(1, wh);
    Rect {
        x: to_i32(i64::from(rect.x).clamp(wx, wx + ww - width)),
        y: to_i32(i64::from(rect.y).clamp(wy, wy + wh - height)),
        width: to_i32(width),
        height: to_i32(height),
    }
}

pub fn resize_anchored(work: Rect, current: Rect, size: Size) -> Rect {
    let keep_right = 2 * i64::from(current.x) + i64::from(current.width)
        > 2 * i64::from(work.x) + i64::from(work.width);
    let keep_bottom = 2 * i64::from(current.y) + i64::from(current.height)
        > 2 * i64::from(work.y) + i64::from(work.height);
    let desired = Rect {
        x: if keep_right {
            current.right().saturating_sub(size.width)
        } else {
            current.x
        },
        y: if keep_bottom {
            current.bottom().saturating_sub(size.height)
        } else {
            current.y
        },
        width: size.width,
        height: size.height,
    };
    clamp_into(work, desired)
}

pub fn corner_rect(work: Rect, size: Size, corner: Corner, margin: i32) -> Rect {
    let left = work.x.saturating_add(margin);
    let top = work.y.saturating_add(margin);
    let right = work
        .right()
        .saturating_sub(margin)
        .saturating_sub(size.width);
    let bottom = work
        .bottom()
        .saturating_sub(margin)
        .saturating_sub(size.height);
    let (x, y) = match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    };
    clamp_into(
        work,
        Rect {
            x,
            y,
            width: size.width,
            height: size.height,
        },
    )
}

pub fn pick_monitor(monitors: &[MonitorArea], rect: Rect) -> Option<usize> {
    if let Some(i) = monitors
        .iter()
        .position(|m| m.work_area.contains_center_of(&rect))
    {
        return Some(i);
    }
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| (i, m.work_area.intersection_area(&rect)))
        .filter(|(_, area)| *area > 0)
        .max_by_key(|(_, area)| *area)
        .map(|(i, _)| i)
}

fn scaled(logical: (f64, f64), scale: f64) -> Size {
    Size {
        width: (logical.0 * scale).round() as i32,
        height: (logical.1 * scale).round() as i32,
    }
}

pub fn mode_size(layout: &Layout, scale: f64) -> Size {
    let (saved, default) = if layout.expanded {
        (layout.expanded_size, EXPANDED_LOGICAL)
    } else {
        (layout.compact_size, COMPACT_LOGICAL)
    };
    let min = scaled(MIN_LOGICAL, scale);
    let size = saved.map_or_else(|| scaled(default, scale), |s| s.at_scale(scale));
    Size {
        width: size.width.max(min.width),
        height: size.height.max(min.height),
    }
}

pub fn target_rect(
    monitors: &[MonitorArea],
    primary: usize,
    layout: &Layout,
    current: Option<Rect>,
) -> Option<Rect> {
    let fallback = *monitors.get(primary).or_else(|| monitors.first())?;
    match current.or(layout.position) {
        Some(base) => {
            let monitor = pick_monitor(monitors, base).map_or(fallback, |i| monitors[i]);
            Some(resize_anchored(
                monitor.work_area,
                base,
                mode_size(layout, monitor.scale),
            ))
        }
        None => Some(corner_rect(
            fallback.work_area,
            mode_size(layout, fallback.scale),
            layout.corner,
            (f64::from(MARGIN_PX) * fallback.scale).round() as i32,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn size(width: i32, height: i32) -> Size {
        Size { width, height }
    }

    fn monitor(work_area: Rect, scale: f64) -> MonitorArea {
        MonitorArea { work_area, scale }
    }

    #[test]
    fn clamp_keeps_inside_rect_unchanged() {
        let r = rect(100, 100, 380, 200);
        assert_eq!(clamp_into(WORK, r), r);
    }

    #[test]
    fn clamp_pulls_rect_back_from_every_edge() {
        assert_eq!(
            clamp_into(WORK, rect(-50, -20, 380, 200)),
            rect(0, 0, 380, 200)
        );
        assert_eq!(
            clamp_into(WORK, rect(1800, 1000, 380, 200)),
            rect(1540, 840, 380, 200)
        );
    }

    #[test]
    fn clamp_shrinks_rect_larger_than_work_area() {
        assert_eq!(
            clamp_into(WORK, rect(-10, -10, 4000, 3000)),
            rect(0, 0, 1920, 1040)
        );
    }

    #[test]
    fn clamp_respects_work_area_offset() {
        let work = rect(-1920, 40, 1920, 1040);
        assert_eq!(
            clamp_into(work, rect(10, 0, 380, 200)),
            rect(-380, 40, 380, 200)
        );
    }

    #[test]
    fn grow_in_bottom_right_keeps_bottom_right_edges() {
        let current = rect(1524, 824, 380, 200);
        let grown = resize_anchored(WORK, current, size(460, 380));
        assert_eq!(grown.right(), current.right());
        assert_eq!(grown.bottom(), current.bottom());
        assert_eq!(grown, rect(1444, 644, 460, 380));
    }

    #[test]
    fn grow_in_top_left_keeps_top_left_edges() {
        let current = rect(16, 16, 380, 200);
        assert_eq!(
            resize_anchored(WORK, current, size(460, 380)),
            rect(16, 16, 460, 380)
        );
    }

    #[test]
    fn grow_in_top_right_keeps_top_and_right_edges() {
        let current = rect(1524, 16, 380, 200);
        assert_eq!(
            resize_anchored(WORK, current, size(460, 380)),
            rect(1444, 16, 460, 380)
        );
    }

    #[test]
    fn shrink_in_bottom_left_keeps_bottom_and_left_edges() {
        let current = rect(16, 644, 460, 380);
        assert_eq!(
            resize_anchored(WORK, current, size(380, 200)),
            rect(16, 824, 380, 200)
        );
    }

    #[test]
    fn compact_expanded_roundtrip_returns_to_same_rect() {
        let compact = rect(1524, 824, 380, 200);
        let expanded = resize_anchored(WORK, compact, size(460, 380));
        assert_eq!(resize_anchored(WORK, expanded, size(380, 200)), compact);
    }

    #[test]
    fn grow_near_edge_is_clamped_into_work_area() {
        let current = rect(10, 900, 380, 100);
        let grown = resize_anchored(WORK, current, size(460, 380));
        assert_eq!(grown.bottom(), 1000);
        assert!(grown.y >= WORK.y && grown.right() <= WORK.right());

        let top = rect(0, 400, 380, 100);
        let grown = resize_anchored(WORK, top, size(460, 900));
        assert_eq!(grown, rect(0, 140, 460, 900));
    }

    #[test]
    fn corner_rect_places_with_margin() {
        let s = size(380, 200);
        assert_eq!(
            corner_rect(WORK, s, Corner::BottomRight, 16),
            rect(1524, 824, 380, 200)
        );
        assert_eq!(
            corner_rect(WORK, s, Corner::TopLeft, 16),
            rect(16, 16, 380, 200)
        );
        assert_eq!(
            corner_rect(WORK, s, Corner::TopRight, 16),
            rect(1524, 16, 380, 200)
        );
        assert_eq!(
            corner_rect(WORK, s, Corner::BottomLeft, 16),
            rect(16, 824, 380, 200)
        );
    }

    #[test]
    fn pick_monitor_by_center_then_by_overlap() {
        let monitors = [monitor(WORK, 1.0), monitor(rect(1920, 0, 2560, 1400), 1.5)];
        assert_eq!(pick_monitor(&monitors, rect(2000, 100, 380, 200)), Some(1));
        assert_eq!(pick_monitor(&monitors, rect(1800, 100, 380, 200)), Some(1));
        assert_eq!(pick_monitor(&monitors, rect(1700, 100, 380, 200)), Some(0));
        assert_eq!(pick_monitor(&monitors, rect(9000, 9000, 380, 200)), None);
    }

    #[test]
    fn startup_without_saved_position_uses_corner_and_scale() {
        let monitors = [monitor(WORK, 1.5)];
        let layout = Layout {
            corner: Corner::TopLeft,
            ..Layout::default()
        };
        assert_eq!(
            target_rect(&monitors, 0, &layout, None),
            Some(rect(24, 24, 570, 300))
        );
    }

    #[test]
    fn startup_restores_saved_position_clamped_after_resolution_change() {
        let monitors = [monitor(rect(0, 0, 1280, 680), 1.0)];
        let mut layout = Layout::default();
        layout.remember(rect(1524, 824, 380, 200), false, 1.0);
        assert_eq!(
            target_rect(&monitors, 0, &layout, None),
            Some(rect(900, 480, 380, 200))
        );
    }

    #[test]
    fn saved_position_on_missing_monitor_is_clamped_into_primary() {
        let monitors = [monitor(WORK, 1.0), monitor(rect(1920, 0, 1920, 1040), 1.0)];
        let mut layout = Layout::default();
        layout.remember(rect(-1500, 300, 380, 200), false, 1.0);
        assert_eq!(
            target_rect(&monitors, 1, &layout, None),
            Some(rect(1920, 300, 380, 200))
        );
    }

    #[test]
    fn toggle_to_expanded_uses_saved_expanded_size_and_anchor() {
        let monitors = [monitor(WORK, 1.0)];
        let mut layout = Layout::default();
        layout.remember(rect(1000, 600, 500, 400), true, 1.0);
        layout.remember(rect(1524, 824, 380, 200), false, 1.0);
        layout.expanded = true;
        assert_eq!(
            target_rect(&monitors, 0, &layout, Some(rect(1524, 824, 380, 200))),
            Some(rect(1404, 624, 500, 400))
        );
    }

    #[test]
    fn tiny_saved_size_is_raised_to_minimum() {
        let layout = Layout {
            compact_size: Some(SavedSize {
                width: 10,
                height: 10,
                scale: None,
            }),
            ..Layout::default()
        };
        assert_eq!(mode_size(&layout, 2.0), size(360, 180));
    }

    #[test]
    fn no_monitors_gives_none() {
        assert_eq!(target_rect(&[], 0, &Layout::default(), None), None);
    }

    #[test]
    fn reset_clears_rects_but_keeps_flags() {
        let mut layout = Layout {
            visible: false,
            expanded: true,
            corner: Corner::TopRight,
            ..Layout::default()
        };
        layout.remember(rect(1, 2, 300, 200), true, 1.0);
        layout.reset_position();
        assert_eq!(layout.position, None);
        assert_eq!(layout.expanded_size, None);
        assert!(!layout.visible && layout.expanded);
        assert_eq!(layout.corner, Corner::TopRight);
    }

    #[test]
    fn old_or_partial_layout_json_gets_defaults() {
        let layout: Layout = serde_json::from_str(r#"{"corner":"top_left"}"#).unwrap();
        assert!(layout.visible);
        assert!(!layout.expanded);
        assert!(!layout.hide_translation);
        assert_eq!(layout.corner, Corner::TopLeft);
        assert_eq!(layout.position, None);
    }

    #[test]
    fn degenerate_work_area_does_not_panic() {
        let empty = rect(0, 0, 0, 0);
        assert_eq!(clamp_into(empty, rect(10, 10, 380, 200)), empty);
        assert_eq!(
            clamp_into(rect(0, 0, -5, 100), rect(1, 1, 2, 2)),
            rect(0, 0, -5, 100)
        );
        let monitors = [monitor(empty, 1.0)];
        assert_eq!(
            target_rect(&monitors, 0, &Layout::default(), None),
            Some(empty)
        );
    }

    #[test]
    fn extreme_values_do_not_overflow() {
        let huge = rect(i32::MAX - 10, i32::MAX - 10, i32::MAX, i32::MAX);
        let r = clamp_into(WORK, huge);
        assert_eq!(r, rect(0, 0, 1920, 1040));
        let r = resize_anchored(WORK, huge, size(460, 380));
        assert!(r.x >= 0 && r.right() <= 1920 && r.bottom() <= 1040);
        let far = rect(i32::MIN, i32::MIN, 380, 200);
        assert_eq!(clamp_into(WORK, far), rect(0, 0, 380, 200));
        let edge_work = rect(i32::MAX - 100, 0, 1000, 1000);
        let r = clamp_into(edge_work, rect(0, 0, 50, 50));
        assert_eq!(r.x, i32::MAX - 100);
        let r = corner_rect(edge_work, size(50, 50), Corner::BottomRight, 16);
        assert!(r.width == 50 && r.x >= edge_work.x);
    }

    #[test]
    fn sanitize_drops_absurd_saved_values() {
        let mut layout = Layout {
            position: Some(rect(5_000_000, 0, 380, 200)),
            compact_size: Some(SavedSize {
                width: 0,
                height: 200,
                scale: None,
            }),
            expanded_size: Some(SavedSize {
                width: 460,
                height: 380,
                scale: Some(f64::NAN),
            }),
            ..Layout::default()
        };
        assert!(layout.sanitize());
        assert_eq!(layout.position, None);
        assert_eq!(layout.compact_size, None);
        assert_eq!(layout.expanded_size, None);

        let mut good = Layout::default();
        good.remember(rect(100, 100, 380, 200), false, 1.25);
        assert!(!good.sanitize());
        assert!(good.position.is_some() && good.compact_size.is_some());
    }

    #[test]
    fn saved_size_is_rescaled_to_monitor_scale() {
        let mut layout = Layout::default();
        layout.remember(rect(100, 100, 760, 400), false, 2.0);
        assert_eq!(mode_size(&layout, 1.0), size(380, 200));
        assert_eq!(mode_size(&layout, 1.5), size(570, 300));
        assert_eq!(mode_size(&layout, 2.0), size(760, 400));
    }

    #[test]
    fn saved_size_without_scale_is_used_as_is() {
        let layout: Layout =
            serde_json::from_str(r#"{"compact_size":{"width":500,"height":250}}"#).unwrap();
        assert_eq!(layout.compact_size.unwrap().scale, None);
        assert_eq!(mode_size(&layout, 2.0), size(500, 250));
    }
}
