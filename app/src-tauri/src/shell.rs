use crate::displays::target;
use crate::feed::Feed;
use crate::geometry::{
    Corner, Edge, PressTarget, Rect, place_panel, place_panel_on_edge, press_target, primary,
};
use crate::jump;
use crate::model::{PillModel, Unseen, tab_tone};
use crate::panel::{self, PANEL};
use crate::platform::{
    self, pill,
    pill::{PillEvent, TabModel},
};
use crate::positions::{self, Style};
use crate::toasts::Toaster;
use crate::tray::Tray;
use beacon_core::Snapshot;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

const HOVER_CLOSE: Duration = Duration::from_millis(400);
const CLOSE_DELAY: Duration = Duration::from_millis(160);
const TOAST_BATCH: Duration = Duration::from_secs(2);
const INITIAL_PANEL_HEIGHT: f64 = 120.0;

pub enum Action {
    Snapshot(Option<Snapshot>),
    Pill(PillEvent),
    TogglePanel,
    OpenPanel,
    ClosePanel,
    FinishClosing(u64),
    PanelHeight(f64),
    FlushToasts,
    TogglePill,
    SetStyle(Style),
    ShowOn(String),
    ToggleAvoided(String),
    MoveToCorner(Corner),
    MoveToEdge(Edge),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Hover,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Opened {
    mode: Mode,
    focused: bool,
    edge: Option<Edge>,
}

struct Panel {
    window: WebviewWindow,
    mode: Mode,
    focused: bool,
    ready: bool,
    announced: Option<Opened>,
    left_at: Option<Instant>,
    closing: Option<u64>,
}

pub struct Shell {
    app: AppHandle,
    feed: Arc<Feed>,
    tray: Tray,
    snapshot: Option<Snapshot>,
    toaster: Toaster,
    panel: Option<Panel>,
    panel_height: f64,
    unseen: Unseen,
    closings: u64,
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
            unseen: Unseen::default(),
            closings: 0,
        }
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::Snapshot(snapshot) => self.take_snapshot(snapshot),
            Action::Pill(PillEvent::Hovered) if self.mode().is_none() => {
                self.open(Mode::Hover, false)
            }
            Action::Pill(PillEvent::Hovered) => {}
            Action::Pill(PillEvent::Clicked) | Action::TogglePanel => match self.mode() {
                Some(Mode::Sticky) => self.close(),
                _ => self.open(Mode::Sticky, true),
            },
            Action::Pill(PillEvent::Moved) => {
                self.place();
                self.tray.refresh_displays();
            }
            Action::Pill(PillEvent::HoverTick) => self.check_hover(),
            Action::Pill(PillEvent::ForegroundChanged) => {
                jump::acknowledge_focused(&self.feed.home, self.snapshot.as_ref());
                if self.mode() == Some(Mode::Sticky) {
                    self.close();
                }
            }
            Action::Pill(PillEvent::Pressed(point)) => self.pressed(point),
            Action::OpenPanel => self.open(Mode::Sticky, true),
            Action::ClosePanel => self.close(),
            Action::FinishClosing(closing) => {
                if self
                    .panel
                    .as_ref()
                    .is_some_and(|panel| panel.closing == Some(closing))
                {
                    self.destroy();
                }
            }
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
            Action::SetStyle(style) => {
                if let Err(error) = positions::save_style(&self.feed.home, style) {
                    eprintln!("cannot save the style: {error}");
                }
                pill::set_style(style);
                self.tray.show_pill_state(pill::user_visible());
                self.tray.refresh_displays();
                self.place();
            }
            Action::ShowOn(display) => {
                self.change_displays(positions::choose_display(&self.feed.home, &display))
            }
            Action::ToggleAvoided(display) => {
                self.change_displays(positions::toggle_avoided(&self.feed.home, &display))
            }
            Action::MoveToCorner(corner) => {
                pill::move_to_corner(corner);
                self.place();
            }
            Action::MoveToEdge(edge) => {
                pill::move_to_edge(edge);
                self.place();
            }
            Action::Quit => {
                self.destroy();
                self.feed.shutdown();
                self.app.exit(0);
            }
        }
        self.announce();
        let mode = self.mode();
        pill::set_hover_polling(mode == Some(Mode::Hover));
        pill::watch_presses(mode.is_some());
    }

    fn mode(&self) -> Option<Mode> {
        self.panel
            .as_ref()
            .filter(|panel| panel.closing.is_none())
            .map(|panel| panel.mode)
    }

    fn change_displays(&mut self, saved: io::Result<()>) {
        if let Err(error) = saved {
            eprintln!("cannot save the display choice: {error}");
        }
        pill::retarget();
        self.place();
        self.tray.refresh_displays();
    }

    fn placement(&self) -> Option<(Rect, f64)> {
        let monitors = platform::monitors();
        target(&monitors, &positions::display_choice(&self.feed.home))
            .or(primary(&monitors))
            .map(|monitor| match pill::shown_tab() {
                Some((tab, edge)) => place_panel_on_edge(tab, edge, monitor, self.panel_height),
                None => place_panel(pill::shown_rect(), monitor, self.panel_height),
            })
    }

    fn open(&mut self, mode: Mode, focused: bool) {
        match &mut self.panel {
            Some(panel) if panel.closing.is_some() => {
                panel.closing = None;
                panel.mode = mode;
                panel.announced = None;
                panel.left_at = None;
            }
            Some(_) => {}
            None => {
                let Some(placement) = self.placement() else {
                    return;
                };
                match panel::create(&self.app, placement, focused) {
                    Ok(window) => {
                        self.panel = Some(Panel {
                            window,
                            mode,
                            focused: false,
                            ready: false,
                            announced: None,
                            left_at: None,
                            closing: None,
                        })
                    }
                    Err(error) => return eprintln!("cannot open the panel: {error}"),
                }
            }
        }
        self.unseen.clear();
        self.show_tab();
        let Some(panel) = &mut self.panel else {
            return;
        };
        if mode == Mode::Sticky {
            panel.mode = Mode::Sticky;
        }
        if focused {
            panel.focused = true;
            match panel.window.hwnd() {
                Ok(hwnd) => platform::focus_window(hwnd.0),
                Err(error) => eprintln!("cannot focus the panel: {error}"),
            }
        }
        self.place();
    }

    fn close(&mut self) {
        let Some(panel) = self.panel.as_mut().filter(|panel| panel.closing.is_none()) else {
            return;
        };
        self.closings += 1;
        panel.closing = Some(self.closings);
        if let Err(error) = self.app.emit_to(PANEL, "beacon://closing", ()) {
            eprintln!("cannot tell the panel it is closing: {error}");
        }
        let (app, closing) = (self.app.clone(), self.closings);
        thread::spawn(move || {
            thread::sleep(CLOSE_DELAY);
            post(&app, Action::FinishClosing(closing));
        });
    }

    fn destroy(&mut self) {
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
        let Some(panel) = self
            .panel
            .as_mut()
            .filter(|panel| panel.ready && panel.closing.is_none())
        else {
            return;
        };
        let opened = Opened {
            mode: panel.mode,
            focused: panel.focused,
            edge: pill::anchor_edge(),
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

    fn pressed(&mut self, point: (i32, i32)) {
        if self.mode().is_none() {
            return;
        }
        match press_target(
            point,
            self.panel_rect(),
            &[pill::shown_rect(), self.tray.rect()],
        ) {
            PressTarget::Panel => self.open(Mode::Sticky, false),
            PressTarget::Anchor => {}
            PressTarget::Outside => self.close(),
        }
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

    fn show_tab(&self) {
        pill::show_tab(TabModel {
            tone: tab_tone(self.snapshot.as_ref()),
            unseen_since: self.unseen.since(),
        });
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
        self.snapshot = snapshot;
        self.unseen.observe(self.snapshot.as_ref(), Instant::now());
        self.show_tab();
        if self.panel.is_some()
            && let Err(error) = self.app.emit_to(PANEL, "beacon://snapshot", &self.snapshot)
        {
            eprintln!("cannot send the snapshot to the panel: {error}");
        }
    }

    fn flush_toasts(&mut self) {
        let preferences = positions::ui_settings(&self.feed.home);
        if !preferences.toast_on_finished {
            self.toaster.drop_finished();
        }
        let Some(toast) = self
            .toaster
            .flush_unless_focused(jump::focused_session(self.snapshot.as_ref()).as_deref())
            .filter(|_| preferences.toasts_enabled)
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
