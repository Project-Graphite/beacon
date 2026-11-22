use super::frames::{self, WM_FRAME};
use super::layer::Layer;
use super::{GdiText, cursor_position, monitor_info, monitors, reduced_motion, refresh_rate};
use crate::displays::{Position, dock, landing, target};
use crate::geometry::{
    Corner, Edge, Monitor, Rect, TabPlacement, corner_rect, default_placement, layout_signature,
    monitor_for, nearest_corner, peek_rect, pill_edge, pill_reveal_strip, resize_anchored,
    restored_pill_rect, slide, snap, tab_rect,
};
use crate::model::{Peek, PillModel, TabTone};
use crate::motion::{
    Attention, Curve, Strip, Tween, next_burst, peek_extent, peek_moving, peek_next_change,
    unseen_moving, unseen_strip,
};
use crate::positions::{self, AttentionStyle, Style, UiSettings};
use crate::raster::{
    Canvas, PillMotion, Rgb, mix, paint_hit_area, paint_peek, paint_pill, paint_tab, render_pill,
    tone_paint,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::io;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MonitorFromWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows_sys::Win32::UI::Controls::{WM_MOUSEHOVER, WM_MOUSELEAVE};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_HOVER, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DefWindowProcW, EVENT_SYSTEM_FOREGROUND, GetClassNameW, GetForegroundWindow,
    GetWindowRect, HC_ACTION, HHOOK, IDC_ARROW, IsZoomed, KillTimer, LoadCursorW, MA_NOACTIVATE,
    MSLLHOOKSTRUCT, PostMessageW, RegisterClassExW, SPI_SETWORKAREA, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, SetTimer, SetWindowPos, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP, WM_CAPTURECHANGED, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_RBUTTONDOWN, WM_SETTINGCHANGE, WM_TIMER, WM_XBUTTONDOWN, WNDCLASSEXW,
};

const CLASS: &str = "BeaconPill";
const HOVER_TIMER: usize = 2;
const WAKE_TIMER: usize = 3;
const TICK_MS: u32 = 100;
const HOVER_MS: u32 = 150;
const REVEAL_HOVER_MS: u32 = 200;
const CLICK_SLOP: f64 = 4.0;
const WM_FOREGROUND_CHANGED: u32 = WM_APP + 1;
const WM_POINTER_PRESSED: u32 = WM_APP + 2;
const DESKTOP_CLASSES: [&str; 2] = ["Progman", "WorkerW"];
const FADE: f64 = 0.16;
const TONE_FADE: f64 = 0.2;
const OPACITY_FADE: f64 = 0.2;
const HOVER_GROW: f64 = 0.12;
const GLIDE: f64 = 0.18;
const MARK_POP: f64 = 0.18;
const WIDTH_GLIDE: f64 = 0.18;
const PHASE_WRAP: f64 = 4.8;
const RESTING_STRIP: f64 = 4.0;
const HOVERED_STRIP: f64 = 6.0;
const STILL_DEPTH: f64 = 8.0;
const PULSING_DEPTH: f64 = 18.0;
const REVEAL_DEPTH: f64 = 2.0;
const PEEK_SLIDE: f64 = 12.0;

