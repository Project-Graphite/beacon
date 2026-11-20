use crate::geometry::Edge;
use crate::model::{Mark, MarkKind, PillModel, TabTone, TrayState};
use crate::motion::Curve;
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const PAPER: Rgb = Rgb(0x0c, 0x0c, 0x0e);
pub const SURFACE: Rgb = Rgb(0x13, 0x13, 0x16);
pub const LINE: Rgb = Rgb(0x26, 0x26, 0x2c);
pub const INK: Rgb = Rgb(0xe8, 0xe8, 0xea);
pub const MUTED: Rgb = Rgb(0x8b, 0x8b, 0x95);
pub const DORMANT: Rgb = Rgb(0x6e, 0x6e, 0x78);
pub const LIVE: Rgb = Rgb(0x46, 0xb9, 0x7a);
pub const WIP: Rgb = Rgb(0xd7, 0xa5, 0x4a);
pub const DANGER: Rgb = Rgb(0xf2, 0xa0, 0x9a);

const BAR_HEIGHT: f32 = 28.0;
const PADDING: f32 = 10.0;
const MARK_SIZE: f32 = 10.0;
const MARK_GAP: f32 = 8.0;
const COUNT_GAP: f32 = 6.0;
const FONT_SIZE: f32 = 11.0;
const BAR_OPACITY: f32 = 0.96;
const SNOOZED_OPACITY: f32 = 0.45;
const WORKING_PERIOD: f32 = 1.2;
const HALO_PERIOD: f32 = 1.6;
const SHAPE_BLEED: f32 = 3.0;
const TRAY_UNITS: f32 = 16.0;
const MARK_CENTRE: f32 = 5.0;
const SMALLEST_GROWTH: f32 = 0.01;
const TAB_RADIUS: f32 = 2.0;
const TAB_HIT_DEPTH: f32 = 8.0;
const TAB_HIT_ALPHA: f32 = 1.0 / 255.0;
const GLOW_REACH: f32 = 18.0;
const GLOW_TAPER: f32 = 8.0;
const GLOW_ALPHA: f32 = 0.45;
const WORKING_TAB_OPACITY: f32 = 0.7;
const DORMANT_TAB_OPACITY: f32 = 0.6;

pub struct PillMotion<'a> {
    pub growths: &'a [f32],
    pub width: Option<usize>,
}

#[derive(Clone)]
pub struct TextMask {
    pub width: usize,
    pub height: usize,
    pub coverage: Vec<u8>,
}

pub trait TextRasterizer {
    fn rasterize(&self, text: &str, pixel_size: f32) -> TextMask;
}

