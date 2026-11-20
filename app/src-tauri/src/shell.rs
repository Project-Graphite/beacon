use crate::feed::Feed;
use crate::geometry::{Rect, place_panel};
use crate::jump;
use crate::model::{PillModel, alerting_count};
use crate::panel::{self, PANEL};
use crate::platform::{self, pill, pill::PillEvent};
use crate::toasts::Toaster;
use crate::tray::Tray;
use beacon_core::Snapshot;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

const HOVER_CLOSE: Duration = Duration::from_millis(400);
const TOAST_BATCH: Duration = Duration::from_secs(2);
const INITIAL_PANEL_HEIGHT: f64 = 120.0;

pub enum Action {
    Snapshot(Option<Snapshot>),
    Pill(PillEvent),
    TogglePanel,
    OpenPanel,
    ClosePanel,
    PanelHeight(f64),
    FlushToasts,
    TogglePill,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    Hover,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Hover,
    Sticky,
    Pinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Opened {
    mode: Mode,
    focused: bool,
}

struct Panel {
    window: WebviewWindow,
    base: Base,
    focused: bool,
    ready: bool,
    announced: Option<Opened>,
    left_at: Option<Instant>,
}

pub struct Shell {
    app: AppHandle,
    feed: Arc<Feed>,
    tray: Tray,
    snapshot: Option<Snapshot>,
    toaster: Toaster,
    panel: Option<Panel>,
    panel_height: f64,
}

thread_local! {
    static SHELL: RefCell<Option<Shell>> = const { RefCell::new(None) };
    static QUEUE: RefCell<VecDeque<Action>> = const { RefCell::new(VecDeque::new()) };
}

pub fn install(shell: Shell) {
    SHELL.with(|cell| cell.replace(Some(shell)));
}

pub fn dispatch(action: Action) {
    QUEUE.with(|queue| queue.borrow_mut().push_back(action));
    SHELL.with(|cell| {
        let Ok(mut shell) = cell.try_borrow_mut() else {
            return;
        };
        let Some(shell) = shell.as_mut() else {
            return;
        };
        while let Some(action) = QUEUE.with(|queue| queue.borrow_mut().pop_front()) {
            shell.handle(action);
        }
    });
}

pub fn post(app: &AppHandle, action: Action) {
    if let Err(error) = app.run_on_main_thread(move || dispatch(action)) {
        eprintln!("cannot reach the main thread: {error}");
    }
}

impl Shell {
    pub fn new(app: AppHandle, feed: Arc<Feed>, tray: Tray) -> Shell {
        Shell {
            app,
            feed,
            tray,
            snapshot: None,
            toaster: Toaster::default(),
            panel: None,
            panel_height: INITIAL_PANEL_HEIGHT,
        }
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::Snapshot(snapshot) => self.take_snapshot(snapshot),
            Action::Pill(PillEvent::Hovered) if self.panel.is_none() => {
                self.open(Base::Hover, false)
            }
            Action::Pill(PillEvent::Hovered) => {}
            Action::Pill(PillEvent::Clicked) | Action::TogglePanel => match self.mode() {
                Some(Mode::Sticky) => self.close(),
                _ => self.open(Base::Sticky, true),
            },
            Action::Pill(PillEvent::Moved) => self.place(),
            Action::Pill(PillEvent::HoverTick) => self.check_hover(),
            Action::Pill(PillEvent::ForegroundChanged) => {
                jump::acknowledge_focused(&self.feed.home, self.snapshot.as_ref())
            }
            Action::OpenPanel => self.open(Base::Sticky, true),
            Action::ClosePanel if self.mode() != Some(Mode::Pinned) => self.close(),
            Action::ClosePanel => {}
            Action::PanelHeight(height) => {
                self.panel_height = height;
                if let Some(panel) = &mut self.panel {
                    panel.ready = true;
                }
                self.place();
            }
            Action::FlushToasts => self.flush_toasts(),
            Action::TogglePill => {
                let shown = !pill::user_visible();
                pill::set_user_visible(shown);
                self.tray.show_pill_state(shown);
                self.place();
            }
            Action::Quit => {
                self.close();
                self.feed.shutdown();
                self.app.exit(0);
            }
        }
        self.announce();
        pill::set_hover_polling(self.mode() == Some(Mode::Hover));
    }

    fn mode(&self) -> Option<Mode> {
        let pinned = alerting_count(self.snapshot.as_ref()) > 0;
        self.panel.as_ref().map(|panel| match (pinned, panel.base) {
            (true, _) => Mode::Pinned,
            (false, Base::Hover) => Mode::Hover,
            (false, Base::Sticky) => Mode::Sticky,
        })
    }