static WINDOW: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillEvent {
    Entered,
    Hovered,
    Clicked,
    Moved,
    HoverTick,
    ForegroundChanged,
    Pressed((i32, i32)),
    RevealHovered,
    PeekPressed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TabModel {
    pub tone: TabTone,
    pub attention_since: Option<Instant>,
    pub alerting: bool,
}

struct Press {
    cursor: (i32, i32),
    origin: Rect,
    placement: TabPlacement,
    dragging: bool,
}

struct Glide {
    from: (i32, i32),
    progress: Tween,
}

struct ShownPeek {
    canvas: Canvas,
    started: f64,
}

struct Pill {
    hwnd: HWND,
    home: PathBuf,
    style: Style,
    settings: UiSettings,
    model: PillModel,
    tab: TabModel,
    scale: f64,
    rect: Rect,
    work: Rect,
    bounds: Rect,
    display: String,
    placement: TabPlacement,
    user_visible: bool,
    revealed: bool,
    reveal_wanted: bool,
    targeted: bool,
    fullscreen: bool,
    shown: bool,
    reduced_motion: bool,
    hovering: bool,
    hover_polling: bool,
    tracking: bool,
    reveal_tracking: bool,
    pressed_since_entry: bool,
    press: Option<Press>,
    started: Instant,
    text: GdiText,
    on_event: Rc<dyn Fn(PillEvent)>,
    refresh_rate: u32,
    framing: bool,
    fade: Tween,
    opacity: Tween,
    paint_from: (Rgb, f32),
    tone_fade: Tween,
    strip: Tween,
    glide: Option<Glide>,
    mark_pops: Vec<f64>,
    growths: Vec<f32>,
    width: Tween,
    canvas: Canvas,
    layer: Layer,
    reveal: Layer,
    reveal_rect: Option<Rect>,
    hit_canvas: Canvas,
    peek: Option<ShownPeek>,
    peek_layer: Layer,
    press_hook: HHOOK,
}

thread_local! {
    static PILL: RefCell<Option<Pill>> = const { RefCell::new(None) };
}

fn with_pill<R>(action: impl FnOnce(&mut Pill) -> R) -> Option<R> {
    PILL.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(action))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub fn create(
    home: PathBuf,
    visible: bool,
    on_event: impl Fn(PillEvent) + 'static,
) -> io::Result<()> {
    let class = wide(CLASS);
    unsafe {
        let window_class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: GetModuleHandleW(null()),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassExW(&window_class) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let layer = Layer::create(&class)?;
    let hwnd = layer.hwnd;
    WINDOW.store(hwnd, Ordering::Relaxed);
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    let settings = positions::ui_settings(&home);
    let mut pill = Pill {
        hwnd,
        style: positions::style(&home),
        opacity: Tween::rest(settings.idle_opacity),
        settings,
        home,
        model: PillModel::default(),
        tab: TabModel::default(),
        scale: 1.0,
        rect: Rect::default(),
        work: Rect::default(),
        bounds: Rect::default(),
        display: String::new(),
        placement: TabPlacement {
            edge: Edge::Right,
            offset: 0.5,
        },
        user_visible: visible,
        revealed: false,
        reveal_wanted: false,
        targeted: false,
        fullscreen: false,
        shown: false,
        reduced_motion: reduced_motion(),
        hovering: false,
        hover_polling: false,
        tracking: false,
        reveal_tracking: false,
        pressed_since_entry: false,
        press: None,
        started: Instant::now(),
        text: GdiText::default(),
        on_event: Rc::new(on_event),
        refresh_rate: 0,
        framing: false,
        fade: Tween::rest(0.0),
        paint_from: tone_paint(TabTone::default()),
        tone_fade: Tween::rest(1.0),
        strip: Tween::rest(RESTING_STRIP),
        glide: None,
        mark_pops: Vec::new(),
        growths: Vec::new(),
        width: Tween::rest(0.0),
        canvas: Canvas::default(),
        layer,
        reveal: Layer::create(&class)?,
        reveal_rect: None,
        hit_canvas: Canvas::default(),
        peek: None,
        peek_layer: Layer::create(&class)?,
        press_hook: null_mut(),
    };
    pill.relocate();
    pill.fullscreen = foreground_is_fullscreen(hwnd);
    pill.sync();
    PILL.with(|cell| cell.replace(Some(pill)));
    Ok(())
}

pub fn show_model(model: PillModel) {
    with_pill(|pill| {
        if pill.model != model {
            let start = pill.animation_start();
            pill.mark_pops = model
                .marks
                .iter()
                .enumerate()
                .map(|(index, mark)| match pill.model.marks.get(index) {
                    Some(previous) if previous.kind == mark.kind => pill
                        .mark_pops
                        .get(index)
                        .copied()
                        .unwrap_or(f64::NEG_INFINITY),
                    _ => start,
                })
                .collect();
            pill.model = model;
            let now = pill.now();
            let width = f64::from(pill.natural_size().width());
            let duration = pill.duration(WIDTH_GLIDE);
            pill.width.head_to(width, now, duration, Curve::Settle);
            pill.present();
            pill.schedule();
        }
    });
}

pub fn show_tab(tab: TabModel) {
    with_pill(|pill| {
        if pill.tab == tab {
            return;
        }
        let now = pill.now();
        if tab.tone != pill.tab.tone {
            pill.paint_from = pill.tone_paint(now);
            pill.tone_fade = Tween::between(0.0, 1.0, now, pill.duration(TONE_FADE), Curve::Settle);
        }
        if let (Some(attention), None) = (pill.attention(now), tab.attention_since) {
            let from = unseen_strip(attention, pill.reduced_motion).thickness;
            pill.strip = Tween::between(
                from,
                pill.resting_strip(),
                now,
                pill.duration(HOVER_GROW),
                Curve::Settle,
            );
        }
        pill.tab = tab;
        pill.fade_opacity();
        pill.present();
        pill.schedule();
    });
}

pub fn configure(settings: UiSettings) {
    with_pill(|pill| {
        if pill.settings == settings {
            return;
        }
        pill.settings = settings;
        pill.width = Tween::rest(f64::from(pill.natural_size().width()));
        pill.fade_opacity();
        pill.sync();
    });
}

pub fn reveal(revealed: bool, reveal_wanted: bool) {
    with_pill(|pill| {
        if (pill.revealed, pill.reveal_wanted) != (revealed, reveal_wanted) {
            (pill.revealed, pill.reveal_wanted) = (revealed, reveal_wanted);
            pill.sync();
        }
    });
}

pub fn peek(peek: Peek) {
    with_pill(|pill| {
        if !pill.may_show() {
            return;
        }
        let mut canvas = Canvas::default();
        paint_peek(
            &mut canvas,
            &peek.text,
            tone_paint(peek.tone).0,
            pill.scale as f32,
            &pill.text,
        );
        pill.peek = Some(ShownPeek {
            canvas,
            started: pill.now(),
        });
        pill.present();
        pill.schedule();
    });
}

pub fn dismiss_peek() {
    with_pill(|pill| {
        if pill.peek.take().is_some() {
            pill.present();
            pill.schedule();
        }
    });
}

pub fn set_user_visible(visible: bool) {
    with_pill(|pill| {
        pill.user_visible = visible;
        pill.sync();
    });
}

pub fn user_visible() -> bool {
    with_pill(|pill| pill.user_visible).unwrap_or(false)
}

pub fn set_style(style: Style) {
    with_pill(|pill| {
        pill.style = style;
        pill.reappear();
    });
}

pub fn retarget() {
    with_pill(Pill::reappear);
}

pub fn move_to_corner(corner: Corner) {
    with_pill(|pill| {
        let monitors = monitors();
        if let Some(target) = target(&monitors, &positions::display_choice(&pill.home)) {
            let placed = corner_rect(pill.rect, corner, target);
            pill.move_to(placed.left, placed.top);
            pill.save(&monitors, &target.id);
        }
    });
}

pub fn move_to_edge(edge: Edge) {
    with_pill(|pill| {
        if !pill.targeted {
            return;
        }
        let from = pill.rect;
        pill.placement = TabPlacement { edge, offset: 0.5 };
        if let Err(error) = positions::save_tab(&pill.home, &pill.display, pill.placement) {
            eprintln!("cannot save the tab position: {error}");
        }
        pill.glide_from(from);
    });
}

pub fn window() -> HWND {
    WINDOW.load(Ordering::Relaxed)
}

pub fn shown_rect() -> Option<Rect> {
    with_pill(|pill| pill.shown.then_some(pill.rect)).flatten()
}

pub fn reveal_rect() -> Option<Rect> {
    with_pill(|pill| pill.reveal_rect).flatten()
}

pub fn shown_tab() -> Option<(Rect, Edge)> {
    with_pill(|pill| {
        (pill.shown && pill.style == Style::Tab).then_some((pill.rect, pill.placement.edge))
    })
    .flatten()
}

pub fn position() -> Option<Position> {
    with_pill(|pill| match pill.style {
        Style::Tab => Position::Edge(pill.placement.edge),
        Style::Pill => Position::Corner(nearest_corner(pill.rect, &pill.work)),
    })
}

pub fn fullscreen_in_front() -> bool {
    with_pill(|pill| pill.fullscreen).unwrap_or(false)
}

pub fn anchor_edge() -> Option<Edge> {
    with_pill(|pill| {
        pill.shown.then(|| match pill.style {
            Style::Tab => pill.placement.edge,
            Style::Pill => pill_edge(pill.rect, &pill.work),
        })
    })
    .flatten()
}

pub fn set_hover_polling(polling: bool) {
    with_pill(|pill| {
        if pill.hover_polling != polling {
            pill.hover_polling = polling;
            unsafe {
                if polling {
                    SetTimer(pill.hwnd, HOVER_TIMER, TICK_MS, None);
                } else {
                    KillTimer(pill.hwnd, HOVER_TIMER);
                }
            }
        }
    });
}

pub fn watch_presses(watching: bool) {
    with_pill(|pill| {
        if watching == !pill.press_hook.is_null() {
            return;
        }
        if watching {
            pill.press_hook = unsafe {
                SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(pointer_pressed),
                    GetModuleHandleW(null()),
                    0,
                )
            };
            if pill.press_hook.is_null() {
                eprintln!(
                    "cannot watch for presses outside the panel: {}",
                    io::Error::last_os_error()
                );
            }
        } else {
            unsafe { UnhookWindowsHookEx(pill.press_hook) };
            pill.press_hook = null_mut();
        }
    });
}

fn track_hover(hwnd: HWND, hover_ms: u32) -> bool {
    let mut tracking = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_HOVER | TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: hover_ms,
    };
    unsafe { TrackMouseEvent(&mut tracking) != 0 }
}