#[derive(Default)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Canvas {
        let mut canvas = Canvas::default();
        canvas.reset(width, height);
        canvas
    }

    fn reset(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.pixels.clear();
        self.pixels.resize(width * height, [0.0; 4]);
    }

    fn blend(&mut self, x: usize, y: usize, colour: Rgb, coverage: f32) {
        let pixel = &mut self.pixels[y * self.width + x];
        let source = [colour.0, colour.1, colour.2].map(|channel| f32::from(channel) / 255.0);
        for channel in 0..3 {
            pixel[channel] = source[channel] * coverage + pixel[channel] * (1.0 - coverage);
        }
        pixel[3] = coverage + pixel[3] * (1.0 - coverage);
    }

    fn paint(
        &mut self,
        bounds: [f32; 4],
        colour: Rgb,
        opacity: f32,
        distance: impl Fn(f32, f32) -> f32,
    ) {
        let [left, top, right, bottom] = bounds;
        let columns =
            (left.floor().max(0.0) as usize)..(right.ceil().max(0.0) as usize).min(self.width);
        for y in (top.floor().max(0.0) as usize)..(bottom.ceil().max(0.0) as usize).min(self.height)
        {
            for x in columns.clone() {
                let coverage =
                    (0.5 - distance(x as f32 + 0.5, y as f32 + 0.5)).clamp(0.0, 1.0) * opacity;
                if coverage > 0.0 {
                    self.blend(x, y, colour, coverage);
                }
            }
        }
    }

    fn paint_mask(&mut self, mask: &TextMask, left: f32, colour: Rgb) {
        let left = left.round() as usize;
        let top = self.height.saturating_sub(mask.height) / 2;
        for y in 0..mask.height.min(self.height - top) {
            for x in 0..mask.width.min(self.width.saturating_sub(left)) {
                let coverage = f32::from(mask.coverage[y * mask.width + x]) / 255.0;
                if coverage > 0.0 {
                    self.blend(left + x, top + y, colour, coverage);
                }
            }
        }
    }

    fn paint_bar(&mut self, scale: f32) {
        let (width, height) = (self.width as f32, self.height as f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let distance = rounded_box(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    [width / 2.0, height / 2.0, width / 2.0, height / 2.0],
                    height / 2.0,
                );
                let outer = (0.5 - distance).clamp(0.0, 1.0);
                let inner = (0.5 - distance - scale).clamp(0.0, 1.0);
                let pixel = &mut self.pixels[y * self.width + x];
                for (channel, (line, surface)) in [
                    (LINE.0, SURFACE.0),
                    (LINE.1, SURFACE.1),
                    (LINE.2, SURFACE.2),
                ]
                .into_iter()
                .enumerate()
                {
                    pixel[channel] =
                        (f32::from(line) * (outer - inner) + f32::from(surface) * inner) / 255.0
                            * BAR_OPACITY;
                }
                pixel[3] = outer * BAR_OPACITY;
            }
        }
    }

    pub fn write_premultiplied_bgra(&self, bytes: &mut Vec<u8>) {
        bytes.clear();
        bytes.extend(
            self.pixels
                .iter()
                .flat_map(|[red, green, blue, alpha]| [*blue, *green, *red, *alpha].map(to_byte)),
        );
    }

    pub fn straight_rgba(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|[red, green, blue, alpha]| {
                let unpremultiply = |channel: f32| if *alpha > 0.0 { channel / alpha } else { 0.0 };
                [
                    unpremultiply(*red),
                    unpremultiply(*green),
                    unpremultiply(*blue),
                    *alpha,
                ]
                .map(to_byte)
            })
            .collect()
    }
}

