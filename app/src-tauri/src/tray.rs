use crate::displays::{
    self, AVOID, CORNER, DisplayMenu, EDGE, MenuEntry, SHOW_ON, STYLE, corner, edge, style,
};
use crate::feed;
use crate::geometry::Rect;
use crate::model::{TrayState, tooltip, tray_state};
use crate::platform::{self, GdiText, tray_icon_size};
use crate::positions::{self, Style};
use crate::raster::render_tray;
use crate::shell::{self, Action};
use beacon_core::{Request, Snapshot};
use std::path::{Path, PathBuf};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

pub struct Tray {
    icon: TrayIcon,
    home: PathBuf,
    pill_item: MenuItem<Wry>,
    state: TrayState,
    tooltip: String,
    text: GdiText,
}

fn pill_label(shown: bool, style: Style) -> &'static str {
    match (shown, style) {
        (true, Style::Tab) => "Hide edge tab",
        (false, Style::Tab) => "Show edge tab",
        (true, Style::Pill) => "Hide pill",
        (false, Style::Pill) => "Show pill",
    }
}

fn image(state: TrayState, text: &GdiText) -> Image<'static> {
    let size = tray_icon_size();
    Image::new_owned(
        render_tray(state, size, text).straight_rgba(),
        size as u32,
        size as u32,
    )
}

fn display_menu(home: &Path) -> DisplayMenu {
    displays::menu(
        &platform::monitors(),
        &positions::display_choice(home),
        positions::style(home),
    )
}

fn submenu(app: &AppHandle, title: &str, entries: &[MenuEntry]) -> tauri::Result<Submenu<Wry>> {
    let submenu = Submenu::new(app, title, true)?;
    for entry in entries {
        match entry.checked {
            Some(checked) => submenu.append(&CheckMenuItem::with_id(
                app,
                &entry.id,
                &entry.label,
                entry.enabled,
                checked,
                None::<&str>,
            )?)?,
            None => submenu.append(&MenuItem::with_id(
                app,
                &entry.id,
                &entry.label,
                entry.enabled,
                None::<&str>,
            )?)?,
        }
    }
    Ok(submenu)
}

fn menu(
    app: &AppHandle,
    pill_item: &MenuItem<Wry>,
    displays: &DisplayMenu,
) -> tauri::Result<Menu<Wry>> {
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open panel", true, None::<&str>)?,
            &MenuItem::with_id(
                app,
                "snooze-5",
                "Snooze all for 5 minutes",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                "snooze-15",
                "Snooze all for 15 minutes",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "clear-snooze", "Clear snooze", true, None::<&str>)?,
            pill_item,
            &submenu(app, "Style", &displays.style)?,
            &submenu(app, "Show on", &displays.show_on)?,
            &submenu(app, "Never show on", &displays.never_show_on)?,
            &submenu(app, "Position", &displays.position)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )
}

pub fn build(app: &AppHandle, home: PathBuf, pill_shown: bool) -> tauri::Result<Tray> {
    let pill_item = MenuItem::with_id(
        app,
        "toggle-pill",
        pill_label(pill_shown, positions::style(&home)),
        true,
        None::<&str>,
    )?;
    let menu = menu(app, &pill_item, &display_menu(&home))?;
    let text = GdiText::default();
    let menu_home = home.clone();
    let icon = TrayIconBuilder::with_id("beacon")
        .icon(image(TrayState::Quiet, &text))
        .tooltip(tooltip(None))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| handle_menu(app, &menu_home, event))
        .on_tray_icon_event(|icon, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                shell::post(icon.app_handle(), Action::TogglePanel);
            }
        })
        .build(app)?;
    Ok(Tray {
        icon,
        home,
        pill_item,
        state: TrayState::Quiet,
        tooltip: tooltip(None),
        text,
    })
}

fn handle_menu(app: &AppHandle, home: &Path, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => shell::post(app, Action::OpenPanel),
        "snooze-5" => feed::send(
            home.to_path_buf(),
            Request::Snooze {
                session_id: None,
                minutes: 5,
            },
        ),
        "snooze-15" => feed::send(
            home.to_path_buf(),
            Request::Snooze {
                session_id: None,
                minutes: 15,
            },
        ),
        "clear-snooze" => feed::send(home.to_path_buf(), Request::Unsnooze { session_id: None }),
        "toggle-pill" => shell::post(app, Action::TogglePill),
        "quit" => shell::post(app, Action::Quit),
        other => match other.split_once(':') {
            Some((SHOW_ON, display)) => shell::post(app, Action::ShowOn(display.to_string())),
            Some((AVOID, display)) => shell::post(app, Action::ToggleAvoided(display.to_string())),
            Some((CORNER, key)) => {
                if let Some(corner) = corner(key) {
                    shell::post(app, Action::MoveToCorner(corner));
                }
            }
            Some((EDGE, key)) => {
                if let Some(edge) = edge(key) {
                    shell::post(app, Action::MoveToEdge(edge));
                }
            }
            Some((STYLE, key)) => {
                if let Some(style) = style(key) {
                    shell::post(app, Action::SetStyle(style));
                }
            }
            _ => {}
        },
    }
}

impl Tray {
    pub fn show(&mut self, snapshot: Option<&Snapshot>) {
        let state = tray_state(snapshot);
        if state != self.state {
            self.state = state;
            if let Err(error) = self.icon.set_icon(Some(image(state, &self.text))) {
                log_error!("cannot update the tray icon: {error}");
            }
        }
        let tooltip = tooltip(snapshot);
        if tooltip != self.tooltip {
            if let Err(error) = self.icon.set_tooltip(Some(&tooltip)) {
                log_error!("cannot update the tray tooltip: {error}");
            }
            self.tooltip = tooltip;
        }
    }

    pub fn refresh_displays(&self) {
        let rebuilt = menu(
            self.icon.app_handle(),
            &self.pill_item,
            &display_menu(&self.home),
        );
        if let Err(error) = rebuilt.and_then(|menu| self.icon.set_menu(Some(menu))) {
            log_error!("cannot rebuild the tray menu: {error}");
        }
    }

    pub fn rect(&self) -> Option<Rect> {
        let rect = self.icon.rect().ok().flatten()?;
        let (position, size) = (
            rect.position.to_physical::<i32>(1.0),
            rect.size.to_physical::<i32>(1.0),
        );
        Some(Rect::sized(position.x, position.y, size.width, size.height))
    }

    pub fn show_pill_state(&self, shown: bool) {
        if let Err(error) = self
            .pill_item
            .set_text(pill_label(shown, positions::style(&self.home)))
        {
            log_error!("cannot relabel the pill menu item: {error}");
        }
    }
}