impl Pill {
    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn at(&self, instant: Instant) -> f64 {
        instant
            .saturating_duration_since(self.started)
            .as_secs_f64()
    }

    fn duration(&self, seconds: f64) -> f64 {
        if self.reduced_motion { 0.0 } else { seconds }
    }

    fn animation_start(&self) -> f64 {
        if self.reduced_motion {
            f64::NEG_INFINITY
        } else {
            self.now()
        }
    }

    fn resting_strip(&self) -> f64 {
        if self.hovering {
            HOVERED_STRIP
        } else {
            RESTING_STRIP
        }
    }

    fn tone_paint(&self, now: f64) -> (Rgb, f32) {
        mix(
            self.paint_from,
            tone_paint(self.tab.tone),
            self.tone_fade.value(now) as f32,
        )
    }

    fn fade_opacity(&mut self) {
        let target = if self.tab.alerting {
            1.0
        } else {
            self.settings.idle_opacity
        };
        let (now, duration) = (self.now(), self.duration(OPACITY_FADE));
        self.opacity.head_to(target, now, duration, Curve::Settle);
    }

    fn attention(&self, now: f64) -> Option<Attention> {
        self.tab
            .attention_since
            .filter(|_| self.settings.attention_style != AttentionStyle::Colour)
            .map(|since| Attention {
                since_gain: now - self.at(since),
                repeat: self.settings.attention_repeat_seconds as f64,
            })
    }