fn to_byte(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

fn circle(x: f32, y: f32, centre: [f32; 2], radius: f32) -> f32 {
    (x - centre[0]).hypot(y - centre[1]) - radius
}

fn ring(x: f32, y: f32, centre: [f32; 2], radius: f32, stroke: f32) -> f32 {
    circle(x, y, centre, radius).abs() - stroke / 2.0
}

fn arc(x: f32, y: f32, centre: [f32; 2], radius: f32, stroke: f32, start: f32, sweep: f32) -> f32 {
    let angle = (y - centre[1]).atan2(x - centre[0]);
    if (angle - start).rem_euclid(TAU) <= sweep {
        return ring(x, y, centre, radius, stroke);
    }
    [start, start + sweep]
        .map(|end| {
            circle(
                x,
                y,
                [
                    centre[0] + radius * end.cos(),
                    centre[1] + radius * end.sin(),
                ],
                stroke / 2.0,
            )
        })
        .into_iter()
        .fold(f32::INFINITY, f32::min)
}

fn segment(x: f32, y: f32, from: [f32; 2], to: [f32; 2], stroke: f32) -> f32 {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let along = (((x - from[0]) * dx + (y - from[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (x - from[0] - dx * along).hypot(y - from[1] - dy * along) - stroke / 2.0
}

fn rounded_box(
    x: f32,
    y: f32,
    [centre_x, centre_y, half_width, half_height]: [f32; 4],
    radius: f32,
) -> f32 {
    let qx = (x - centre_x).abs() - half_width + radius;
    let qy = (y - centre_y).abs() - half_height + radius;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

fn triangle(x: f32, y: f32, corners: [[f32; 2]; 3]) -> f32 {
    let edge = |index: usize| {
        let (from, to) = (corners[index], corners[(index + 1) % 3]);
        [to[0] - from[0], to[1] - from[1]]
    };
    let orientation = (edge(0)[0] * edge(2)[1] - edge(0)[1] * edge(2)[0]).signum();
    let (squared, inside) = (0..3)
        .map(|index| {
            let [ex, ey] = edge(index);
            let (vx, vy) = (x - corners[index][0], y - corners[index][1]);
            let along = ((vx * ex + vy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
            let (px, py) = (vx - ex * along, vy - ey * along);
            (px * px + py * py, orientation * (vx * ey - vy * ex))
        })
        .fold(
            (f32::INFINITY, f32::INFINITY),
            |(squared, inside), (s, i)| (squared.min(s), inside.min(i)),
        );
    -squared.sqrt() * inside.signum()
}

fn paint_shape(
    canvas: &mut Canvas,
    origin: [f32; 2],
    extent: f32,
    scale: f32,
    colour: Rgb,
    opacity: f32,
    distance: impl Fn(f32, f32) -> f32,
) {
    let bleed = SHAPE_BLEED * scale;
    let far = (extent + SHAPE_BLEED) * scale;
    canvas.paint(
        [
            origin[0] - bleed,
            origin[1] - bleed,
            origin[0] + far,
            origin[1] + far,
        ],
        colour,
        opacity,
        |x, y| distance((x - origin[0]) / scale, (y - origin[1]) / scale) * scale,
    );
}

fn paint_mark(
    canvas: &mut Canvas,
    mark: Mark,
    origin: [f32; 2],
    scale: f32,
    phase: f32,
    reduced_motion: bool,
) {
    const CENTRE: [f32; 2] = [5.0, 5.0];
    let opacity = if mark.snoozed { SNOOZED_OPACITY } else { 1.0 };
    let animated = !reduced_motion && !mark.snoozed;
    let mut shape = |colour: Rgb, opacity: f32, distance: &dyn Fn(f32, f32) -> f32| {
        paint_shape(canvas, origin, MARK_SIZE, scale, colour, opacity, distance)
    };
    match mark.kind {
        MarkKind::Starting => shape(DORMANT, opacity, &|x, y| circle(x, y, CENTRE, 2.0)),
        MarkKind::Working if animated => {
            let start = phase / WORKING_PERIOD * TAU;
            shape(INK, opacity, &|x, y| {
                arc(x, y, CENTRE, 4.0, 1.5, start, 0.75 * TAU)
            })
        }
        MarkKind::Working => shape(INK, opacity, &|x, y| ring(x, y, CENTRE, 4.0, 1.5)),
        MarkKind::NeedsInput => {
            if animated {
                let progress = Curve::Settle.at(f64::from((phase / HALO_PERIOD).fract())) as f32;
                shape(WIP, 1.0 - progress, &|x, y| {
                    ring(x, y, CENTRE, 4.5 + 2.5 * progress, 1.25)
                });
            }
            shape(WIP, opacity, &|x, y| circle(x, y, CENTRE, 4.5))
        }
        MarkKind::Stalled => shape(WIP, opacity, &|x, y| {
            rounded_box(x, y, [2.75, 5.0, 1.25, 4.0], 0.0).min(rounded_box(
                x,
                y,
                [7.25, 5.0, 1.25, 4.0],
                0.0,
            ))
        }),
        MarkKind::Done => shape(LIVE, opacity, &|x, y| {
            segment(x, y, [1.5, 5.2], [4.0, 7.6], 1.75).min(segment(
                x,
                y,
                [4.0, 7.6],
                [8.5, 2.6],
                1.75,
            ))
        }),
        MarkKind::Error => shape(DANGER, opacity, &|x, y| {
            triangle(x, y, [[0.5, 9.0], [9.5, 9.0], [5.0, 1.0]])
        }),
        MarkKind::Idle => shape(MUTED, opacity, &|x, y| ring(x, y, CENTRE, 3.5, 1.25)),
    }
}

pub fn render_pill(
    model: &PillModel,
    scale: f32,
    phase: f32,
    reduced_motion: bool,
    text: &dyn TextRasterizer,
) -> Canvas {
    let mut canvas = Canvas::default();
    paint_pill(
        &mut canvas,
        model,
        scale,
        phase,
        reduced_motion,
        text,
        &PillMotion {
            growths: &[],
            width: None,
        },
    );
    canvas
}

pub fn paint_pill(
    canvas: &mut Canvas,
    model: &PillModel,
    scale: f32,
    phase: f32,
    reduced_motion: bool,
    text: &dyn TextRasterizer,
    motion: &PillMotion,
) {
    let font = FONT_SIZE * scale;
    let overflow =
        (model.overflow > 0).then(|| text.rasterize(&format!("+{}", model.overflow), font));
    let count = (model.alerting > 0).then(|| text.rasterize(&model.alerting.to_string(), font));
    let slots = model.marks.len().max(1) as f32;
    let marks_end = (PADDING + slots * MARK_SIZE + (slots - 1.0) * MARK_GAP) * scale;
    let overflow_left = marks_end + MARK_GAP * scale;
    let overflow_end = overflow
        .as_ref()
        .map_or(marks_end, |mask| overflow_left + mask.width as f32);
    let count_left = overflow_end + COUNT_GAP * scale;
    let content_end = count
        .as_ref()
        .map_or(overflow_end, |mask| count_left + mask.width as f32);
    canvas.reset(
        motion
            .width
            .unwrap_or((content_end + PADDING * scale).round() as usize),
        (BAR_HEIGHT * scale).round() as usize,
    );
    canvas.paint_bar(scale);
    let top = (BAR_HEIGHT - MARK_SIZE) / 2.0 * scale;
    if model.marks.is_empty() {
        paint_shape(
            canvas,
            [PADDING * scale, top],
            MARK_SIZE,
            scale,
            DORMANT,
            1.0,
            |x, y| ring(x, y, [5.0, 5.0], 3.5, 1.25),
        );
    }
    for (index, mark) in model.marks.iter().enumerate() {
        let growth = motion.growths.get(index).copied().unwrap_or(1.0);
        if growth < SMALLEST_GROWTH {
            continue;
        }
        let shrink = MARK_CENTRE * scale * (1.0 - growth);
        let left = (PADDING + index as f32 * (MARK_SIZE + MARK_GAP)) * scale;
        paint_mark(
            canvas,
            *mark,
            [left + shrink, top + shrink],
            scale * growth,
            phase,
            reduced_motion,
        );
    }
    if let Some(mask) = &overflow {
        canvas.paint_mask(mask, overflow_left, MUTED);
    }
    if let Some(mask) = &count {
        canvas.paint_mask(mask, count_left, WIP);
    }
}

pub fn tone_paint(tone: TabTone) -> (Rgb, f32) {
    match tone {
        TabTone::NeedsYou => (WIP, 1.0),
        TabTone::Failed => (DANGER, 1.0),
        TabTone::Finished => (LIVE, 1.0),
        TabTone::Working => (INK, WORKING_TAB_OPACITY),
        TabTone::Dormant => (DORMANT, DORMANT_TAB_OPACITY),
    }
}

pub fn mix(
    (from, from_opacity): (Rgb, f32),
    (to, to_opacity): (Rgb, f32),
    amount: f32,
) -> (Rgb, f32) {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    (
        Rgb(
            channel(from.0, to.0),
            channel(from.1, to.1),
            channel(from.2, to.2),
        ),
        from_opacity + (to_opacity - from_opacity) * amount,
    )
}

pub fn paint_tab(
    canvas: &mut Canvas,
    (width, height): (usize, usize),
    edge: Edge,
    (colour, opacity): (Rgb, f32),
    thickness: f32,
    glow: f32,
    scale: f32,
) {
    canvas.reset(width, height);
    let length = match edge {
        Edge::Left | Edge::Right => height,
        Edge::Top | Edge::Bottom => width,
    } as f32;
    let (thickness, radius) = (thickness * scale, TAB_RADIUS * scale);
    let (hit_depth, reach, taper) = (
        TAB_HIT_DEPTH * scale,
        GLOW_REACH * scale,
        GLOW_TAPER * scale,
    );
    let strip = [
        length / 2.0,
        (thickness - radius) / 2.0,
        length / 2.0,
        (thickness + radius) / 2.0,
    ];
    for y in 0..height {
        for x in 0..width {
            let (across, down) = (x as f32 + 0.5, y as f32 + 0.5);
            let (along, depth) = match edge {
                Edge::Left => (down, across),
                Edge::Right => (down, width as f32 - across),
                Edge::Top => (across, down),
                Edge::Bottom => (across, height as f32 - down),
            };
            if depth < hit_depth {
                canvas.pixels[y * width + x] = [0.0, 0.0, 0.0, TAB_HIT_ALPHA];
            }
            if glow > 0.0 && depth > thickness {
                let fade = (1.0 - (depth - thickness) / (reach - thickness)).clamp(0.0, 1.0);
                let ends = (along.min(length - along) / taper).clamp(0.0, 1.0);
                let coverage = glow * GLOW_ALPHA * fade * fade * ends * opacity;
                if coverage > 0.0 {
                    canvas.blend(x, y, colour, coverage);
                }
            }
            let coverage =
                (0.5 - rounded_box(along, depth, strip, radius)).clamp(0.0, 1.0) * opacity;
            if coverage > 0.0 {
                canvas.blend(x, y, colour, coverage);
            }
        }
    }
}

pub fn render_tray(state: TrayState, size: usize, text: &dyn TextRasterizer) -> Canvas {
    const CENTRE: [f32; 2] = [8.0, 8.0];
    let mut canvas = Canvas::new(size, size);
    let unit = size as f32 / TRAY_UNITS;
    let mut shape = |colour: Rgb, distance: &dyn Fn(f32, f32) -> f32| {
        paint_shape(
            &mut canvas,
            [0.0, 0.0],
            TRAY_UNITS,
            unit,
            colour,
            1.0,
            distance,
        )
    };
    match state {
        TrayState::Quiet => shape(MUTED, &|x, y| ring(x, y, CENTRE, 5.5, 2.0)),
        TrayState::Working => shape(INK, &|x, y| ring(x, y, CENTRE, 5.5, 2.0)),
        TrayState::Error => shape(DANGER, &|x, y| {
            triangle(x, y, [[1.0, 14.5], [15.0, 14.5], [8.0, 1.5]])
        }),
        TrayState::Alerting(count) => {
            shape(WIP, &|x, y| circle(x, y, CENTRE, 7.5));
            let label = if count > 9 {
                "9+".to_string()
            } else {
                count.to_string()
            };
            let mask = text.rasterize(&label, 10.0 * unit);
            let left = size.saturating_sub(mask.width) as f32 / 2.0;
            canvas.paint_mask(&mask, left, PAPER);
        }
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mark, MarkKind};

    struct BlockText;

    impl TextRasterizer for BlockText {
        fn rasterize(&self, text: &str, pixel_size: f32) -> TextMask {
            let width = (text.chars().count() as f32 * pixel_size * 0.6).round() as usize;
            let height = (pixel_size * 0.75).round() as usize;
            TextMask {
                width,
                height,
                coverage: vec![255; width * height],
            }
        }
    }

    const SCALES: [f32; 2] = [1.0, 1.5];

    fn model(kinds: &[MarkKind], alerting: usize) -> PillModel {
        PillModel {
            marks: kinds
                .iter()
                .map(|&kind| Mark {
                    kind,
                    snoozed: false,
                })
                .collect(),
            overflow: 0,
            alerting,
        }
    }

    fn render(model: &PillModel, scale: f32) -> Canvas {
        render_pill(model, scale, 0.0, true, &BlockText)
    }

    fn straight(canvas: &Canvas, x: usize, y: usize) -> [u8; 4] {
        let rgba = canvas.straight_rgba();
        let index = (y * canvas.width + x) * 4;
        [
            rgba[index],
            rgba[index + 1],
            rgba[index + 2],
            rgba[index + 3],
        ]
    }

    fn close_to(actual: [u8; 4], colour: Rgb, tolerance: u8) -> bool {
        [colour.0, colour.1, colour.2]
            .iter()
            .zip(actual)
            .all(|(expected, actual)| expected.abs_diff(actual) <= tolerance)
    }

    #[test]
    fn corners_are_fully_transparent() {
        for scale in SCALES {
            let canvas = render(&model(&[MarkKind::Working, MarkKind::Idle], 0), scale);
            for (x, y) in [
                (0, 0),
                (canvas.width - 1, 0),
                (0, canvas.height - 1),
                (canvas.width - 1, canvas.height - 1),
            ] {
                assert_eq!(straight(&canvas, x, y)[3], 0, "scale {scale} at {x},{y}");
            }
        }
    }

    #[test]
    fn the_bar_interior_is_the_surface_colour_at_ninety_six_percent() {
        for scale in SCALES {
            let canvas = render(&model(&[MarkKind::Idle, MarkKind::Idle], 0), scale);
            let pixel = straight(&canvas, (24.0 * scale) as usize, canvas.height / 2);
            assert!(close_to(pixel, SURFACE, 1), "scale {scale}: {pixel:?}");
            assert_eq!(pixel[3], 245);
        }
    }

    #[test]
    fn the_bar_is_twenty_eight_logical_pixels_high() {
        assert_eq!(render(&model(&[], 0), 1.0).height, 28);
        assert_eq!(render(&model(&[], 0), 1.5).height, 42);
    }

    #[test]
    fn width_grows_by_eighteen_logical_pixels_per_mark() {
        for scale in SCALES {
            let widths: Vec<usize> = (1..=10)
                .map(|count| render(&model(&vec![MarkKind::Working; count], 0), scale).width)
                .collect();
            assert_eq!(widths[0], (30.0 * scale) as usize);
            for pair in widths.windows(2) {
                assert_eq!(pair[1] - pair[0], (18.0 * scale) as usize, "scale {scale}");
            }
        }
    }

    #[test]
    fn each_state_draws_its_colour_at_its_point() {
        let cases = [
            (MarkKind::Starting, [5.0, 5.0], DORMANT),
            (MarkKind::Working, [8.5, 6.5], INK),
            (MarkKind::NeedsInput, [5.0, 5.0], WIP),
            (MarkKind::Stalled, [2.75, 5.0], WIP),
            (MarkKind::Done, [4.0, 7.6], LIVE),
            (MarkKind::Error, [5.0, 6.0], DANGER),
            (MarkKind::Idle, [8.5, 5.5], MUTED),
        ];
        let kinds: Vec<MarkKind> = cases.iter().map(|(kind, ..)| *kind).collect();
        for scale in SCALES {
            let canvas = render(&model(&kinds, 0), scale);
            for (index, (kind, [x, y], colour)) in cases.iter().enumerate() {
                let left = (10.0 + index as f32 * 18.0) * scale;
                let pixel = straight(
                    &canvas,
                    (left + x * scale) as usize,
                    (9.0 * scale + y * scale) as usize,
                );
                assert!(
                    close_to(pixel, *colour, 2),
                    "{kind:?} at scale {scale}: {pixel:?}"
                );
            }
        }
    }

    #[test]
    fn a_snoozed_mark_is_drawn_at_forty_five_percent() {
        let snoozed = PillModel {
            marks: vec![Mark {
                kind: MarkKind::NeedsInput,
                snoozed: true,
            }],
            overflow: 0,
            alerting: 0,
        };
        for scale in SCALES {
            let canvas = render_pill(&snoozed, scale, 0.4, false, &BlockText);
            let pixel = straight(&canvas, (15.0 * scale) as usize, (14.0 * scale) as usize);
            let alpha = 0.45 + BAR_OPACITY * 0.55;
            let expected = |mark: u8, bar: u8| {
                ((f32::from(mark) * 0.45 + f32::from(bar) * BAR_OPACITY * 0.55) / alpha).round()
                    as u8
            };
            let colour = Rgb(
                expected(WIP.0, SURFACE.0),
                expected(WIP.1, SURFACE.1),
                expected(WIP.2, SURFACE.2),
            );
            assert!(
                close_to(pixel, colour, 2),
                "scale {scale}: {pixel:?} vs {colour:?}"
            );
            assert_eq!(pixel[3], to_byte(alpha));
        }
    }

    #[test]
    fn the_alerting_count_appears_only_when_a_session_is_alerting() {
        for scale in SCALES {
            let quiet = render(&model(&[MarkKind::NeedsInput], 0), scale);
            assert_eq!(quiet.width, (30.0 * scale) as usize);
            let alerting = render(&model(&[MarkKind::NeedsInput], 1), scale);
            let count_left = (26.0 * scale) as usize;
            let count_pixels = (count_left..alerting.width - (10.0 * scale) as usize)
                .filter(|x| close_to(straight(&alerting, *x, alerting.height / 2), WIP, 2))
                .count();
            assert!(alerting.width > quiet.width);
            assert!(count_pixels > 0, "scale {scale}");
        }
    }

    #[test]
    fn overflow_text_follows_the_marks_in_muted() {
        let mut overflowing = model(&[MarkKind::Idle; 10], 0);
        overflowing.overflow = 2;
        let canvas = render(&overflowing, 1.0);
        let text_left = 10 + 10 * 18 - 8 + 8;
        assert!(close_to(straight(&canvas, text_left + 1, 14), MUTED, 2));
    }

    #[test]
    fn the_empty_pill_is_a_single_dormant_ring() {
        let canvas = render(&model(&[], 0), 1.0);
        assert_eq!(canvas.width, 30);
        assert!(close_to(straight(&canvas, 18, 14), DORMANT, 2));
        assert!(close_to(straight(&canvas, 15, 14), SURFACE, 1));
    }

    #[test]
    fn the_tab_paints_its_strip_a_faint_hit_band_and_nothing_beyond() {
        for scale in SCALES {
            let lp = |value: f32| (value * scale) as usize;
            let mut canvas = Canvas::default();
            paint_tab(
                &mut canvas,
                (lp(18.0), lp(72.0)),
                Edge::Left,
                (WIP, 1.0),
                4.0,
                0.0,
                scale,
            );
            let middle = lp(36.0);
            assert!(
                close_to(straight(&canvas, lp(2.0), middle), WIP, 1),
                "scale {scale}"
            );
            assert_eq!(straight(&canvas, lp(2.0), middle)[3], 255);
            assert_eq!(straight(&canvas, lp(6.0), middle)[3], 1);
            assert_eq!(straight(&canvas, lp(12.0), middle)[3], 0);
            assert!(
                straight(&canvas, lp(3.0), 0)[3] < 200,
                "rounded inner corner"
            );
            paint_tab(
                &mut canvas,
                (lp(72.0), lp(8.0)),
                Edge::Bottom,
                (DORMANT, 0.6),
                4.0,
                0.0,
                scale,
            );
            let bottom = straight(&canvas, middle, lp(8.0) - 1);
            assert!(close_to(bottom, DORMANT, 1) && bottom[3] == to_byte(0.6));
            assert_eq!(straight(&canvas, middle, 0)[3], 1);
            paint_tab(
                &mut canvas,
                (lp(18.0), lp(72.0)),
                Edge::Right,
                (LIVE, 1.0),
                6.0,
                1.0,
                scale,
            );
            assert!(close_to(straight(&canvas, lp(18.0) - 1, middle), LIVE, 1));
            assert!(straight(&canvas, lp(8.0), middle)[3] > 1);
            assert_eq!(straight(&canvas, 0, middle)[3], 0);
        }
    }

    #[test]
    fn tones_follow_the_palette_and_mix_smoothly() {
        assert_eq!(tone_paint(TabTone::NeedsYou), (WIP, 1.0));
        assert_eq!(tone_paint(TabTone::Working), (INK, 0.7));
        assert_eq!(tone_paint(TabTone::Dormant), (DORMANT, 0.6));
        assert_eq!(mix((WIP, 1.0), (LIVE, 0.5), 0.0), (WIP, 1.0));
        assert_eq!(mix((WIP, 1.0), (LIVE, 0.5), 1.0), (LIVE, 0.5));
        assert_eq!(
            mix((Rgb(0, 0, 0), 0.0), (Rgb(200, 100, 50), 1.0), 0.5),
            (Rgb(100, 50, 25), 0.5)
        );
    }

    #[test]
    fn a_growing_mark_starts_small_and_an_animated_width_overrides_the_layout() {
        let model = model(&[MarkKind::NeedsInput], 0);
        let mut canvas = Canvas::default();
        let motion = |growths, width| PillMotion { growths, width };
        paint_pill(
            &mut canvas,
            &model,
            1.0,
            0.0,
            true,
            &BlockText,
            &motion(&[0.5], Some(40)),
        );
        assert_eq!(canvas.width, 40);
        assert!(close_to(straight(&canvas, 14, 13), WIP, 2));
        assert!(close_to(straight(&canvas, 11, 14), SURFACE, 1));
        paint_pill(
            &mut canvas,
            &model,
            1.0,
            0.0,
            true,
            &BlockText,
            &motion(&[1.0], None),
        );
        assert_eq!(canvas.width, 30);
        assert!(close_to(straight(&canvas, 11, 14), WIP, 2));
    }

    #[test]
    fn tray_icons_follow_the_tray_state() {
        for size in [16, 24, 32] {
            let unit = size as f32 / 16.0;
            let at = |state: TrayState, x: f32, y: f32| {
                straight(
                    &render_tray(state, size, &BlockText),
                    (x * unit) as usize,
                    (y * unit) as usize,
                )
            };
            assert!(
                close_to(at(TrayState::Quiet, 13.5, 8.0), MUTED, 3),
                "{size}"
            );
            assert_eq!(at(TrayState::Quiet, 8.0, 8.0)[3], 0);
            assert!(
                close_to(at(TrayState::Working, 13.5, 8.0), INK, 3),
                "{size}"
            );
            assert!(
                close_to(at(TrayState::Alerting(3), 8.0, 1.5), WIP, 3),
                "{size}"
            );
            assert!(
                close_to(at(TrayState::Alerting(12), 8.0, 8.0), PAPER, 3),
                "{size}"
            );
            assert!(
                close_to(at(TrayState::Error, 8.0, 10.0), DANGER, 3),
                "{size}"
            );
        }
    }
}
