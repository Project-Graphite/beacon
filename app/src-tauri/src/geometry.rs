use serde::{Deserialize, Serialize};

const EDGE_MARGIN: f64 = 12.0;
const SNAP_DISTANCE: f64 = 16.0;
const PANEL_GAP: f64 = 8.0;
const PANEL_WIDTH: f64 = 376.0;
const PANEL_MIN_HEIGHT: f64 = 120.0;
const PANEL_SCREEN_ALLOWANCE: f64 = 24.0;
const DOCK_DISTANCE: f64 = 80.0;
const PEEK_GAP: f64 = 6.0;
const REVEAL_DEPTH: f64 = 4.0;
const EDGE_ORDER: [Edge; 4] = [Edge::Right, Edge::Left, Edge::Top, Edge::Bottom];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TabPlacement {
    pub edge: Edge,
    pub offset: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressTarget {
    Panel,
    Anchor,
    Outside,
}

impl Edge {
    fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

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

    pub fn inflated(&self, by: i32) -> Rect {
        Rect {
            left: self.left - by,
            top: self.top - by,
            right: self.right + by,
            bottom: self.bottom + by,
        }
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

fn panel_size(monitor: &Monitor, content_height: f64) -> (i32, i32) {
    let (work, scale) = (monitor.work, monitor.scale);
    let tallest = f64::from(work.height()) / scale - PANEL_SCREEN_ALLOWANCE;
    (
        px(PANEL_WIDTH, scale),
        px(
            content_height
                .min(tallest)
                .max(PANEL_MIN_HEIGHT.min(tallest)),
            scale,
        ),
    )
}

pub fn place_panel(pill: Option<Rect>, monitor: &Monitor, content_height: f64) -> (Rect, f64) {
    let (work, scale) = (monitor.work, monitor.scale);
    let (width, height) = panel_size(monitor, content_height);
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

pub fn pill_edge(pill: Rect, work: &Rect) -> Edge {
    if pill.centre().1 < work.centre().1 {
        Edge::Top
    } else {
        Edge::Bottom
    }
}

pub fn reveal_strip(pill: Rect, work: &Rect, scale: f64) -> Rect {
    let depth = px(REVEAL_DEPTH, scale).clamp(1, pill.height());
    match pill_edge(pill, work) {
        Edge::Top => Rect::sized(pill.left, pill.top, pill.width(), depth),
        _ => Rect::sized(pill.left, pill.bottom - depth, pill.width(), depth),
    }
}

pub fn nearest_corner(pill: Rect, work: &Rect) -> Corner {
    let ((x, y), (middle_x, middle_y)) = (pill.centre(), work.centre());
    match (y < middle_y, x < middle_x) {
        (true, true) => Corner::TopLeft,
        (true, false) => Corner::TopRight,
        (false, true) => Corner::BottomLeft,
        (false, false) => Corner::BottomRight,
    }
}

pub fn place_panel_on_edge(
    tab: Rect,
    edge: Edge,
    monitor: &Monitor,
    content_height: f64,
) -> (Rect, f64) {
    let (bounds, scale) = (monitor.bounds, monitor.scale);
    let (width, height) = panel_size(monitor, content_height);
    let gap = px(PANEL_GAP, scale);
    let (x, y) = tab.centre();
    let (left, top) = match edge {
        Edge::Left => (bounds.left + gap, y - height / 2),
        Edge::Right => (bounds.right - gap - width, y - height / 2),
        Edge::Top => (x - width / 2, bounds.top + gap),
        Edge::Bottom => (x - width / 2, bounds.bottom - gap - height),
    };
    (
        Rect::sized(left, top, width, height).kept_inside(&monitor.work),
        scale,
    )
}

fn span(bounds: &Rect, edge: Edge) -> (i32, i32) {
    if edge.vertical() {
        (bounds.top, bounds.bottom)
    } else {
        (bounds.left, bounds.right)
    }
}

pub fn tab_rect(
    bounds: &Rect,
    placement: TabPlacement,
    length: f64,
    depth: f64,
    scale: f64,
) -> Rect {
    let (length, depth) = (px(length, scale), px(depth, scale));
    let (start, end) = span(bounds, placement.edge);
    let centre = start + (f64::from(end - start) * placement.offset.clamp(0.0, 1.0)).round() as i32;
    let along = (centre - length / 2).clamp(start, (end - length).max(start));
    match placement.edge {
        Edge::Left => Rect::sized(bounds.left, along, depth, length),
        Edge::Right => Rect::sized(bounds.right - depth, along, depth, length),
        Edge::Top => Rect::sized(along, bounds.top, length, depth),
        Edge::Bottom => Rect::sized(along, bounds.bottom - depth, length, depth),
    }
}

pub fn peek_rect(
    anchor: Rect,
    edge: Edge,
    (width, height): (i32, i32),
    tucked: f64,
    area: &Rect,
    scale: f64,
) -> Rect {
    let (gap, tucked) = (px(PEEK_GAP, scale), px(tucked, scale));
    let (x, y) = anchor.centre();
    let (left, top) = match edge {
        Edge::Left => (anchor.right + gap - tucked, y - height / 2),
        Edge::Right => (anchor.left - gap - width + tucked, y - height / 2),
        Edge::Top => (x - width / 2, anchor.bottom + gap - tucked),
        Edge::Bottom => (x - width / 2, anchor.top - gap - height + tucked),
    };
    Rect::sized(left, top, width, height).kept_inside(area)
}

fn shares_segment(from: i32, to: i32, other_from: i32, other_to: i32) -> bool {
    to.min(other_to) - from.max(other_from) > 0
}

fn touches(bounds: &Rect, edge: Edge, other: &Rect) -> bool {
    match edge {
        Edge::Left => {
            other.right == bounds.left
                && shares_segment(bounds.top, bounds.bottom, other.top, other.bottom)
        }
        Edge::Right => {
            other.left == bounds.right
                && shares_segment(bounds.top, bounds.bottom, other.top, other.bottom)
        }
        Edge::Top => {
            other.bottom == bounds.top
                && shares_segment(bounds.left, bounds.right, other.left, other.right)
        }
        Edge::Bottom => {
            other.top == bounds.bottom
                && shares_segment(bounds.left, bounds.right, other.left, other.right)
        }
    }
}

pub fn default_placement(monitor: &Monitor, monitors: &[Monitor]) -> TabPlacement {
    let edge = EDGE_ORDER
        .into_iter()
        .find(|edge| {
            !monitors
                .iter()
                .filter(|other| other.id != monitor.id)
                .any(|other| touches(&monitor.bounds, *edge, &other.bounds))
        })
        .unwrap_or(Edge::Right);
    TabPlacement { edge, offset: 0.5 }
}

fn edge_distance(bounds: &Rect, edge: Edge, (x, y): (i32, i32)) -> i32 {
    match edge {
        Edge::Left => (x - bounds.left).abs(),
        Edge::Right => (bounds.right - x).abs(),
        Edge::Top => (y - bounds.top).abs(),
        Edge::Bottom => (bounds.bottom - y).abs(),
    }
}

pub fn near_edge(monitor: &Monitor, edge: Edge, point: (i32, i32)) -> bool {
    edge_distance(&monitor.bounds, edge, point) <= px(DOCK_DISTANCE, monitor.scale)
}

pub fn nearest_edge(bounds: &Rect, point: (i32, i32)) -> Edge {
    EDGE_ORDER.into_iter().fold(Edge::Right, |nearest, edge| {
        if edge_distance(bounds, edge, point) < edge_distance(bounds, nearest, point) {
            edge
        } else {
            nearest
        }
    })
}

pub fn slide(bounds: &Rect, edge: Edge, (x, y): (i32, i32)) -> TabPlacement {
    let (start, end) = span(bounds, edge);
    let along = if edge.vertical() { y } else { x };
    TabPlacement {
        edge,
        offset: (f64::from(along - start) / f64::from(end - start)).clamp(0.0, 1.0),
    }
}

pub fn press_target(
    point: (i32, i32),
    panel: Option<Rect>,
    anchors: &[Option<Rect>],
) -> PressTarget {
    if panel.is_some_and(|panel| panel.contains(point)) {
        PressTarget::Panel
    } else if anchors
        .iter()
        .flatten()
        .any(|anchor| anchor.contains(point))
    {
        PressTarget::Anchor
    } else {
        PressTarget::Outside
    }
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
            Rect::sized(1532, 48, 376, 300)
        );
        assert_eq!(
            place(Rect::sized(12, 12, 100, 28)),
            Rect::sized(12, 48, 376, 300)
        );
        assert_eq!(
            place(Rect::sized(12, 992, 100, 28)),
            Rect::sized(12, 684, 376, 300)
        );
        assert_eq!(
            place(Rect::sized(1808, 992, 100, 28)),
            Rect::sized(1532, 684, 376, 300)
        );
    }

    #[test]
    fn a_dragged_pill_flips_the_panel_across_the_middle_and_keeps_it_in_the_work_area() {
        let bar = monitor("bar", Rect::sized(-3000, 0, 1920, 464), 0, 1.0, false);
        let place =
            |left: i32, top: i32| place_panel(Some(Rect::sized(left, top, 46, 28)), &bar, 120.0).0;
        assert_eq!(place(-2500, 200), Rect::sized(-2500, 236, 376, 120));
        assert_eq!(place(-2500, 220), Rect::sized(-2500, 92, 376, 120));
        assert_eq!(place(-1500, 200), Rect::sized(-1830, 236, 376, 120));
        assert_eq!(place(-3000, 0), Rect::sized(-3000, 36, 376, 120));
        assert_eq!(place(-1126, 0), Rect::sized(-1456, 36, 376, 120));
        assert_eq!(place(-3000, 436), Rect::sized(-3000, 308, 376, 120));
        assert_eq!(place(-1126, 436), Rect::sized(-1456, 308, 376, 120));
        assert_eq!(place(-3020, 200), Rect::sized(-3000, 236, 376, 120));
        assert_eq!(place(-1060, 200), Rect::sized(-1456, 236, 376, 120));
        assert_eq!(place(-2500, 420), Rect::sized(-2500, 292, 376, 120));
        let tall = place_panel(Some(Rect::sized(-2500, 200, 46, 28)), &bar, 2000.0).0;
        assert_eq!(tall, Rect::sized(-2500, 24, 376, 440));
    }

    #[test]
    fn the_panel_scales_clamps_its_height_and_stays_inside_the_work_area() {
        let main = monitor("main", Rect::sized(0, 0, 1920, 1080), 48, 1.5, true);
        let (panel, scale) = place_panel(Some(Rect::sized(1800, 18, 100, 42)), &main, 40.0);
        assert_eq!(scale, 1.5);
        assert_eq!(panel, Rect::sized(1336, 72, 564, 180));
        let (tall, _) = place_panel(Some(Rect::sized(0, 18, 100, 42)), &main, 2000.0);
        assert_eq!(tall.height(), 996);
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
            Rect::sized(1920 - 12 - 376, 1032 - 12 - 200, 376, 200)
        );
    }

    #[test]
    fn without_a_pill_the_panel_sits_bottom_right_of_a_portrait_work_area() {
        let (panel, _) = place_panel(None, &portrait_desk()[1], 200.0);
        assert_eq!(panel, Rect::sized(-12 - 376, 1572 - 12 - 200, 376, 200));
    }

    pub fn owner_desk() -> Vec<Monitor> {
        vec![
            monitor("oled", Rect::sized(0, 0, 2560, 1440), 48, 1.0, true),
            monitor("philips", Rect::sized(-1080, 0, 1080, 1920), 48, 1.0, false),
            monitor("bar", Rect::sized(-3000, 0, 1920, 464), 0, 1.0, false),
        ]
    }

    fn at(edge: Edge, offset: f64) -> TabPlacement {
        TabPlacement { edge, offset }
    }

    #[test]
    fn a_span_shorter_than_the_tab_starts_the_tab_at_its_origin() {
        let empty = Rect::sized(0, 0, 0, 0);
        for edge in [Edge::Left, Edge::Right] {
            assert_eq!(tab_rect(&empty, at(edge, 0.5), 72.0, 4.0, 1.0).top, 0);
        }
        for edge in [Edge::Top, Edge::Bottom] {
            assert_eq!(tab_rect(&empty, at(edge, 0.5), 72.0, 4.0, 1.0).left, 0);
        }
    }

    #[test]
    fn the_default_edge_is_the_first_free_one_in_right_left_top_bottom_order() {
        let monitors = owner_desk();
        let edges: Vec<Edge> = monitors
            .iter()
            .map(|monitor| default_placement(monitor, &monitors).edge)
            .collect();
        assert_eq!(edges, [Edge::Right, Edge::Top, Edge::Left]);
        assert_eq!(default_placement(&monitors[0], &monitors).offset, 0.5);
        let boxed_in = vec![
            monitor("middle", Rect::sized(0, 0, 100, 100), 0, 1.0, true),
            monitor("left", Rect::sized(-100, 0, 100, 100), 0, 1.0, false),
            monitor("right", Rect::sized(100, 0, 100, 100), 0, 1.0, false),
            monitor("above", Rect::sized(0, -100, 100, 100), 0, 1.0, false),
            monitor("below", Rect::sized(0, 100, 100, 100), 0, 1.0, false),
        ];
        assert_eq!(default_placement(&boxed_in[0], &boxed_in).edge, Edge::Right);
        let corner_only = vec![
            monitor("a", Rect::sized(0, 0, 100, 100), 0, 1.0, true),
            monitor("beside", Rect::sized(100, 0, 100, 100), 0, 1.0, false),
            monitor("diagonal", Rect::sized(-100, 100, 100, 100), 0, 1.0, false),
        ];
        assert_eq!(
            default_placement(&corner_only[0], &corner_only).edge,
            Edge::Left
        );
    }

    #[test]
    fn the_tab_and_its_hover_band_sit_flush_on_every_edge() {
        let bar = Rect::sized(-3000, 0, 1920, 464);
        let cases = [
            (
                Edge::Left,
                1.0,
                Rect::sized(-3000, 196, 4, 72),
                Rect::sized(-3000, 196, 8, 72),
            ),
            (
                Edge::Right,
                1.0,
                Rect::sized(-1084, 196, 4, 72),
                Rect::sized(-1088, 196, 8, 72),
            ),
            (
                Edge::Top,
                1.0,
                Rect::sized(-2076, 0, 72, 4),
                Rect::sized(-2076, 0, 72, 8),
            ),
            (
                Edge::Bottom,
                1.0,
                Rect::sized(-2076, 460, 72, 4),
                Rect::sized(-2076, 456, 72, 8),
            ),
            (
                Edge::Left,
                1.5,
                Rect::sized(-3000, 178, 6, 108),
                Rect::sized(-3000, 178, 12, 108),
            ),
            (
                Edge::Right,
                1.5,
                Rect::sized(-1086, 178, 6, 108),
                Rect::sized(-1092, 178, 12, 108),
            ),
            (
                Edge::Top,
                1.5,
                Rect::sized(-2094, 0, 108, 6),
                Rect::sized(-2094, 0, 108, 12),
            ),
            (
                Edge::Bottom,
                1.5,
                Rect::sized(-2094, 458, 108, 6),
                Rect::sized(-2094, 452, 108, 12),
            ),
        ];
        for (edge, scale, strip, hover) in cases {
            assert_eq!(
                tab_rect(&bar, at(edge, 0.5), 72.0, 4.0, scale),
                strip,
                "{edge:?} at {scale}"
            );
            assert_eq!(
                tab_rect(&bar, at(edge, 0.5), 72.0, 8.0, scale),
                hover,
                "{edge:?} at {scale}"
            );
        }
        assert_eq!(
            tab_rect(&bar, at(Edge::Left, 0.0), 72.0, 8.0, 1.0),
            Rect::sized(-3000, 0, 8, 72)
        );
        assert_eq!(
            tab_rect(&bar, at(Edge::Top, 1.0), 72.0, 8.0, 1.0),
            Rect::sized(-1152, 0, 72, 8)
        );
    }

    #[test]
    fn the_panel_opens_against_the_tab_edge_centred_on_the_tab() {
        let oled = monitor("oled", Rect::sized(0, 0, 2560, 1440), 48, 1.0, true);
        let place = |edge, offset| {
            place_panel_on_edge(
                tab_rect(&oled.bounds, at(edge, offset), 72.0, 8.0, 1.0),
                edge,
                &oled,
                300.0,
            )
            .0
        };
        assert_eq!(
            place(Edge::Right, 0.5),
            Rect::sized(2560 - 8 - 376, 720 - 150, 376, 300)
        );
        assert_eq!(place(Edge::Left, 0.5), Rect::sized(8, 720 - 150, 376, 300));
        assert_eq!(place(Edge::Top, 0.5), Rect::sized(1280 - 188, 8, 376, 300));
        assert_eq!(
            place(Edge::Bottom, 0.5),
            Rect::sized(1280 - 188, 1392 - 300, 376, 300)
        );
        assert_eq!(
            place(Edge::Right, 0.0),
            Rect::sized(2560 - 8 - 376, 0, 376, 300)
        );
        assert_eq!(
            place(Edge::Bottom, 1.0),
            Rect::sized(2560 - 376, 1392 - 300, 376, 300)
        );
    }

    #[test]
    fn the_panel_grows_to_the_work_area_and_stays_attached_to_the_tab_edge() {
        let oled = monitor("oled", Rect::sized(0, 0, 2560, 1440), 48, 1.0, true);
        let place = |edge, offset, content_height| {
            place_panel_on_edge(
                tab_rect(&oled.bounds, at(edge, offset), 72.0, 8.0, 1.0),
                edge,
                &oled,
                content_height,
            )
            .0
        };
        let tallest = 1392 - 24;
        assert_eq!(
            place(Edge::Right, 0.5, 1000.0),
            Rect::sized(2560 - 8 - 376, 720 - 500, 376, 1000)
        );
        assert_eq!(
            place(Edge::Right, 0.1, 1000.0),
            Rect::sized(2560 - 8 - 376, 0, 376, 1000)
        );
        assert_eq!(
            place(Edge::Left, 0.5, 3000.0),
            Rect::sized(8, 1392 - tallest, 376, tallest)
        );
        assert_eq!(
            place(Edge::Top, 0.5, 3000.0),
            Rect::sized(1280 - 188, 8, 376, tallest)
        );
        assert_eq!(
            place(Edge::Bottom, 0.5, 3000.0),
            Rect::sized(1280 - 188, 1392 - tallest, 376, tallest)
        );
        let bar = &owner_desk()[2];
        let on_bar = place_panel_on_edge(
            tab_rect(&bar.bounds, at(Edge::Left, 0.5), 72.0, 8.0, 1.0),
            Edge::Left,
            bar,
            3000.0,
        )
        .0;
        assert_eq!(on_bar, Rect::sized(-3000 + 8, 12, 376, 464 - 24));
    }

    #[test]
    fn a_pill_reports_the_nearer_horizontal_edge() {
        let work = Rect::sized(0, 0, 1920, 1032);
        assert_eq!(pill_edge(Rect::sized(1800, 12, 100, 28), &work), Edge::Top);
        assert_eq!(
            pill_edge(Rect::sized(12, 990, 100, 28), &work),
            Edge::Bottom
        );
    }

    #[test]
    fn a_hidden_pill_reveals_from_a_thin_strip_on_the_side_nearest_the_screen_edge() {
        let work = Rect::sized(0, 0, 1920, 1032);
        let top = Rect::sized(1800, 12, 100, 28);
        assert_eq!(reveal_strip(top, &work, 1.0), Rect::sized(1800, 12, 100, 4));
        assert_eq!(reveal_strip(top, &work, 1.5), Rect::sized(1800, 12, 100, 6));
        let bottom = Rect::sized(12, 990, 100, 28);
        assert_eq!(
            reveal_strip(bottom, &work, 1.0),
            Rect::sized(12, 1014, 100, 4)
        );
        assert_eq!(reveal_strip(top, &work, 100.0).height(), 28);
    }

    #[test]
    fn sliding_follows_the_pointer_along_the_edge_and_the_nearest_edge_wins() {
        let bar = Rect::sized(-3000, 0, 1920, 464);
        assert_eq!(slide(&bar, Edge::Left, (-2990, 116)), at(Edge::Left, 0.25));
        assert_eq!(slide(&bar, Edge::Top, (-2040, 300)), at(Edge::Top, 0.5));
        assert_eq!(slide(&bar, Edge::Left, (-2990, 900)), at(Edge::Left, 1.0));
        assert_eq!(nearest_edge(&bar, (-2990, 200)), Edge::Left);
        assert_eq!(nearest_edge(&bar, (-1100, 200)), Edge::Right);
        assert_eq!(nearest_edge(&bar, (-2000, 10)), Edge::Top);
        assert_eq!(nearest_edge(&bar, (-2000, 450)), Edge::Bottom);
        let philips = &owner_desk()[1];
        assert!(near_edge(philips, Edge::Top, (-500, 80)));
        assert!(!near_edge(philips, Edge::Top, (-500, 81)));
    }

    #[test]
    fn the_tab_length_follows_the_chosen_size() {
        let bar = Rect::sized(-3000, 0, 1920, 464);
        assert_eq!(
            tab_rect(&bar, at(Edge::Left, 0.5), 56.0, 3.0, 1.0),
            Rect::sized(-3000, 204, 3, 56)
        );
        assert_eq!(
            tab_rect(&bar, at(Edge::Top, 0.5), 96.0, 6.0, 1.5),
            Rect::sized(-2112, 0, 144, 9)
        );
    }

    #[test]
    fn the_peek_slides_from_the_anchor_toward_the_screen_interior() {
        let bar = Rect::sized(-3000, 0, 1920, 464);
        let size = (160, 26);
        let tab = tab_rect(&bar, at(Edge::Left, 0.5), 72.0, 8.0, 1.0);
        assert_eq!(
            peek_rect(tab, Edge::Left, size, 0.0, &bar, 1.0),
            Rect::sized(-3000 + 8 + 6, 219, 160, 26)
        );
        assert_eq!(
            peek_rect(tab, Edge::Left, size, 12.0, &bar, 1.0),
            Rect::sized(-3000 + 2, 219, 160, 26)
        );
        let right = tab_rect(&bar, at(Edge::Right, 0.0), 72.0, 8.0, 1.0);
        assert_eq!(
            peek_rect(right, Edge::Right, size, 0.0, &bar, 1.0),
            Rect::sized(-1080 - 8 - 6 - 160, 23, 160, 26)
        );
        let top = tab_rect(&bar, at(Edge::Top, 0.5), 72.0, 8.0, 1.0);
        assert_eq!(
            peek_rect(top, Edge::Top, size, 12.0, &bar, 1.0),
            Rect::sized(-2120, 2, 160, 26)
        );
        let pill = Rect::sized(-1200, 436, 100, 28);
        assert_eq!(
            peek_rect(pill, Edge::Bottom, size, 0.0, &bar, 1.0),
            Rect::sized(-1240, 436 - 6 - 26, 160, 26)
        );
        assert_eq!(
            peek_rect(
                Rect::sized(-1150, 12, 100, 28),
                Edge::Top,
                size,
                0.0,
                &bar,
                1.0
            ),
            Rect::sized(-1240, 46, 160, 26)
        );
    }

    #[test]
    fn a_press_is_inside_the_panel_on_an_anchor_or_outside() {
        let panel = Some(Rect::sized(100, 100, 376, 300));
        let anchors = [Some(Rect::sized(0, 214, 8, 72)), None];
        assert_eq!(
            press_target((200, 200), panel, &anchors),
            PressTarget::Panel
        );
        assert_eq!(press_target((4, 250), panel, &anchors), PressTarget::Anchor);
        assert_eq!(
            press_target((900, 900), panel, &anchors),
            PressTarget::Outside
        );
        assert_eq!(press_target((4, 250), None, &anchors), PressTarget::Anchor);
        assert_eq!(press_target((200, 200), None, &[]), PressTarget::Outside);
    }

    #[test]
    fn a_pill_reads_as_the_corner_of_its_quarter() {
        let work = Rect::sized(-3000, 0, 1920, 464);
        let corner = |left, top| nearest_corner(Rect::sized(left, top, 100, 28), &work);
        assert_eq!(corner(-2988, 12), Corner::TopLeft);
        assert_eq!(corner(-1192, 12), Corner::TopRight);
        assert_eq!(corner(-2988, 424), Corner::BottomLeft);
        assert_eq!(corner(-1192, 424), Corner::BottomRight);
    }
}