    fn pill_scale(&self) -> f64 {
        self.scale * self.settings.size.pill_factor()
    }

    fn tab_rect(&self, depth: f64) -> Rect {
        tab_rect(
            &self.bounds,
            self.placement,
            self.settings.size.tab_length(),
            depth * self.settings.size.tab_factor(),
            self.scale,
        )
    }

    fn natural_size(&self) -> Rect {
        let canvas = render_pill(&self.model, self.pill_scale() as f32, 0.0, true, &self.text);
        Rect::sized(0, 0, canvas.width as i32, canvas.height as i32)
    }

    fn may_show(&self) -> bool {
        self.user_visible && self.targeted && !self.fullscreen
    }

    fn adopt(&mut self, monitor: &Monitor) {
        self.scale = monitor.scale;
        self.work = monitor.work;
        self.bounds = monitor.bounds;
        self.display = monitor.id.clone();
    }

    fn relocate(&mut self) {
        let monitors = monitors();
        let target = target(&monitors, &positions::display_choice(&self.home));
        self.targeted = target.is_some();
        if let Some(target) = target {
            self.adopt(target);
            self.placement = positions::load_tab(&self.home, &target.id)
                .unwrap_or_else(|| default_placement(target, &monitors));
            let saved = [
                positions::load_for_display(&self.home, &target.id),
                positions::load(&self.home, &layout_signature(&monitors)),
            ];
            let size = self.natural_size();
            self.rect = restored_pill_rect(saved, size, target, &monitors);
            self.width = Tween::rest(f64::from(size.width()));
        }
        self.glide = None;
        self.present();
        self.refresh_rate = refresh_rate(self.hwnd);
    }

