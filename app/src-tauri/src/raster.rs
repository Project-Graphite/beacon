use crate::geometry::Edge;
use crate::model::{Mark, MarkKind, PillModel, TabTone, TrayState};
use crate::motion::Curve;
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const SURFACE: Rgb = Rgb(0x19, 0x19, 0x1d);
pub const LINE: Rgb = Rgb(0x2b, 0x2b, 0x32);
pub const INK: Rgb = Rgb(0xec, 0xec, 0xef);
pub const MUTED: Rgb = Rgb(0x9b, 0x9b, 0xa5);
pub const DORMANT: Rgb = Rgb(0x6e, 0x6e, 0x78);
pub const LIVE: Rgb = Rgb(0x4c, 0xc2, 0x83);
pub const WIP: Rgb = Rgb(0xe0, 0xad, 0x52);
pub const DANGER: Rgb = Rgb(0xf2, 0xa0, 0x9a);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub surface: Rgb,
    pub line: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    pub dormant: Rgb,
    pub live: Rgb,
    pub wip: Rgb,
    pub danger: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub theme: Theme,
    pub solid: bool,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            theme: Theme::Dark,
            solid: true,
        }
    }
}

struct Shadow {
    colour: Rgb,
    alpha: f32,
    offset: f32,
    radius: f32,
}

impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette {
                surface: SURFACE,
                line: LINE,
                ink: INK,
                muted: MUTED,
                dormant: DORMANT,
                live: LIVE,
                wip: WIP,
                danger: DANGER,
            },
            Theme::Light => Palette {
                surface: Rgb(0xff, 0xff, 0xff),
                line: Rgb(0xdc, 0xdc, 0xe2),
                ink: Rgb(0x18, 0x18, 0x1b),
                muted: Rgb(0x55, 0x55, 0x5f),
                dormant: Rgb(0x85, 0x85, 0x8e),
                live: Rgb(0x1f, 0x8a, 0x54),
                wip: Rgb(0xa8, 0x74, 0x1c),
                danger: Rgb(0xc2, 0x45, 0x3b),
            },
        }
    }
}

impl Look {
    fn shadow(self) -> Shadow {
        match (self.theme, self.solid) {
            (Theme::Dark, true) => Shadow {
                colour: Rgb(0, 0, 0),
                alpha: 0.55,
                offset: 2.0,
                radius: SHADOW_RADIUS,
            },
            (Theme::Light, true) => Shadow {
                colour: Rgb(0x18, 0x18, 0x1b),
                alpha: 0.22,
                offset: 2.0,
                radius: SHADOW_RADIUS,
            },
            (Theme::Dark, false) => Shadow {
                colour: Rgb(0, 0, 0),
                alpha: 1.6,
                offset: 1.0,
                radius: FLOAT_SHADOW_RADIUS,
            },
            (Theme::Light, false) => Shadow {
                colour: Rgb(0xff, 0xff, 0xff),
                alpha: 1.6,
                offset: 1.0,
                radius: FLOAT_SHADOW_RADIUS,
            },
        }
    }
}

const BAR_HEIGHT: f32 = 28.0;
const PADDING: f32 = 10.0;
const MARK_SIZE: f32 = 10.0;
const MARK_GAP: f32 = 8.0;
const COUNT_GAP: f32 = 6.0;
const CHEVRON_GAP: f32 = 6.0;
const CHEVRON_SIZE: f32 = 10.0;
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
const PEEK_HEIGHT: f32 = 26.0;
const PEEK_PADDING: f32 = 10.0;
const PEEK_DOT: f32 = 6.0;
const PEEK_DOT_GAP: f32 = 8.0;
const PEEK_RADIUS: f32 = 8.0;
const PEEK_FONT_SIZE: f32 = 12.0;
const SHADOW_REACH: f32 = 9.0;
const SHADOW_RADIUS: f32 = 5.0;
const FLOAT_SHADOW_RADIUS: f32 = 3.0;
const SHADOW_CUTOFF: f32 = 2.0 / 255.0;
const TAB_HALO: f32 = 0.35;

