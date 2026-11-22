use crate::displays::target;
use crate::feed::{self, Feed};
use crate::geometry::{
    Corner, Edge, PressTarget, Rect, place_panel, place_panel_on_edge, press_target, primary,
};
use crate::jump;
use crate::model::{
    LINGER, Peek, PeekBatch, PillModel, Pin, Presence, TabTone, Unseen, Unused, alerting_count,
    claude_active, in_use, overlay_wanted, peek_for, pin_decision, tab_tone,
};
use crate::panel::{self, PANEL};
use crate::platform::{
    self, pill,
    pill::{PillEvent, TabModel},
};
use crate::positions::{self, AttentionStyle, StartMode, Style, UiSettings, Visibility};
use crate::sounds::sound_for;
use crate::toasts::Toaster;
use crate::tray::Tray;
use beacon_core::{Request, Snapshot};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

const HOVER_CLOSE: Duration = Duration::from_millis(400);
const CLOSE_DELAY: Duration = Duration::from_millis(160);
const TOAST_BATCH: Duration = Duration::from_secs(2);
const PREVIEW_TAIL: Duration = Duration::from_secs(1);
const INITIAL_PANEL_HEIGHT: f64 = 120.0;
const KEPT_WARM: Duration = Duration::from_secs(45);
const KEPT_WARM_WHILE_ACTIVE: Duration = Duration::from_secs(60);
const PREVIEW_TEXT: &str = "A session needs you (preview)";

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
    SettingsChanged,
    PreviewAttention,
    Recheck,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    edge: Option<Edge>,
}

#[derive(Clone, Serialize)]
struct OpenedPayload<'a> {
    #[serde(flatten)]
    opened: Opened,
    snapshot: Option<&'a Snapshot>,
    ui: &'a UiSettings,
}

#[derive(Serialize)]
struct InitialData<'a> {
    snapshot: Option<&'a Snapshot>,
    ui: &'a UiSettings,
}