    fn placement(&self) -> Option<(Rect, f64)> {
        place_panel(pill::shown_rect(), &platform::monitors(), self.panel_height)
    }

    fn open(&mut self, base: Base, focused: bool) {
        if self.panel.is_none() {
            let Some(placement) = self.placement() else {
                return;
            };
            match panel::create(&self.app, placement, focused) {
                Ok(window) => {
                    self.panel = Some(Panel {
                        window,
                        base,
                        focused: false,
                        ready: false,
                        announced: None,
                        left_at: None,
                    })
                }
                Err(error) => return eprintln!("cannot open the panel: {error}"),
            }
        }
        let Some(panel) = &mut self.panel else {
            return;
        };
        if base == Base::Sticky {
            panel.base = Base::Sticky;
        }
        if focused {
            panel.focused = true;
            match panel.window.hwnd() {
                Ok(hwnd) => platform::focus_window(hwnd.0),
                Err(error) => eprintln!("cannot focus the panel: {error}"),
            }
        }
    }

    fn close(&mut self) {
        if let Some(panel) = self.panel.take()
            && let Err(error) = panel.window.destroy()
        {
            eprintln!("cannot close the panel: {error}");
        }
    }

    fn place(&self) {
        let (Some(panel), Some((rect, _))) = (&self.panel, self.placement()) else {
            return;
        };
        let placed = panel
            .window
            .set_size(PhysicalSize::new(rect.width() as u32, rect.height() as u32))
            .and_then(|()| {
                panel
                    .window
                    .set_position(PhysicalPosition::new(rect.left, rect.top))
            });
        if let Err(error) = placed {
            eprintln!("cannot place the panel: {error}");
        }
    }

    fn announce(&mut self) {
        let mode = self.mode();
        let Some((panel, mode)) = self
            .panel
            .as_mut()
            .zip(mode)
            .filter(|(panel, _)| panel.ready)
        else {
            return;
        };
        let opened = Opened {
            mode,
            focused: panel.focused,
        };
        if panel.announced != Some(opened) {
            panel.announced = Some(opened);
            if let Err(error) = self.app.emit_to(PANEL, "beacon://opened", opened) {
                eprintln!("cannot tell the panel it opened: {error}");
            }
        }
    }

    fn panel_rect(&self) -> Option<Rect> {
        let window = &self.panel.as_ref()?.window;
        let (position, size) = (window.outer_position().ok()?, window.outer_size().ok()?);
        Some(Rect::sized(
            position.x,
            position.y,
            size.width as i32,
            size.height as i32,
        ))
    }

    fn check_hover(&mut self) {
        if self.mode() != Some(Mode::Hover) {
            return;
        }
        let cursor = platform::cursor_position();
        let inside = [pill::shown_rect(), self.panel_rect()]
            .into_iter()
            .flatten()
            .any(|rect| rect.contains(cursor));
        let Some(panel) = &mut self.panel else {
            return;
        };
        match (inside, panel.left_at) {
            (true, _) => panel.left_at = None,
            (false, None) => panel.left_at = Some(Instant::now()),
            (false, Some(left_at)) if left_at.elapsed() >= HOVER_CLOSE => self.close(),
            (false, Some(_)) => {}
        }
    }

    fn take_snapshot(&mut self, snapshot: Option<Snapshot>) {
        pill::show_model(PillModel::from_snapshot(snapshot.as_ref()));
        self.tray.show(snapshot.as_ref());
        if let Some(current) = &snapshot
            && self.toaster.observe(current)
        {
            let app = self.app.clone();
            thread::spawn(move || {
                thread::sleep(TOAST_BATCH);
                post(&app, Action::FlushToasts);
            });
        }
        let was_alerting = alerting_count(self.snapshot.as_ref()) > 0;
        self.snapshot = snapshot;
        if !was_alerting && alerting_count(self.snapshot.as_ref()) > 0 && self.panel.is_none() {
            self.open(Base::Hover, false);
        }
        if self.panel.is_some()
            && let Err(error) = self.app.emit_to(PANEL, "beacon://snapshot", &self.snapshot)
        {
            eprintln!("cannot send the snapshot to the panel: {error}");
        }
    }

    fn flush_toasts(&mut self) {
        let Some(toast) = self
            .toaster
            .flush_unless_focused(jump::focused_session(self.snapshot.as_ref()).as_deref())
        else {
            return;
        };
        if let Err(error) = self
            .app
            .notification()
            .builder()
            .title(toast.title)
            .body(toast.body)
            .show()
        {
            eprintln!("cannot show a toast: {error}");
        }
    }
}