    fn reappear(&mut self) {
        self.fade = Tween::rest(0.0);
        self.relocate();
        self.sync();
    }

    fn paint_pill(&mut self, now: f64) -> Rect {
        self.growths.clear();
        self.growths.extend(
            self.mark_pops
                .iter()
                .map(|start| Curve::Pop.at((now - start) / MARK_POP) as f32),
        );
        let scale = self.pill_scale() as f32;
        paint_pill(
            &mut self.canvas,
            &self.model,
            scale,
            (now % PHASE_WRAP) as f32,
            self.reduced_motion,
            &self.text,
            &PillMotion {
                growths: &self.growths,
                width: Some(self.width.value(now).round() as usize),
            },
        );
        resize_anchored(
            self.rect,
            self.canvas.width as i32,
            self.canvas.height as i32,
            &self.work,
        )
    }

    fn paint_tab(&mut self, now: f64) -> Rect {
        let attention = self.attention(now);
        let strip = match attention {
            Some(attention) => unseen_strip(attention, self.reduced_motion),
            None => Strip {
                thickness: self.strip.value(now),
                glow: 0.0,
            },
        };
        let depth = if attention.is_some() && !self.reduced_motion {
            PULSING_DEPTH
        } else {
            STILL_DEPTH
        };
        let docked = self.tab_rect(depth);
        let rect = match &self.glide {
            Some(glide) => {
                let progress = glide.progress.value(now);
                let between = |from: i32, to: i32| {
                    (f64::from(from) + f64::from(to - from) * progress).round() as i32
                };
                Rect::sized(
                    between(glide.from.0, docked.left),
                    between(glide.from.1, docked.top),
                    docked.width(),
                    docked.height(),
                )
            }
            None => docked,
        };
        let paint = self.tone_paint(now);
        paint_tab(
            &mut self.canvas,
            (rect.width() as usize, rect.height() as usize),
            self.placement.edge,
            paint,
            strip.thickness as f32,
            strip.glow as f32,
            (self.scale * self.settings.size.tab_factor()) as f32,
        );
        rect
    }

    fn present(&mut self) {
        let now = self.now();
        let rect = match self.style {
            Style::Pill => self.paint_pill(now),
            Style::Tab => self.paint_tab(now),
        };
        let alpha = (self.fade.value(now).clamp(0.0, 1.0)
            * self.opacity.value(now).clamp(0.0, 1.0)
            * 255.0)
            .round() as u8;
        self.layer.update(rect, &self.canvas, alpha);
        self.rect = rect;
        self.present_reveal();
        self.present_peek(now);
    }

    fn present_reveal(&mut self) {
        let strip = (self.reveal_wanted && self.may_show())
            .then(|| match self.style {
                Style::Tab => Some(self.tab_rect(REVEAL_DEPTH / self.settings.size.tab_factor())),
                Style::Pill => pill_reveal_strip(self.rect, &self.work, REVEAL_DEPTH, self.scale),
            })
            .flatten();
        if let Some(rect) = strip {
            paint_hit_area(
                &mut self.hit_canvas,
                (rect.width() as usize, rect.height() as usize),
            );
            self.reveal.update(rect, &self.hit_canvas, u8::MAX);
            self.reveal_rect = Some(rect);
        }
        self.reveal.show(strip.is_some());
    }

    fn present_peek(&mut self, now: f64) {
        let hold = self.settings.peek_seconds;
        let shown = self.peek.as_ref().and_then(|peek| {
            peek_extent(now - peek.started, hold, self.reduced_motion).map(|extent| (peek, extent))
        });
        let Some((peek, extent)) = shown else {
            self.peek = None;
            self.peek_layer.show(false);
            return;
        };
        let (anchor, edge) = match self.style {
            Style::Tab => (self.tab_rect(STILL_DEPTH), self.placement.edge),
            Style::Pill => (
                resize_anchored(
                    self.rect,
                    peek.canvas.width as i32,
                    self.rect.height(),
                    &self.work,
                ),
                pill_edge(self.rect, &self.work),
            ),
        };
        let rect = peek_rect(
            anchor,
            edge,
            (peek.canvas.width as i32, peek.canvas.height as i32),
            PEEK_SLIDE * (1.0 - extent),
            &self.work,
            self.scale,
        );
        self.peek_layer
            .update(rect, &peek.canvas, (extent * 255.0).round() as u8);
        self.peek_layer.show(true);
    }

