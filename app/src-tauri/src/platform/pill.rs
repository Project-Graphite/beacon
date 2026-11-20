use super::{GdiText, bitmap, cursor_position, monitor_info, monitors, reduced_motion};
use crate::geometry::{
    Rect, clamp_into, default_pill_rect, layout_signature, monitor_for, primary, resize_anchored,
    snap,
};
use crate::model::PillModel;
use crate::positions;
use crate::raster::render_pill;
use std::cell::RefCell;
use std::ffi::c_void;
use std::io;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MonitorFromWindow, ReleaseDC, SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows_sys::Win32::UI::Controls::{WM_MOUSEHOVER, WM_MOUSELEAVE};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_HOVER, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, EVENT_SYSTEM_FOREGROUND, GetClassNameW, GetForegroundWindow,
    GetWindowRect, IDC_ARROW, IsZoomed, KillTimer, LoadCursorW, MA_NOACTIVATE, PostMessageW,
    RegisterClassExW, SPI_SETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, SetTimer, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP, WM_CAPTURECHANGED, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_SETTINGCHANGE,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

const CLASS: &str = "BeaconPill";
const ANIMATION_TIMER: usize = 1;
const HOVER_TIMER: usize = 2;
const TICK_MS: u32 = 100;
const HOVER_MS: u32 = 150;
const CLICK_SLOP: f32 = 4.0;
const WM_FOREGROUND_CHANGED: u32 = WM_APP + 1;
const DESKTOP_CLASSES: [&str; 2] = ["Progman", "WorkerW"];

static WINDOW: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillEvent {
    Hovered,
    Clicked,
    Moved,
    HoverTick,
}

struct Press {
    cursor: (i32, i32),
    origin: Rect,
    dragging: bool,
}

struct Pill {
    hwnd: HWND,
    home: PathBuf,
    model: PillModel,
    scale: f32,
    rect: Rect,
    user_visible: bool,
    fullscreen: bool,
    shown: bool,
    reduced_motion: bool,
    animating: bool,
    hover_polling: bool,
    tracking: bool,
    pressed_since_entry: bool,
    press: Option<Press>,
    started: Instant,
    text: GdiText,
    on_event: Rc<dyn Fn(PillEvent)>,
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
    let hwnd = unsafe {
        let instance = GetModuleHandleW(null());
        let window_class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassExW(&window_class) == 0 {
            return Err(io::Error::last_os_error());
        }
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        return Err(io::Error::last_os_error());
    }
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
    let mut pill = Pill {
        hwnd,
        home,
        model: PillModel::default(),
        scale: 1.0,
        rect: Rect::default(),
        user_visible: visible,
        fullscreen: false,
        shown: false,
        reduced_motion: reduced_motion(),
        animating: false,
        hover_polling: false,
        tracking: false,
        pressed_since_entry: false,
        press: None,
        started: Instant::now(),
        text: GdiText::default(),
        on_event: Rc::new(on_event),
    };
    pill.relocate(None);
    pill.fullscreen = foreground_is_fullscreen(hwnd);
    pill.sync();
    PILL.with(|cell| cell.replace(Some(pill)));
    Ok(())
}