pub struct PillMotion<'a> {
    pub growths: &'a [f32],
    pub width: Option<usize>,
    pub chevron: Chevron,
    pub look: Look,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Chevron {
    pub turn: f32,
    pub hot: bool,
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

    fn paint_bar(&mut self, scale: f32, radius: f32, palette: &Palette) {
        let (width, height) = (self.width as f32, self.height as f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let distance = rounded_box(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    [width / 2.0, height / 2.0, width / 2.0, height / 2.0],
                    radius,
                );
                let outer = (0.5 - distance).clamp(0.0, 1.0);
                let inner = (0.5 - distance - scale).clamp(0.0, 1.0);
                let pixel = &mut self.pixels[y * self.width + x];
                for (channel, (line, surface)) in [
                    (palette.line.0, palette.surface.0),
                    (palette.line.1, palette.surface.1),
                    (palette.line.2, palette.surface.2),
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

    fn paint_hit_bar(&mut self, radius: f32) {
        let (width, height) = (self.width as f32, self.height as f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let distance = rounded_box(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    [width / 2.0, height / 2.0, width / 2.0, height / 2.0],
                    radius,
                );
                if distance < 0.0 {
                    self.pixels[y * self.width + x] = [0.0, 0.0, 0.0, TAB_HIT_ALPHA];
                }
            }
        }
    }

    #[cfg(test)]
    pub fn lift_into(&self, target: &mut Canvas, scale: f32, look: Look) {
        let mut shadow = CastShadow::default();
        shadow.cast(self, scale, look);
        *target = shadow.canvas;
        let pad = shadow_pad(scale);
        for y in 0..self.height {
            for x in 0..self.width {
                let above = self.pixels[y * self.width + x];
                let below = &mut target.pixels[(y + pad) * target.width + x + pad];
                for channel in 0..4 {
                    below[channel] = above[channel] + below[channel] * (1.0 - above[3]);
                }
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

#[derive(Default)]
pub struct CastShadow {
    pub canvas: Canvas,
    solid: Option<(usize, usize, u32, Look)>,
    cast: Vec<f32>,
    scratch: Vec<f32>,
}

impl CastShadow {
    pub fn cast(&mut self, shape: &Canvas, scale: f32, look: Look) {
        let key = (shape.width, shape.height, scale.to_bits(), look);
        if self.solid == Some(key) {
            return;
        }
        self.solid = look.solid.then_some(key);
        let pad = shadow_pad(scale);
        let (width, height) = (shape.width + 2 * pad, shape.height + 2 * pad);
        let shadow = look.shadow();
        let offset = (shadow.offset * scale).round() as usize;
        self.cast.clear();
        self.cast.resize(width * height, 0.0);
        for y in 0..shape.height {
            for x in 0..shape.width {
                self.cast[(y + pad + offset) * width + x + pad] =
                    shape.pixels[y * shape.width + x][3];
            }
        }
        let radius = ((shadow.radius * scale / 2.0).ceil() as usize).max(1);
        self.scratch.clear();
        self.scratch.resize(width * height, 0.0);
        for _ in 0..2 {
            box_blur(&self.cast, &mut self.scratch, (width, height), radius, true);
            box_blur(
                &self.scratch,
                &mut self.cast,
                (width, height),
                radius,
                false,
            );
        }
        self.canvas.reset(width, height);
        let tint = [shadow.colour.0, shadow.colour.1, shadow.colour.2]
            .map(|channel| f32::from(channel) / 255.0);
        for (pixel, strength) in self.canvas.pixels.iter_mut().zip(&self.cast) {
            let alpha = (strength * shadow.alpha).min(1.0);
            if alpha >= SHADOW_CUTOFF {
                *pixel = [tint[0] * alpha, tint[1] * alpha, tint[2] * alpha, alpha];
            }
        }
    }
}

pub fn shadow_pad(scale: f32) -> usize {
    (SHADOW_REACH * scale).ceil() as usize
}

fn box_blur(
    source: &[f32],
    target: &mut [f32],
    (width, height): (usize, usize),
    radius: usize,
    horizontal: bool,
) {
    let (length, lines, stride) = if horizontal {
        (width, height, 1)
    } else {
        (height, width, width)
    };
    let window = (2 * radius + 1) as f32;
    for line in 0..lines {
        let first = if horizontal { line * width } else { line };
        let at = |along: usize| first + along * stride;
        let mut sum: f32 = (0..=radius.min(length - 1))
            .map(|step| source[at(step)])
            .sum();
        for along in 0..length {
            target[at(along)] = sum / window;
            if along + radius + 1 < length {
                sum += source[at(along + radius + 1)];
            }
            if along >= radius {
                sum -= source[at(along - radius)];
            }
        }
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
    palette: &Palette,
) {
    const CENTRE: [f32; 2] = [5.0, 5.0];
    let opacity = if mark.snoozed { SNOOZED_OPACITY } else { 1.0 };
    let animated = !reduced_motion && !mark.snoozed;
    let mut shape = |colour: Rgb, opacity: f32, distance: &dyn Fn(f32, f32) -> f32| {
        paint_shape(canvas, origin, MARK_SIZE, scale, colour, opacity, distance)
    };
    match mark.kind {
        MarkKind::Starting => shape(palette.dormant, opacity, &|x, y| circle(x, y, CENTRE, 2.0)),
        MarkKind::Working if animated => {
            let start = phase / WORKING_PERIOD * TAU;
            shape(palette.ink, opacity, &|x, y| {
                arc(x, y, CENTRE, 4.0, 1.5, start, 0.75 * TAU)
            })
        }
        MarkKind::Working => shape(palette.ink, opacity, &|x, y| ring(x, y, CENTRE, 4.0, 1.5)),
        MarkKind::NeedsInput => {
            if animated {
                let progress = Curve::Settle.at(f64::from((phase / HALO_PERIOD).fract())) as f32;
                shape(palette.wip, 1.0 - progress, &|x, y| {
                    ring(x, y, CENTRE, 4.5 + 2.5 * progress, 1.25)
                });
            }
            shape(palette.wip, opacity, &|x, y| circle(x, y, CENTRE, 4.5))
        }
        MarkKind::Stalled => shape(palette.wip, opacity, &|x, y| {
            rounded_box(x, y, [2.75, 5.0, 1.25, 4.0], 0.0).min(rounded_box(
                x,
                y,
                [7.25, 5.0, 1.25, 4.0],
                0.0,
            ))
        }),
        MarkKind::Done => shape(palette.live, opacity, &|x, y| {
            segment(x, y, [1.5, 5.2], [4.0, 7.6], 1.75).min(segment(
                x,
                y,
                [4.0, 7.6],
                [8.5, 2.6],
                1.75,
            ))
        }),
        MarkKind::Error => shape(palette.danger, opacity, &|x, y| {
            triangle(x, y, [[0.5, 9.0], [9.5, 9.0], [5.0, 1.0]])
        }),
        MarkKind::Idle => shape(palette.muted, opacity, &|x, y| {
            ring(x, y, CENTRE, 3.5, 1.25)
        }),
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
            chevron: Chevron::default(),
            look: Look::default(),
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
    let palette = motion.look.theme.palette();
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
        motion.width.unwrap_or(
            (content_end + (CHEVRON_GAP + CHEVRON_SIZE + PADDING) * scale).round() as usize,
        ),
        (BAR_HEIGHT * scale).round() as usize,
    );
    let radius = canvas.height as f32 / 2.0;
    if motion.look.solid {
        canvas.paint_bar(scale, radius, &palette);
    } else {
        canvas.paint_hit_bar(radius);
    }
    let top = (BAR_HEIGHT - MARK_SIZE) / 2.0 * scale;
    if model.marks.is_empty() {
        paint_shape(
            canvas,
            [PADDING * scale, top],
            MARK_SIZE,
            scale,
            palette.dormant,
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
            &palette,
        );
    }
    if let Some(mask) = &overflow {
        canvas.paint_mask(mask, overflow_left, palette.muted);
    }
    if let Some(mask) = &count {
        canvas.paint_mask(mask, count_left, palette.wip);
    }
    let [left, top, ..] = chevron_box(canvas.width, scale);
    let angle = motion.chevron.turn * PI;
    let (sin, cos) = angle.sin_cos();
    paint_shape(
        canvas,
        [left, top],
        CHEVRON_SIZE,
        scale,
        if motion.chevron.hot {
            palette.ink
        } else {
            palette.muted
        },
        1.0,
        |x, y| {
            let (dx, dy) = (x - MARK_CENTRE, y - MARK_CENTRE);
            let (x, y) = (
                MARK_CENTRE + dx * cos + dy * sin,
                MARK_CENTRE - dx * sin + dy * cos,
            );
            segment(x, y, [2.0, 3.5], [5.0, 6.5], 1.5).min(segment(
                x,
                y,
                [5.0, 6.5],
                [8.0, 3.5],
                1.5,
            ))
        },
    );
}

fn chevron_box(width: usize, scale: f32) -> [f32; 4] {
    let right = width as f32 - PADDING * scale;
    let top = (BAR_HEIGHT - CHEVRON_SIZE) / 2.0 * scale;
    [
        right - CHEVRON_SIZE * scale,
        top,
        right,
        top + CHEVRON_SIZE * scale,
    ]
}

pub fn on_chevron((x, y): (f32, f32), width: usize, scale: f32) -> bool {
    let [left, ..] = chevron_box(width, scale);
    x >= left - CHEVRON_GAP / 2.0 * scale && x < width as f32 && y >= 0.0 && y < BAR_HEIGHT * scale
}

pub fn tone_paint(tone: TabTone, theme: Theme) -> Rgb {
    let palette = theme.palette();
    match tone {
        TabTone::NeedsYou => palette.wip,
        TabTone::Failed => palette.danger,
        TabTone::Finished => palette.live,
        TabTone::Working => palette.ink,
        TabTone::Dormant => palette.dormant,
    }
}

pub fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Rgb(
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

pub fn paint_tab(
    canvas: &mut Canvas,
    (width, height): (usize, usize),
    edge: Edge,
    colour: Rgb,
    (thickness, glow): (f32, f32),
    scale: f32,
    theme: Theme,
) {
    canvas.reset(width, height);
    let halo = if theme == Theme::Dark {
        Rgb(0, 0, 0)
    } else {
        Rgb(0xff, 0xff, 0xff)
    };
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
                let coverage = glow * GLOW_ALPHA * fade * fade * ends;
                if coverage > 0.0 {
                    canvas.blend(x, y, colour, coverage);
                }
            }
            let distance = rounded_box(along, depth, strip, radius);
            let outline = (0.5 - distance + scale).clamp(0.0, 1.0);
            if outline > 0.0 {
                canvas.blend(x, y, halo, outline * TAB_HALO);
            }
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            if coverage > 0.0 {
                canvas.blend(x, y, colour, coverage);
            }
        }
    }
}

pub fn paint_hit_area(canvas: &mut Canvas, (width, height): (usize, usize)) {
    canvas.reset(width, height);
    canvas.pixels.fill([0.0, 0.0, 0.0, TAB_HIT_ALPHA]);
}

pub fn paint_peek(
    canvas: &mut Canvas,
    label: &str,
    colour: Rgb,
    scale: f32,
    text: &dyn TextRasterizer,
    theme: Theme,
) {
    let palette = theme.palette();
    let mask = text.rasterize(label, PEEK_FONT_SIZE * scale);
    let text_left = (PEEK_PADDING + PEEK_DOT + PEEK_DOT_GAP) * scale;
    canvas.reset(
        (text_left + mask.width as f32 + PEEK_PADDING * scale).round() as usize,
        (PEEK_HEIGHT * scale).round() as usize,
    );
    canvas.paint_bar(scale, PEEK_RADIUS * scale, &palette);
    let (centre, radius) = (
        [
            (PEEK_PADDING + PEEK_DOT / 2.0) * scale,
            canvas.height as f32 / 2.0,
        ],
        PEEK_DOT / 2.0 * scale,
    );
    canvas.paint(
        [
            centre[0] - radius - 1.0,
            centre[1] - radius - 1.0,
            centre[0] + radius + 1.0,
            centre[1] + radius + 1.0,
        ],
        colour,
        1.0,
        |x, y| circle(x, y, centre, radius),
    );
    canvas.paint_mask(&mask, text_left, palette.ink);
}

pub fn render_tray(state: TrayState, size: usize, theme: Theme) -> Canvas {
    const CORE: [f32; 2] = [8.0, 8.0];
    const BADGE: [f32; 2] = [13.0, 3.0];
    let palette = theme.palette();
    let mut canvas = Canvas::new(size, size);
    let unit = size as f32 / TRAY_UNITS;
    let mut shape = |alpha: f32, distance: &dyn Fn(f32, f32) -> f32| {
        let colour = if matches!(state, TrayState::Quiet) {
            palette.muted
        } else {
            palette.ink
        };
        paint_shape(
            &mut canvas,
            [0.0, 0.0],
            TRAY_UNITS,
            unit,
            colour,
            alpha,
            distance,
        )
    };
    let badged = matches!(state, TrayState::Alerting(_) | TrayState::Error);
    let clear_of_badge = |distance: f32, x: f32, y: f32| {
        if badged {
            distance.max(-circle(x, y, BADGE, 3.6))
        } else {
            distance
        }
    };
    shape(0.45, &|x, y| {
        clear_of_badge(ring(x, y, CORE, 6.8, 1.2), x, y)
    });
    shape(0.8, &|x, y| {
        clear_of_badge(ring(x, y, CORE, 4.4, 1.5), x, y)
    });
    shape(1.0, &|x, y| circle(x, y, CORE, 2.3));
    if badged {
        shape(1.0, &|x, y| circle(x, y, BADGE, 2.5));
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
            assert_eq!(widths[0], (46.0 * scale) as usize);
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
            assert_eq!(quiet.width, (46.0 * scale) as usize);
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
        assert_eq!(canvas.width, 46);
        assert!(close_to(straight(&canvas, 18, 14), DORMANT, 2));
        assert!(close_to(straight(&canvas, 15, 14), SURFACE, 1));
    }

    #[test]
    fn the_chevron_points_down_closed_and_up_open_in_muted_or_ink() {
        for scale in SCALES {
            let paint = |chevron: Chevron| {
                let mut canvas = Canvas::default();
                paint_pill(
                    &mut canvas,
                    &model(&[MarkKind::Idle], 0),
                    scale,
                    0.0,
                    true,
                    &BlockText,
                    &PillMotion {
                        growths: &[],
                        width: None,
                        chevron,
                        look: Look::default(),
                    },
                );
                canvas
            };
            let at = |canvas: &Canvas, [x, y]: [f32; 2]| {
                straight(
                    canvas,
                    ((26.0 + x) * scale) as usize,
                    ((9.0 + y) * scale) as usize,
                )
            };
            let (below, above) = ([5.0, 6.3], [5.0, 3.7]);
            let closed = paint(Chevron::default());
            assert_eq!(closed.width, (46.0 * scale) as usize);
            let red = |canvas: &Canvas, point: [f32; 2]| at(canvas, point)[0];
            assert!(
                (100..=MUTED.0).contains(&red(&closed, below)),
                "scale {scale}"
            );
            assert!(red(&closed, above) < 30, "scale {scale}");
            let open = paint(Chevron {
                turn: 1.0,
                hot: true,
            });
            assert!(red(&open, above) > 180, "scale {scale}");
            assert!(red(&open, below) < 30, "scale {scale}");
        }
    }

    #[test]
    fn only_the_chevron_end_of_the_pill_takes_a_press_for_the_panel() {
        for scale in SCALES {
            let width = (46.0 * scale) as usize;
            let lp = |value: f32| value * scale;
            assert!(on_chevron((lp(35.0), lp(14.0)), width, scale));
            assert!(on_chevron((lp(28.0), lp(2.0)), width, scale));
            assert!(on_chevron((lp(45.0), lp(27.0)), width, scale));
            assert!(!on_chevron((lp(22.0), lp(14.0)), width, scale));
            assert!(!on_chevron((lp(15.0), lp(14.0)), width, scale));
            assert!(!on_chevron((lp(35.0), lp(28.5)), width, scale));
            assert!(!on_chevron((lp(46.5), lp(14.0)), width, scale));
        }
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
                WIP,
                (4.0, 0.0),
                scale,
                Theme::Dark,
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
                DORMANT,
                (4.0, 0.0),
                scale,
                Theme::Dark,
            );
            let bottom = straight(&canvas, middle, lp(8.0) - 1);
            assert!(close_to(bottom, DORMANT, 1) && bottom[3] == to_byte(1.0));
            assert_eq!(straight(&canvas, middle, 0)[3], 1);
            paint_tab(
                &mut canvas,
                (lp(18.0), lp(72.0)),
                Edge::Right,
                LIVE,
                (6.0, 1.0),
                scale,
                Theme::Dark,
            );
            assert!(close_to(straight(&canvas, lp(18.0) - 1, middle), LIVE, 1));
            assert!(straight(&canvas, lp(8.0), middle)[3] > 1);
            assert_eq!(straight(&canvas, 0, middle)[3], 0);
        }
    }

    #[test]
    fn tones_follow_the_palette_and_mix_smoothly() {
        assert_eq!(tone_paint(TabTone::NeedsYou, Theme::Dark), WIP);
        assert_eq!(tone_paint(TabTone::Working, Theme::Dark), INK);
        assert_eq!(tone_paint(TabTone::Dormant, Theme::Dark), DORMANT);
        assert_eq!(
            tone_paint(TabTone::NeedsYou, Theme::Light),
            Rgb(0xa8, 0x74, 0x1c)
        );
        assert_eq!(
            tone_paint(TabTone::Working, Theme::Light),
            Rgb(0x18, 0x18, 0x1b)
        );
        assert_eq!(mix(WIP, LIVE, 0.0), WIP);
        assert_eq!(mix(WIP, LIVE, 1.0), LIVE);
        assert_eq!(mix(Rgb(0, 0, 0), Rgb(200, 100, 50), 0.5), Rgb(100, 50, 25));
    }

    #[test]
    fn a_growing_mark_starts_small_and_an_animated_width_overrides_the_layout() {
        let model = model(&[MarkKind::NeedsInput], 0);
        let mut canvas = Canvas::default();
        let motion = |growths, width| PillMotion {
            growths,
            width,
            chevron: Chevron::default(),
            look: Look::default(),
        };
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
        assert_eq!(canvas.width, 46);
        assert!(close_to(straight(&canvas, 11, 14), WIP, 2));
    }

    #[test]
    fn the_peek_is_a_bordered_surface_with_a_state_dot_and_ink_text() {
        for scale in SCALES {
            let lp = |value: f32| (value * scale) as usize;
            let mut canvas = Canvas::default();
            paint_peek(&mut canvas, "abcd", LIVE, scale, &BlockText, Theme::Dark);
            let text_width = (4.0 * 12.0 * scale * 0.6).round();
            assert_eq!(canvas.height, lp(26.0));
            assert_eq!(
                canvas.width,
                ((24.0 * scale + text_width) + 10.0 * scale).round() as usize
            );
            assert_eq!(straight(&canvas, 0, 0)[3], 0, "rounded corner");
            let middle = canvas.height / 2;
            assert!(close_to(straight(&canvas, lp(13.0), middle), LIVE, 2));
            assert!(close_to(straight(&canvas, lp(20.0), middle), SURFACE, 1));
            assert_eq!(straight(&canvas, lp(20.0), middle)[3], 245);
            assert!(close_to(straight(&canvas, lp(30.0), middle), INK, 2));
            assert!(close_to(straight(&canvas, lp(20.0), 0), LINE, 2));
        }
        let mut hit = Canvas::default();
        paint_hit_area(&mut hit, (3, 2));
        assert!(hit.straight_rgba().chunks(4).all(|pixel| pixel[3] == 1));
    }

    #[test]
    fn tray_icons_are_a_monochrome_core_with_two_rings_and_a_badge_when_needed() {
        let light = Theme::Light.palette();
        for size in [16, 24, 32] {
            let unit = size as f32 / 16.0;
            let at = |state: TrayState, theme: Theme, x: f32, y: f32| {
                straight(
                    &render_tray(state, size, theme),
                    (x * unit) as usize,
                    (y * unit) as usize,
                )
            };
            let core = |state: TrayState, theme: Theme| at(state, theme, 8.0, 8.0);
            assert!(close_to(core(TrayState::Quiet, Theme::Dark), MUTED, 3));
            assert!(close_to(core(TrayState::Working, Theme::Dark), INK, 3));
            assert!(close_to(core(TrayState::Alerting(3), Theme::Dark), INK, 3));
            assert!(close_to(core(TrayState::Error, Theme::Dark), INK, 3));
            assert!(close_to(
                core(TrayState::Working, Theme::Light),
                light.ink,
                3
            ));
            for state in [TrayState::Alerting(1), TrayState::Error] {
                let badge = at(state, Theme::Dark, 13.0, 3.0);
                assert!(close_to(badge, INK, 3) && badge[3] == 255, "{size}");
            }
            assert!(at(TrayState::Working, Theme::Dark, 13.0, 3.0)[3] < 128);
            assert_eq!(at(TrayState::Quiet, Theme::Dark, 0.0, 0.0)[3], 0);
            assert_eq!(at(TrayState::Working, Theme::Dark, 15.5, 15.5)[3], 0);
        }
    }

    fn painted(kinds: &[MarkKind], scale: f32, look: Look) -> Canvas {
        let mut canvas = Canvas::default();
        paint_pill(
            &mut canvas,
            &model(kinds, 0),
            scale,
            0.0,
            true,
            &BlockText,
            &PillMotion {
                growths: &[],
                width: None,
                chevron: Chevron::default(),
                look,
            },
        );
        canvas
    }

    #[test]
    fn the_light_theme_draws_a_white_capsule_with_ink_marks_and_a_light_border() {
        let light = Look {
            theme: Theme::Light,
            solid: true,
        };
        for scale in SCALES {
            let canvas = painted(&[MarkKind::Working, MarkKind::Idle], scale, light);
            let inside = straight(&canvas, (24.0 * scale) as usize, canvas.height / 2);
            assert!(close_to(inside, Rgb(0xff, 0xff, 0xff), 1), "{inside:?}");
            assert_eq!(inside[3], 245);
            let border = straight(&canvas, (24.0 * scale) as usize, 0);
            assert!(close_to(border, Rgb(0xdc, 0xdc, 0xe2), 2), "{border:?}");
            let ring = straight(
                &canvas,
                ((10.0 + 8.5) * scale) as usize,
                ((9.0 + 6.5) * scale) as usize,
            );
            assert!(close_to(ring, Rgb(0x18, 0x18, 0x1b), 2), "{ring:?}");
        }
    }

    #[test]
    fn a_lifted_capsule_keeps_its_pixels_and_gains_a_soft_shadow_that_sits_lower() {
        for scale in SCALES {
            let capsule = painted(&[MarkKind::Idle], scale, Look::default());
            let mut lifted = Canvas::default();
            capsule.lift_into(&mut lifted, scale, Look::default());
            let pad = shadow_pad(scale);
            assert_eq!(lifted.width, capsule.width + 2 * pad);
            assert_eq!(lifted.height, capsule.height + 2 * pad);
            let middle = capsule.width / 2;
            let kept = straight(&lifted, middle + pad, capsule.height / 2 + pad);
            let original = straight(&capsule, middle, capsule.height / 2);
            assert!(close_to(kept, SURFACE, 1) && kept[3].abs_diff(original[3]) <= 6);
            let (above, below) = (
                straight(&lifted, middle + pad, pad - 2)[3],
                straight(&lifted, middle + pad, pad + capsule.height + 1)[3],
            );
            assert!(below > above && below > 10, "{above} {below}");
            assert_eq!(straight(&lifted, 0, 0)[3], 0);
            assert_eq!(straight(&lifted, middle + pad, 0)[3], 0);
        }
    }

    #[test]
    fn without_a_background_only_a_hit_area_and_the_marks_are_drawn() {
        let clear = Look {
            theme: Theme::Dark,
            solid: false,
        };
        for scale in SCALES {
            let canvas = painted(&[MarkKind::NeedsInput, MarkKind::Idle], scale, clear);
            let gap = straight(&canvas, (23.0 * scale) as usize, canvas.height / 2);
            assert_eq!(gap[3], 1, "scale {scale}");
            assert_eq!(straight(&canvas, 0, 0)[3], 0);
            let dot = straight(&canvas, (15.0 * scale) as usize, (14.0 * scale) as usize);
            assert!(close_to(dot, WIP, 2) && dot[3] == 255);
            let mut lifted = Canvas::default();
            canvas.lift_into(&mut lifted, scale, clear);
            let pad = shadow_pad(scale);
            let under = straight(
                &lifted,
                (15.0 * scale) as usize + pad,
                pad + (21.0 * scale) as usize,
            );
            assert!(under[3] > 0, "the marks cast a shadow");
        }
    }

    #[test]
    fn the_sliding_box_blur_matches_a_plain_window_sum() {
        let (width, height) = (9, 7);
        let source: Vec<f32> = (0..width * height)
            .map(|index| ((index * 37) % 11) as f32 / 10.0)
            .collect();
        for radius in [1, 2, 5, 12] {
            for horizontal in [true, false] {
                let mut blurred = vec![0.0; width * height];
                box_blur(&source, &mut blurred, (width, height), radius, horizontal);
                for y in 0..height {
                    for x in 0..width {
                        let (along, length) = if horizontal { (x, width) } else { (y, height) };
                        let expected: f32 = (along.saturating_sub(radius)
                            ..=(along + radius).min(length - 1))
                            .map(|step| {
                                if horizontal {
                                    source[y * width + step]
                                } else {
                                    source[step * width + x]
                                }
                            })
                            .sum::<f32>()
                            / (2 * radius + 1) as f32;
                        assert!(
                            (blurred[y * width + x] - expected).abs() < 1e-5,
                            "radius {radius} horizontal {horizontal} at {x},{y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_solid_capsule_shadow_is_reused_for_the_same_size_and_a_clear_one_follows_its_marks() {
        let solid = Look::default();
        let clear = Look {
            theme: Theme::Dark,
            solid: false,
        };
        let capsule = painted(&[MarkKind::Idle], 1.0, solid);
        let mut emptied = Canvas::default();
        emptied.reset(capsule.width, capsule.height);
        let alphas = |shadow: &CastShadow| -> Vec<u8> {
            shadow
                .canvas
                .straight_rgba()
                .chunks(4)
                .map(|p| p[3])
                .collect()
        };
        let mut shadow = CastShadow::default();
        shadow.cast(&capsule, 1.0, solid);
        let first = alphas(&shadow);
        assert!(first.iter().any(|&alpha| alpha > 0));
        shadow.cast(&emptied, 1.0, solid);
        assert_eq!(alphas(&shadow), first);
        shadow.cast(&emptied, 1.5, solid);
        assert_ne!(alphas(&shadow), first);
        let mut marks = CastShadow::default();
        marks.cast(&capsule, 1.0, clear);
        let drawn = alphas(&marks);
        marks.cast(&emptied, 1.0, clear);
        assert_ne!(alphas(&marks), drawn);
        assert!(alphas(&marks).iter().all(|&alpha| alpha == 0));
    }
}
