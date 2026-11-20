use crate::feed;
use crate::model::{TrayState, tooltip, tray_state};
use crate::platform::{GdiText, tray_icon_size};
use crate::raster::render_tray;
use crate::shell::{self, Action};
use beacon_core::{Request, Snapshot};
use std::path::{Path, PathBuf};
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

pub struct Tray {
    icon: TrayIcon,
    pill_item: MenuItem<Wry>,
    state: TrayState,
    tooltip: String,
    text: GdiText,
}

fn pill_label(shown: bool) -> &'static str {
    if shown { "Hide pill" } else { "Show pill" }
}

fn image(state: TrayState, text: &GdiText) -> Image<'static> {
    let size = tray_icon_size();
    Image::new_owned(
        render_tray(state, size, text).straight_rgba(),
        size as u32,
        size as u32,
    )
}

pub fn build(app: &AppHandle, home: PathBuf, pill_shown: bool) -> tauri::Result<Tray> {
    let pill_item = MenuItem::with_id(
        app,
        "toggle-pill",
        pill_label(pill_shown),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
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
            &pill_item,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    let text = GdiText::default();
    let icon = TrayIconBuilder::with_id("beacon")
        .icon(image(TrayState::Quiet, &text))
        .tooltip(tooltip(None))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| handle_menu(app, &home, event))
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
        _ => {}
    }
}

impl Tray {
    pub fn show(&mut self, snapshot: Option<&Snapshot>) {
        let state = tray_state(snapshot);
        if state != self.state {
            self.state = state;
            if let Err(error) = self.icon.set_icon(Some(image(state, &self.text))) {
                eprintln!("cannot update the tray icon: {error}");
            }
        }
        let tooltip = tooltip(snapshot);
        if tooltip != self.tooltip {
            if let Err(error) = self.icon.set_tooltip(Some(&tooltip)) {
                eprintln!("cannot update the tray tooltip: {error}");
            }
            self.tooltip = tooltip;
        }
    }

    pub fn show_pill_state(&self, shown: bool) {
        if let Err(error) = self.pill_item.set_text(pill_label(shown)) {
            eprintln!("cannot relabel the pill menu item: {error}");
        }
    }
}
