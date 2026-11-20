use serde::{Deserialize, Serialize};

const EDGE_MARGIN: f64 = 12.0;
const SNAP_DISTANCE: f64 = 16.0;
const PANEL_GAP: f64 = 8.0;
const PANEL_WIDTH: f64 = 380.0;
const PANEL_MIN_HEIGHT: f64 = 120.0;
const PANEL_MAX_HEIGHT: f64 = 640.0;
const PANEL_SCREEN_ALLOWANCE: f64 = 24.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn sized(left: i32, top: i32, width: i32, height: i32) -> Rect {
        Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    fn centre(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        (self.left..self.right).contains(&x) && (self.top..self.bottom).contains(&y)
    }

    fn overlap(&self, other: &Rect) -> i64 {
        let width = (self.right.min(other.right) - self.left.max(other.left)).max(0);
        let height = (self.bottom.min(other.bottom) - self.top.max(other.top)).max(0);
        i64::from(width) * i64::from(height)
    }

    fn moved_to(&self, left: i32, top: i32) -> Rect {
        Rect::sized(left, top, self.width(), self.height())
    }

    fn kept_inside(&self, area: &Rect) -> Rect {
        let left = self.left.min(area.right - self.width()).max(area.left);
        let top = self.top.min(area.bottom - self.height()).max(area.top);
        self.moved_to(left, top)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub bounds: Rect,
    pub work: Rect,
    pub scale: f64,
    pub primary: bool,
}

fn px(logical: f64, scale: f64) -> i32 {
    (logical * scale).round() as i32
}

pub fn layout_signature(monitors: &[Monitor]) -> String {
    let mut parts: Vec<String> = monitors
        .iter()
        .map(|monitor| {
            let Rect {
                left,
                top,
                right,
                bottom,
            } = monitor.bounds;
            format!(
                "{left},{top},{right},{bottom}@{}",
                (monitor.scale * 100.0).round()
            )
        })
        .collect();
    parts.sort();
    parts.join(";")
}

pub fn primary(monitors: &[Monitor]) -> Option<&Monitor> {
    monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or(monitors.first())
}

pub fn monitor_for<'a>(rect: &Rect, monitors: &'a [Monitor]) -> Option<&'a Monitor> {
    let (x, y) = rect.centre();
    monitors
        .iter()
        .find(|monitor| monitor.bounds.contains((x, y)))
        .or_else(|| {
            monitors
                .iter()
                .max_by_key(|monitor| monitor.work.overlap(rect))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub fn corner_rect(rect: Rect, corner: Corner, monitor: &Monitor) -> Rect {
    let (work, margin) = (monitor.work, px(EDGE_MARGIN, monitor.scale));
    let left = match corner {
        Corner::TopLeft | Corner::BottomLeft => work.left + margin,
        Corner::TopRight | Corner::BottomRight => work.right - margin - rect.width(),
    };
    let top = match corner {
        Corner::TopLeft | Corner::TopRight => work.top + margin,
        Corner::BottomLeft | Corner::BottomRight => work.bottom - margin - rect.height(),
    };
    rect.moved_to(left, top)
}

pub fn restored_pill_rect(
    saved: [Option<Rect>; 2],
    size: Rect,
    target: &Monitor,
    monitors: &[Monitor],
) -> Rect {
    saved
        .into_iter()
        .flatten()
        .find(|rect| monitor_for(rect, monitors).is_some_and(|monitor| monitor.id == target.id))
        .unwrap_or_else(|| corner_rect(size, Corner::TopRight, target))
        .kept_inside(&target.work)
}

pub fn snap(rect: Rect, work: &Rect, scale: f64) -> Rect {
    let (reach, margin) = (px(SNAP_DISTANCE, scale), px(EDGE_MARGIN, scale));
    let left = if rect.left - work.left <= reach {
        work.left + margin
    } else if work.right - rect.right <= reach {
        work.right - margin - rect.width()
    } else {
        rect.left
    };
    let top = if rect.top - work.top <= reach {
        work.top + margin
    } else if work.bottom - rect.bottom <= reach {
        work.bottom - margin - rect.height()
    } else {
        rect.top
    };
    rect.moved_to(left, top)
}

pub fn resize_anchored(rect: Rect, width: i32, height: i32, work: &Rect) -> Rect {
    let left = if rect.centre().0 > work.centre().0 {
        rect.right - width
    } else {
        rect.left
    };
    Rect::sized(left, rect.top, width, height)
}

pub fn place_panel(pill: Option<Rect>, monitor: &Monitor, content_height: f64) -> (Rect, f64) {
    let (work, scale) = (monitor.work, monitor.scale);
    let tallest = PANEL_MAX_HEIGHT.min(f64::from(work.height()) / scale - PANEL_SCREEN_ALLOWANCE);
    let height = px(
        content_height
            .min(tallest)
            .max(PANEL_MIN_HEIGHT.min(tallest)),
        scale,
    );
    let width = px(PANEL_WIDTH, scale);
    let gap = px(PANEL_GAP, scale);
    let (left, top) = match pill {
        Some(pill) => (
            if pill.centre().0 > work.centre().0 {
                pill.right - width
            } else {
                pill.left
            },
            if pill.centre().1 < work.centre().1 {
                pill.bottom + gap
            } else {
                pill.top - gap - height
            },
        ),
        None => {
            let margin = px(EDGE_MARGIN, scale);
            (work.right - margin - width, work.bottom - margin - height)
        }
    };
    (
        Rect::sized(left, top, width, height).kept_inside(&work),
        scale,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn monitor(id: &str, bounds: Rect, taskbar: i32, scale: f64, primary: bool) -> Monitor {
        Monitor {
            id: id.to_string(),
            name: id.to_string(),
            bounds,
            work: Rect {
                bottom: bounds.bottom - taskbar,
                ..bounds
            },
            scale,
            primary,
        }
    }

    fn desk() -> Vec<Monitor> {
        vec![
            monitor("wide", Rect::sized(1920, -200, 2560, 1440), 72, 1.5, false),
            monitor("main", Rect::sized(0, 0, 1920, 1080), 48, 1.0, true),
        ]
    }

    fn portrait_desk() -> Vec<Monitor> {
        vec![
            monitor("oled", Rect::sized(0, 0, 2560, 1440), 48, 1.0, true),
            monitor(
                "philips",
                Rect::sized(-1080, -300, 1080, 1920),
                48,
                1.0,
                false,
            ),
        ]
    }

    #[test]
    fn the_signature_sorts_monitors_and_includes_their_scale() {
        let mut reversed = desk();
        reversed.reverse();
        assert_eq!(layout_signature(&desk()), layout_signature(&reversed));
        assert_eq!(
            layout_signature(&desk()),
            "0,0,1920,1080@100;1920,-200,4480,1240@150"
        );
        let mut rescaled = desk();
        rescaled[0].scale = 1.25;
        assert_ne!(layout_signature(&desk()), layout_signature(&rescaled));
    }

    #[test]
    fn the_default_is_the_top_right_of_the_work_area() {
        assert_eq!(
            corner_rect(Rect::sized(0, 0, 100, 28), Corner::TopRight, &desk()[1]),
            Rect::sized(1920 - 12 - 100, 12, 100, 28)
        );
    }

    #[test]
    fn corners_sit_inside_a_portrait_work_area_left_of_the_primary() {
        let philips = &portrait_desk()[1];
        let corner = |corner| corner_rect(Rect::sized(500, 500, 100, 28), corner, philips);
        assert_eq!(corner(Corner::TopLeft), Rect::sized(-1068, -288, 100, 28));
        assert_eq!(corner(Corner::TopRight), Rect::sized(-112, -288, 100, 28));
        assert_eq!(
            corner(Corner::BottomLeft),
            Rect::sized(-1068, 1532, 100, 28)
        );
        assert_eq!(
            corner(Corner::BottomRight),
            Rect::sized(-112, 1532, 100, 28)
        );
    }

    #[test]
    fn restoring_prefers_the_target_displays_saved_rect() {
        let monitors = portrait_desk();
        let (oled, philips) = (&monitors[0], &monitors[1]);
        let size = Rect::sized(0, 0, 100, 28);
        let on_philips = Rect::sized(-500, 400, 100, 28);
        let on_oled = Rect::sized(2000, 12, 100, 28);
        let layout_on_oled = Rect::sized(1500, 900, 100, 28);
        assert_eq!(
            restored_pill_rect(
                [Some(on_philips), Some(layout_on_oled)],
                size,
                philips,
                &monitors
            ),
            on_philips
        );
        assert_eq!(
            restored_pill_rect([Some(on_oled), Some(layout_on_oled)], size, oled, &monitors),
            on_oled
        );
        assert_eq!(
            restored_pill_rect([None, Some(layout_on_oled)], size, oled, &monitors),
            layout_on_oled
        );
        assert_eq!(
            restored_pill_rect([None, Some(layout_on_oled)], size, philips, &monitors),
            Rect::sized(-112, -288, 100, 28)
        );
        assert_eq!(
            restored_pill_rect([Some(on_oled), None], size, philips, &monitors),
            Rect::sized(-112, -288, 100, 28)
        );
        assert_eq!(
            restored_pill_rect(
                [Some(Rect::sized(-60, 400, 100, 28)), None],
                size,
                philips,
                &monitors
            ),
            Rect::sized(-100, 400, 100, 28)
        );
    }

    #[test]
    fn snapping_moves_near_edges_to_the_margin_and_leaves_the_middle_alone() {
        let work = Rect::sized(0, 0, 1920, 1032);
        let pill = |left, top| Rect::sized(left, top, 100, 28);
        assert_eq!(snap(pill(15, 500), &work, 1.0), pill(12, 500));
        assert_eq!(snap(pill(-40, 500), &work, 1.0), pill(12, 500));
        assert_eq!(snap(pill(17, 500), &work, 1.0), pill(17, 500));
        assert_eq!(
            snap(pill(1920 - 100 - 10, 3), &work, 1.0),
            pill(1920 - 100 - 12, 12)
        );
        assert_eq!(
            snap(pill(800, 1032 - 28 - 16), &work, 1.0),
            pill(800, 1032 - 28 - 12)
        );
        assert_eq!(snap(pill(800, 500), &work, 1.0), pill(800, 500));
        assert_eq!(snap(pill(20, 20), &work, 1.5), pill(18, 18));
        assert_eq!(snap(pill(20, 20), &work, 1.0), pill(20, 20));
    }

    #[test]
    fn resizing_keeps_the_edge_nearer_the_screen_edge() {
        let work = Rect::sized(0, 0, 1920, 1032);
        assert_eq!(
            resize_anchored(Rect::sized(1800, 12, 100, 28), 136, 28, &work),
            Rect::sized(1764, 12, 136, 28)
        );
        assert_eq!(
            resize_anchored(Rect::sized(12, 12, 100, 28), 136, 28, &work),
            Rect::sized(12, 12, 136, 28)
        );
    }

    #[test]
    fn the_panel_opens_beside_the_pill_in_every_quadrant() {
        let main = monitor("main", Rect::sized(0, 0, 1920, 1080), 48, 1.0, true);
        let place = |pill: Rect| place_panel(Some(pill), &main, 300.0).0;
        assert_eq!(
            place(Rect::sized(1808, 12, 100, 28)),
            Rect::sized(1528, 48, 380, 300)
        );
        assert_eq!(
            place(Rect::sized(12, 12, 100, 28)),
            Rect::sized(12, 48, 380, 300)
        );
        assert_eq!(
            place(Rect::sized(12, 992, 100, 28)),
            Rect::sized(12, 684, 380, 300)
        );
        assert_eq!(
            place(Rect::sized(1808, 992, 100, 28)),
            Rect::sized(1528, 684, 380, 300)
        );
    }

    #[test]
    fn the_panel_scales_clamps_its_height_and_stays_inside_the_work_area() {
        let main = monitor("main", Rect::sized(0, 0, 1920, 1080), 48, 1.5, true);
        let (panel, scale) = place_panel(Some(Rect::sized(1800, 18, 100, 42)), &main, 40.0);
        assert_eq!(scale, 1.5);
        assert_eq!(panel, Rect::sized(1330, 72, 570, 180));
        let (tall, _) = place_panel(Some(Rect::sized(0, 18, 100, 42)), &main, 2000.0);
        assert_eq!(tall.height(), 960);
        let (short, _) = place_panel(
            Some(Rect::sized(0, 18, 100, 42)),
            &monitor("small", Rect::sized(0, 0, 800, 400), 0, 1.0, true),
            2000.0,
        );
        assert_eq!(short.height(), 376);
        assert_eq!(short.top, 24);
    }

    #[test]
    fn without_a_pill_the_panel_sits_bottom_right_of_the_work_area() {
        let (panel, scale) = place_panel(None, &desk()[1], 200.0);
        assert_eq!(scale, 1.0);
        assert_eq!(
            panel,
            Rect::sized(1920 - 12 - 380, 1032 - 12 - 200, 380, 200)
        );
    }

    #[test]
    fn without_a_pill_the_panel_sits_bottom_right_of_a_portrait_work_area() {
        let (panel, _) = place_panel(None, &portrait_desk()[1], 200.0);
        assert_eq!(panel, Rect::sized(-12 - 380, 1572 - 12 - 200, 380, 200));
    }
}