    fn peek_moving(&self, now: f64) -> bool {
        self.peek.as_ref().is_some_and(|peek| {
            peek_moving(
                now - peek.started,
                self.settings.peek_seconds,
                self.reduced_motion,
            )
        })
    }

    fn moving(&self, now: f64) -> bool {
        self.opacity.moving(now)
            || match self.style {
                Style::Pill => {
                    self.model.animates(self.reduced_motion)
                        || self.width.moving(now)
                        || self.mark_pops.iter().any(|start| now - start < MARK_POP)
                }
                Style::Tab => {
                    self.tone_fade.moving(now)
                        || self.strip.moving(now)
                        || self.glide.is_some()
                        || (!self.reduced_motion && self.attention(now).is_some_and(unseen_moving))
                }
            }
    }

    fn schedule(&mut self) {
        let now = self.now();
        if self
            .glide
            .as_ref()
            .is_some_and(|glide| !glide.progress.moving(now))
        {
            self.glide = None;
            self.refresh_rate = refresh_rate(self.hwnd);
        }
        let fading_out = self.fade.target() == 0.0;
        if self.shown && fading_out && !self.fade.moving(now) {
            self.shown = false;
            self.layer.show(false);
        }
        let framing = (self.shown && (self.fade.moving(now) || (!fading_out && self.moving(now))))
            || self.peek_moving(now);
        if framing != self.framing {
            self.framing = framing;
            if framing {
                frames::run(self.hwnd, self.refresh_rate);
            } else {
                frames::stop();
            }
        }
        let burst_in = self
            .attention(now)
            .filter(|_| self.shown && !self.reduced_motion && self.style == Style::Tab)
            .and_then(|attention| next_burst(attention).map(|at| at - attention.since_gain));
        let peek_in = self.peek.as_ref().and_then(|peek| {
            peek_next_change(
                now - peek.started,
                self.settings.peek_seconds,
                self.reduced_motion,
            )
        });
        let wake_in = [burst_in, peek_in]
            .into_iter()
            .flatten()
            .filter(|_| !framing)
            .reduce(f64::min);
        unsafe {
            if let Some(wait) = wake_in {
                SetTimer(
                    self.hwnd,
                    WAKE_TIMER,
                    (wait * 1000.0).ceil().max(1.0) as u32,
                    None,
                );
            } else {
                KillTimer(self.hwnd, WAKE_TIMER);
            }
        }
    }

    fn sync(&mut self) {
        let now = self.now();
        let visible = self.may_show() && self.revealed;
        if !self.may_show() {
            self.peek = None;
        }
        let duration = self.duration(FADE);
        self.fade.head_to(
            if visible { 1.0 } else { 0.0 },
            now,
            duration,
            Curve::Settle,
        );
        self.present();
        if visible && !self.shown {
            self.shown = true;
            self.layer.show(true);
        }
        self.schedule();
    }

    fn hover(&mut self, hovering: bool) {
        if self.hovering != hovering {
            self.hovering = hovering;
            let (now, duration) = (self.now(), self.duration(HOVER_GROW));
            let resting = self.resting_strip();
            self.strip.head_to(resting, now, duration, Curve::Settle);
            self.present();
            self.schedule();
        }
    }

    fn glide_from(&mut self, from: Rect) {
        self.glide = (!self.reduced_motion).then(|| Glide {
            from: (from.left, from.top),
            progress: Tween::between(0.0, 1.0, self.now(), GLIDE, Curve::Pop),
        });
        self.present();
        self.schedule();
    }