pub fn show_model(model: PillModel) {
    with_pill(|pill| {
        if pill.model != model {
            pill.model = model;
            pill.present();
            pill.sync();
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

pub fn shown_rect() -> Option<Rect> {
    with_pill(|pill| pill.shown.then_some(pill.rect)).flatten()
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

impl Pill {
    fn relocate(&mut self, fallback: Option<Rect>) {
        let monitors = monitors();
        let rect = positions::load(&self.home, &layout_signature(&monitors))
            .or(fallback)
            .unwrap_or_else(|| {
                let scale = primary(&monitors).map_or(1.0, |monitor| monitor.scale);
                let canvas = render_pill(&self.model, scale as f32, 0.0, true, &self.text);
                default_pill_rect(canvas.width as i32, canvas.height as i32, &monitors)
            });
        self.rect = clamp_into(rect, &monitors);
        self.scale = monitor_for(&self.rect, &monitors).map_or(1.0, |monitor| monitor.scale) as f32;
        self.present();
    }

    fn present(&mut self) {
        let canvas = render_pill(
            &self.model,
            self.scale,
            self.started.elapsed().as_secs_f32(),
            self.reduced_motion,
            &self.text,
        );
        let (width, height) = (canvas.width as i32, canvas.height as i32);
        if (width, height) != (self.rect.width(), self.rect.height()) {
            let monitors = monitors();
            let work = monitor_for(&self.rect, &monitors).map_or(self.rect, |monitor| monitor.work);
            self.rect = resize_anchored(self.rect, width, height, &work);
        }
        let pixels = canvas.premultiplied_bgra();
        unsafe {
            let screen = GetDC(null_mut());
            let memory = CreateCompatibleDC(screen);
            let (bitmap, bits) = bitmap(memory, width, height);
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits, pixels.len());
            let previous = SelectObject(memory, bitmap);
            UpdateLayeredWindow(
                self.hwnd,
                screen,
                &POINT {
                    x: self.rect.left,
                    y: self.rect.top,
                },
                &SIZE {
                    cx: width,
                    cy: height,
                },
                memory,
                &POINT { x: 0, y: 0 },
                0,
                &BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                },
                ULW_ALPHA,
            );
            SelectObject(memory, previous);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(null_mut(), screen);
        }
    }

    fn sync(&mut self) {
        let visible = self.user_visible && !self.fullscreen;
        if visible != self.shown {
            self.shown = visible;
            unsafe { ShowWindow(self.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
        }
        let animate = visible && self.model.animates(self.reduced_motion);
        if animate != self.animating {
            self.animating = animate;
            unsafe {
                if animate {
                    SetTimer(self.hwnd, ANIMATION_TIMER, TICK_MS, None);
                } else {
                    KillTimer(self.hwnd, ANIMATION_TIMER);
                }
            }
        }
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
        let scale =
            monitor_for(&self.rect, &monitors).map_or(self.scale, |monitor| monitor.scale as f32);
        if scale != self.scale {
            self.scale = scale;
            self.present();
        }
    }

    fn finish_drag(&mut self) {
        let monitors = monitors();
        if let Some(monitor) = monitor_for(&self.rect, &monitors) {
            let snapped = snap(self.rect, &monitor.work, monitor.scale);
            self.move_to(snapped.left, snapped.top);
        }
        if let Err(error) = positions::save(&self.home, &layout_signature(&monitors), self.rect) {
            eprintln!("cannot save the pill position: {error}");
        }
    }

    fn handle(&mut self, message: u32, wparam: WPARAM) -> Option<Option<PillEvent>> {
        match message {
            WM_MOUSEMOVE => {
                if !self.tracking {
                    let mut tracking = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_HOVER | TME_LEAVE,
                        hwndTrack: self.hwnd,
                        dwHoverTime: HOVER_MS,
                    };
                    self.tracking = unsafe { TrackMouseEvent(&mut tracking) } != 0;
                }
                let slop = (CLICK_SLOP * self.scale) as i32;
                let target = self.press.as_mut().and_then(|press| {
                    let (x, y) = cursor_position();
                    let (dx, dy) = (x - press.cursor.0, y - press.cursor.1);
                    press.dragging |= dx.abs() > slop || dy.abs() > slop;
                    press
                        .dragging
                        .then_some((press.origin.left + dx, press.origin.top + dy))
                });
                if let Some((left, top)) = target {
                    self.move_to(left, top);
                }
                Some(None)
            }
            WM_MOUSEHOVER => Some(
                (self.press.is_none() && !self.pressed_since_entry).then_some(PillEvent::Hovered),
            ),
            WM_MOUSELEAVE => {
                self.tracking = false;
                self.pressed_since_entry = false;
                Some(None)
            }
            WM_LBUTTONDOWN => {
                unsafe { SetCapture(self.hwnd) };
                self.pressed_since_entry = true;
                self.press = Some(Press {
                    cursor: cursor_position(),
                    origin: self.rect,
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
                        self.finish_drag();
                        Some(PillEvent::Moved)
                    }
                    Some(_) if message == WM_LBUTTONUP => Some(PillEvent::Clicked),
                    _ => None,
                })
            }
            WM_TIMER if wparam == ANIMATION_TIMER => {
                self.present();
                Some(None)
            }
            WM_TIMER if wparam == HOVER_TIMER => Some(Some(PillEvent::HoverTick)),
            WM_DPICHANGED => {
                self.scale = (wparam & 0xffff) as f32 / 96.0;
                self.present();
                Some(Some(PillEvent::Moved))
            }
            WM_DISPLAYCHANGE => {
                self.relocate(Some(self.rect));
                Some(Some(PillEvent::Moved))
            }
            WM_SETTINGCHANGE => {
                self.reduced_motion = reduced_motion();
                if wparam == SPI_SETWORKAREA as WPARAM {
                    self.relocate(Some(self.rect));
                } else {
                    self.present();
                }
                self.sync();
                Some(Some(PillEvent::Moved))
            }
            WM_FOREGROUND_CHANGED => {
                self.fullscreen = foreground_is_fullscreen(self.hwnd);
                self.sync();
                Some(None)
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
        pill.handle(message, wparam)
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