struct Panel {
    window: WebviewWindow,
    mode: Mode,
    focused: bool,
    ready: bool,
    announced: Option<Opened>,
    left_at: Option<Instant>,
    closing: Option<u64>,
    hidden_since: Option<Instant>,
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
    settings: UiSettings,
    presence: Presence,
    unused: Unused,
    quit_armed: Option<Instant>,
    peek_batch: PeekBatch,
    preview: Option<Instant>,
    was_active: bool,
    pin_dismissed: bool,
    pin_released: Option<Instant>,
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
            tray,
            snapshot: None,
            toaster: Toaster::default(),
            panel: None,
            panel_height: INITIAL_PANEL_HEIGHT,
            unseen: Unseen::default(),
            closings: 0,
            settings: positions::ui_settings(&feed.home),
            presence: Presence::default(),
            unused: Unused::default(),
            quit_armed: None,
            peek_batch: PeekBatch::default(),
            preview: None,
            was_active: false,
            pin_dismissed: false,
            pin_released: None,
            feed,
        }
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::Snapshot(snapshot) => self.take_snapshot(snapshot),
            Action::Pill(PillEvent::Entered) => self.prepare(),
            Action::Pill(PillEvent::Hovered | PillEvent::RevealHovered)
                if self.mode().is_none() =>
            {
                self.open(Mode::Hover, false)
            }
            Action::Pill(PillEvent::Hovered | PillEvent::RevealHovered) => {}
            Action::Pill(PillEvent::PeekPressed) => self.open(Mode::Sticky, true),
            Action::Pill(PillEvent::Clicked) | Action::TogglePanel => match self.mode() {
                Some(Mode::Sticky | Mode::Pinned) => self.put_away(),
                _ => self.open(Mode::Sticky, true),
            },
            Action::Pill(PillEvent::Moved) => self.overlay_changed(),
            Action::Pill(PillEvent::HoverTick) => self.check_hover(),
            Action::Pill(PillEvent::ForegroundChanged) => {
                jump::acknowledge_focused(&self.feed.home, self.snapshot.as_ref());
                if self.mode() == Some(Mode::Sticky) {
                    self.close();
                }
            }
            Action::Pill(PillEvent::Pressed(point)) => self.pressed(point),
            Action::OpenPanel => self.open(Mode::Sticky, true),
            Action::ClosePanel => self.put_away(),
            Action::FinishClosing(closing) => {
                if self
                    .panel
                    .as_ref()
                    .is_some_and(|panel| panel.closing == Some(closing))
                {
                    self.hide();
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
                self.overlay_changed();
            }
            Action::ShowOn(display) => {
                self.change_displays(positions::choose_display(&self.feed.home, &display))
            }
            Action::ToggleAvoided(display) => {
                self.change_displays(positions::toggle_avoided(&self.feed.home, &display))
            }
            Action::MoveToCorner(corner) => {
                pill::move_to_corner(corner);
                self.overlay_changed();
            }
            Action::MoveToEdge(edge) => {
                pill::move_to_edge(edge);
                self.overlay_changed();
            }
            Action::SettingsChanged => {
                self.settings = positions::ui_settings(&self.feed.home);
                pill::configure(self.settings.clone());
                self.show_tab();
            }
            Action::PreviewAttention => self.preview(),
            Action::Recheck => {}
            Action::Quit => return self.quit(),
        }
        self.follow_activity();
        self.announce();
        let mode = self.mode();
        pill::set_hover_polling(mode == Some(Mode::Hover));
        pill::watch_presses(mode.is_some_and(|mode| mode != Mode::Pinned));
        self.settle();
    }

    fn put_away(&mut self) {
        self.pin_dismissed |= self.mode() == Some(Mode::Pinned);
        self.close();
    }

    fn follow_activity(&mut self) {
        let active = claude_active(self.snapshot.as_ref());
        let (pin, dismissed) = pin_decision(
            self.was_active,
            active,
            self.settings.pin_while_active,
            self.pin_dismissed,
        );
        (self.was_active, self.pin_dismissed) = (active, dismissed);
        let pinned = self.mode() == Some(Mode::Pinned);
        match pin {
            Pin::Open => {
                self.pin_released = None;
                if !pinned && !pill::fullscreen_in_front() {
                    self.open(Mode::Pinned, false);
                }
            }
            Pin::Close if pinned && self.pin_released.is_none() => {
                self.pin_released = Some(Instant::now());
                self.recheck_after(LINGER);
            }
            Pin::Close | Pin::Keep | Pin::StayHidden => {}
        }
        if pinned
            && self
                .pin_released
                .is_some_and(|released| released.elapsed() >= LINGER)
        {
            self.pin_released = None;
            self.close();
        }
    }

    fn overlay_changed(&self) {
        self.place();
        self.tray.refresh_displays();
        if self.panel.is_some()
            && let Err(error) = self.app.emit_to(PANEL, "beacon://settings-changed", ())
        {
            eprintln!("cannot tell the panel the settings changed: {error}");
        }
    }

    fn quit(&mut self) {
        self.destroy();
        self.feed.shutdown();
        self.app.exit(0);
    }

    fn recheck_after(&self, delay: Duration) {
        let app = self.app.clone();
        thread::spawn(move || {
            thread::sleep(delay);
            post(&app, Action::Recheck);
        });
    }

    fn preview_length(&self) -> Duration {
        Duration::from_secs_f64(self.settings.peek_seconds) + PREVIEW_TAIL
    }

    fn preview(&mut self) {
        if self.settings.visibility == Visibility::Never {
            return;
        }
        self.preview = Some(Instant::now());
        self.show_tab();
        if self.settings.attention_style == AttentionStyle::PulsePeek {
            pill::peek(Peek {
                text: PREVIEW_TEXT.to_string(),
                tone: TabTone::NeedsYou,
            });
        }
        self.recheck_after(self.preview_length());
    }

    fn settle(&mut self) {
        let now = Instant::now();
        if self
            .preview
            .is_some_and(|started| now.duration_since(started) >= self.preview_length())
        {
            self.preview = None;
            self.show_tab();
        }
        let never = self.settings.visibility == Visibility::Never;
        let needed = !never
            && (self.mode().is_some()
                || self.preview.is_some()
                || overlay_wanted(self.snapshot.as_ref(), self.settings.visibility));
        let was_shown = self.presence == Presence::Shown;
        if let Some(linger) = self.presence.update(needed, now)
            && was_shown
        {
            self.recheck_after(linger);
        }
        let shown = self.presence.shown();
        pill::reveal(shown, !never && !shown && self.settings.reveal_on_hover);
        self.unused
            .observe(in_use(self.snapshot.as_ref(), self.mode().is_some()), now);
        let kept_warm = if claude_active(self.snapshot.as_ref()) {
            KEPT_WARM_WHILE_ACTIVE
        } else {
            KEPT_WARM
        };
        if self
            .panel
            .as_ref()
            .and_then(|panel| panel.hidden_since)
            .is_some_and(|since| since.elapsed() >= kept_warm)
        {
            self.destroy();
        }
        let minutes = if self.settings.start_mode == StartMode::Claude {
            self.settings.quit_when_idle_minutes
        } else {
            0
        };
        match self.unused.quit_in(minutes, now) {
            Some(wait) if wait.is_zero() => self.quit(),
            Some(wait) if self.quit_armed != Some(now + wait) => {
                self.quit_armed = Some(now + wait);
                self.recheck_after(wait);
            }
            Some(_) => {}
            None => self.quit_armed = None,
        }
    }

    fn peek_gains(&mut self, gained: &[String]) {
        if gained.is_empty()
            || self.mode().is_some()
            || self.settings.visibility == Visibility::Never
            || self.settings.attention_style != AttentionStyle::PulsePeek
        {
            return;
        }
        let focused = jump::focused_session(self.snapshot.as_ref());
        let unfocused: Vec<String> = gained
            .iter()
            .filter(|id| focused.as_ref() != Some(*id))
            .cloned()
            .collect();
        let Some(snapshot) = self.snapshot.as_ref().filter(|_| !unfocused.is_empty()) else {
            return;
        };
        let batch = self.peek_batch.add(&unfocused, Instant::now());
        let sessions: Vec<_> = snapshot
            .sessions
            .iter()
            .filter(|session| session.alerting && batch.contains(&session.session_id))
            .collect();
        if let Some(peek) = peek_for(&sessions) {
            pill::peek(peek);
        }
    }

    fn mode(&self) -> Option<Mode> {
        self.panel
            .as_ref()
            .filter(|panel| panel.closing.is_none() && panel.hidden_since.is_none())
            .map(|panel| panel.mode)
    }

    fn change_displays(&mut self, saved: io::Result<()>) {
        if let Err(error) = saved {
            eprintln!("cannot save the display choice: {error}");
        }
        pill::retarget();
        self.overlay_changed();
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

    fn prepare(&mut self) {
        if self.panel.is_some() {
            return;
        }
        let Some(placement) = self.placement() else {
            return;
        };
        let initial = serde_json::to_string(&InitialData {
            snapshot: self.snapshot.as_ref(),
            ui: &self.settings,
        });
        match initial
            .map_err(tauri::Error::from)
            .and_then(|initial| panel::create(&self.app, placement, &initial))
        {
            Ok(window) => {
                self.panel = Some(Panel {
                    window,
                    mode: Mode::Hover,
                    focused: false,
                    ready: false,
                    announced: None,
                    left_at: None,
                    closing: None,
                    hidden_since: Some(Instant::now()),
                });
                self.recheck_after(KEPT_WARM);
                self.recheck_after(KEPT_WARM_WHILE_ACTIVE);
            }
            Err(error) => eprintln!("cannot create the panel: {error}"),
        }
    }

    fn open(&mut self, mode: Mode, focused: bool) {
        self.pin_dismissed &= mode == Mode::Pinned;
        pill::dismiss_peek();
        pill::reveal(self.settings.visibility != Visibility::Never, false);
        self.prepare();
        let Some(panel) = &mut self.panel else {
            return;
        };
        if panel.closing.is_some() || panel.hidden_since.is_some() {
            panel.closing = None;
            panel.mode = mode;
            panel.focused = false;
            panel.announced = None;
            panel.left_at = None;
        }
        if panel.hidden_since.take().is_some() {
            self.place();
            self.show();
            feed::send(self.feed.home.clone(), Request::Reconcile);
        }
        self.unseen.clear();
        self.show_tab();
        let Some(panel) = &mut self.panel else {
            return;
        };
        panel.mode = panel.mode.max(mode);
        if focused {
            panel.focused = true;
            match panel.window.hwnd() {
                Ok(hwnd) => platform::focus_window(hwnd.0),
                Err(error) => eprintln!("cannot focus the panel: {error}"),
            }
        }
        self.place();
    }

    fn show(&self) {
        let Some(panel) = &self.panel else {
            return;
        };
        match panel.window.hwnd() {
            Ok(hwnd) => platform::show_without_activation(hwnd.0),
            Err(error) => eprintln!("cannot show the panel: {error}"),
        }
        if let Err(error) = panel.window.as_ref().show() {
            eprintln!("cannot show the panel page: {error}");
        }
    }

    fn hide(&mut self) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        panel.closing = None;
        panel.hidden_since = Some(Instant::now());
        if let Err(error) = panel.window.as_ref().hide() {
            eprintln!("cannot hide the panel page: {error}");
        }
        match panel.window.hwnd() {
            Ok(hwnd) => platform::hide_window(hwnd.0),
            Err(error) => eprintln!("cannot hide the panel: {error}"),
        }
        self.recheck_after(KEPT_WARM);
        self.recheck_after(KEPT_WARM_WHILE_ACTIVE);
    }

    fn close(&mut self) {
        let Some(panel) = self
            .panel
            .as_mut()
            .filter(|panel| panel.closing.is_none() && panel.hidden_since.is_none())
        else {
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
            .hwnd()
            .map_err(|error| error.to_string())
            .and_then(|hwnd| {
                platform::place_window(hwnd.0, rect).map_err(|error| error.to_string())
            });
        if let Err(error) = placed {
            eprintln!("cannot place the panel: {error}");
        }
    }

    fn announce(&mut self) {
        let Some(panel) = self
            .panel
            .as_mut()
            .filter(|panel| panel.ready && panel.closing.is_none() && panel.hidden_since.is_none())
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
            let payload = OpenedPayload {
                opened,
                snapshot: self.snapshot.as_ref(),
                ui: &self.settings,
            };
            if let Err(error) = self.app.emit_to(PANEL, "beacon://opened", payload) {
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
            PressTarget::Outside if self.mode() != Some(Mode::Pinned) => self.close(),
            PressTarget::Anchor | PressTarget::Outside => {}
        }
    }

    fn check_hover(&mut self) {
        if self.mode() != Some(Mode::Hover) {
            return;
        }
        let cursor = platform::cursor_position();
        let inside = [pill::shown_rect(), self.panel_rect(), pill::reveal_rect()]
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
        pill::show_tab(match self.preview {
            Some(started) => TabModel {
                tone: TabTone::NeedsYou,
                attention_since: Some(started),
                alerting: true,
            },
            None => TabModel {
                tone: tab_tone(self.snapshot.as_ref()),
                attention_since: self.unseen.since().map(|(_, newest)| newest),
                alerting: alerting_count(self.snapshot.as_ref()) > 0,
            },
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
        if let Some(current) = &snapshot
            && let Some(sound) = sound_for(
                self.snapshot.as_ref(),
                current,
                self.settings.sounds_enabled,
            )
        {
            platform::play_system_sound(sound.system_alias());
        }
        self.snapshot = snapshot;
        let gained = self.unseen.observe(self.snapshot.as_ref(), Instant::now());
        if self.mode().is_some() {
            self.unseen.clear();
        }
        self.peek_gains(&gained);
        self.show_tab();
        if self.mode().is_some()
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