    fn move_to(&mut self, left: i32, top: i32) {
        self.rect = Rect::sized(left, top, self.rect.width(), self.rect.height());
        unsafe {
            SetWindowPos(
                self.hwnd,
                null_mut(),
                left,
                top,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        let monitors = monitors();
        if let Some(monitor) = monitor_for(&self.rect, &monitors) {
            let rescaled = monitor.scale != self.scale;
            self.work = monitor.work;
            self.scale = monitor.scale;
            if rescaled {
                self.width = Tween::rest(f64::from(self.natural_size().width()));
                self.present();
                self.refresh_rate = refresh_rate(self.hwnd);
            }
        }
    }

    fn finish_drag(&mut self, origin: Rect) {
        let monitors = monitors();
        let Some(monitor) = landing(
            &self.rect,
            &monitors,
            &positions::display_choice(&self.home),
        ) else {
            return self.move_to(origin.left, origin.top);
        };
        let snapped = snap(self.rect, &monitor.work, monitor.scale);
        self.move_to(snapped.left, snapped.top);
        self.adopt(monitor);
        if let Err(error) = positions::choose_display(&self.home, &monitor.id) {
            eprintln!("cannot save the chosen display: {error}");
        }
        self.save(&monitors, &monitor.id);
    }

    fn finish_tab_drag(&mut self, origin: TabPlacement) {
        let monitors = monitors();
        let from = self.rect;
        let docked = monitors
            .iter()
            .find(|monitor| monitor.id == self.display)
            .and_then(|current| {
                dock(
                    cursor_position(),
                    current,
                    self.placement.edge,
                    &monitors,
                    &positions::display_choice(&self.home),
                )
            });
        match docked {
            Some((monitor, placement)) => {
                self.adopt(monitor);
                self.placement = placement;
                let saved = positions::save_tab(&self.home, &monitor.id, placement)
                    .and_then(|()| positions::choose_display(&self.home, &monitor.id));
                if let Err(error) = saved {
                    eprintln!("cannot save the tab position: {error}");
                }
            }
            None => self.placement = origin,
        }
        self.glide_from(from);
    }

    fn save(&self, monitors: &[Monitor], display: &str) {
        let saved = positions::save(&self.home, &layout_signature(monitors), self.rect)
            .and_then(|()| positions::save_for_display(&self.home, display, self.rect));
        if let Err(error) = saved {
            eprintln!("cannot save the pill position: {error}");
        }
    }

    fn drag(&mut self, press_cursor: (i32, i32), origin: Rect) {
        let (x, y) = cursor_position();
        match self.style {
            Style::Pill => self.move_to(
                origin.left + x - press_cursor.0,
                origin.top + y - press_cursor.1,
            ),
            Style::Tab => {
                self.placement = slide(&self.bounds, self.placement.edge, (x, y));
                self.present();
            }
        }
    }

    fn handle_reveal(&mut self, message: u32) -> Option<Option<PillEvent>> {
        match message {
            WM_MOUSEMOVE => {
                let entered = !self.reveal_tracking;
                if entered {
                    self.reveal_tracking = track_hover(self.reveal.hwnd, REVEAL_HOVER_MS);
                }
                Some(entered.then_some(PillEvent::Entered))
            }
            WM_MOUSEHOVER => {
                self.reveal_tracking = false;
                Some(Some(PillEvent::RevealHovered))
            }
            WM_MOUSELEAVE => {
                self.reveal_tracking = false;
                Some(None)
            }
            _ => None,
        }
    }

    fn handle(
        &mut self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<Option<PillEvent>> {
        if hwnd == self.reveal.hwnd {
            return self.handle_reveal(message);
        }
        if hwnd == self.peek_layer.hwnd {
            return (message == WM_LBUTTONDOWN).then_some(Some(PillEvent::PeekPressed));
        }
        match message {
            WM_MOUSEMOVE => {
                let entered = !self.tracking;
                if entered {
                    self.tracking = track_hover(self.hwnd, HOVER_MS);
                }
                self.hover(true);
                let slop = (CLICK_SLOP * self.scale) as i32;
                let dragged = self.press.as_mut().and_then(|press| {
                    let (x, y) = cursor_position();
                    let (dx, dy) = (x - press.cursor.0, y - press.cursor.1);
                    press.dragging |= dx.abs() > slop || dy.abs() > slop;
                    press.dragging.then_some((press.cursor, press.origin))
                });
                if let Some((cursor, origin)) = dragged {
                    self.drag(cursor, origin);
                }
                Some(entered.then_some(PillEvent::Entered))
            }
            WM_MOUSEHOVER => Some(
                (self.press.is_none() && !self.pressed_since_entry).then_some(PillEvent::Hovered),
            ),
            WM_MOUSELEAVE => {
                self.tracking = false;
                self.pressed_since_entry = false;
                if self.press.is_none() {
                    self.hover(false);
                }
                Some(None)
            }
            WM_LBUTTONDOWN => {
                unsafe { SetCapture(self.hwnd) };
                self.pressed_since_entry = true;
                self.press = Some(Press {
                    cursor: cursor_position(),
                    origin: self.rect,
                    placement: self.placement,
                    dragging: false,
                });
                Some(None)
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED => {
                let press = self.press.take();
                if message == WM_LBUTTONUP {
                    self.pressed_since_entry = true;
                    unsafe { ReleaseCapture() };
                }
                Some(match press {
                    Some(press) if press.dragging => {
                        self.hover(false);
                        match self.style {
                            Style::Pill => self.finish_drag(press.origin),
                            Style::Tab => self.finish_tab_drag(press.placement),
                        }
                        Some(PillEvent::Moved)
                    }
                    Some(_) if message == WM_LBUTTONUP => Some(PillEvent::Clicked),
                    _ => None,
                })
            }
            WM_FRAME => {
                frames::received();
                if self.framing {
                    self.present();
                    self.schedule();
                }
                Some(None)
            }
            WM_TIMER if wparam == WAKE_TIMER => {
                unsafe { KillTimer(self.hwnd, WAKE_TIMER) };
                self.present();
                self.schedule();
                Some(None)
            }
            WM_TIMER if wparam == HOVER_TIMER => Some(Some(PillEvent::HoverTick)),
            WM_POINTER_PRESSED => Some(Some(PillEvent::Pressed((
                wparam as isize as i32,
                lparam as i32,
            )))),
            WM_DPICHANGED => {
                self.scale = f64::from((wparam & 0xffff) as u16) / 96.0;
                self.present();
                Some(Some(PillEvent::Moved))
            }
            WM_DISPLAYCHANGE => {
                self.reappear();
                Some(Some(PillEvent::Moved))
            }
            WM_SETTINGCHANGE => {
                self.reduced_motion = reduced_motion();
                if wparam == SPI_SETWORKAREA as WPARAM {
                    self.relocate();
                } else {
                    self.present();
                }
                self.sync();
                Some(Some(PillEvent::Moved))
            }
            WM_FOREGROUND_CHANGED => {
                self.fullscreen = foreground_is_fullscreen(self.hwnd);
                self.sync();
                Some(Some(PillEvent::ForegroundChanged))
            }
            _ => None,
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    let handled = with_pill(|pill| {
        pill.handle(hwnd, message, wparam, lparam)
            .map(|event| (event, pill.on_event.clone()))
    })
    .flatten();
    match handled {
        Some((event, on_event)) => {
            if let Some(event) = event {
                on_event(event);
            }
            0
        }
        None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe extern "system" fn pointer_pressed(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        )
    {
        let point = unsafe { (*(lparam as *const MSLLHOOKSTRUCT)).pt };
        unsafe {
            PostMessageW(
                WINDOW.load(Ordering::Relaxed),
                WM_POINTER_PRESSED,
                point.x as isize as WPARAM,
                point.y as LPARAM,
            )
        };
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn foreground_changed(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    unsafe { PostMessageW(WINDOW.load(Ordering::Relaxed), WM_FOREGROUND_CHANGED, 0, 0) };
}

fn foreground_is_fullscreen(pill: HWND) -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() || IsZoomed(foreground) != 0 {
            return false;
        }
        let mut class = [0u16; 64];
        let length =
            GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        if DESKTOP_CLASSES.contains(&String::from_utf16_lossy(&class[..length]).as_str()) {
            return false;
        }
        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONULL);
        if monitor.is_null() || monitor != MonitorFromWindow(pill, MONITOR_DEFAULTTONEAREST) {
            return false;
        }
        let mut window = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(foreground, &mut window);
        monitor_info(monitor).is_some_and(|info| {
            let screen = info.rcMonitor;
            window.left <= screen.left
                && window.top <= screen.top
                && window.right >= screen.right
                && window.bottom >= screen.bottom
        })
    }
}
