use crate::displays::{
    self, AVOID, CORNER, DisplayMenu, EDGE, MenuEntry, SHOW_ON, STYLE, corner, edge, find_key,
    style,
};
use crate::feed;
use crate::geometry::Rect;
use crate::model::{TrayState, tooltip, tray_state};
use crate::platform::{self, pill, theme, tray_icon_size};
use crate::positions::{
    self, OverlaySize, PillBackground, Style, UiSettings, UiSettingsChange, Visibility,
};
use crate::raster::render_tray;
use crate::shell::{self, Action};
use beacon_core::{Request, Snapshot};
use std::path::{Path, PathBuf};
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

const SNOOZE: &str = "snooze";
const VISIBILITY: &str = "visibility";
const SIZE: &str = "size";
const BACKGROUND: &str = "pill-background";
const SNOOZE_MINUTES: [(u64, &str); 3] = [
    (5, "Snooze for 5 minutes"),
    (15, "Snooze for 15 minutes"),
    (60, "Snooze for 1 hour"),
];
const VISIBILITIES: [(Visibility, &str, &str); 4] = [
    (Visibility::Always, "always", "Always"),
    (Visibility::Active, "active", "While Claude is active"),
    (
        Visibility::Attention,
        "attention",
        "Only when Claude needs you",
    ),
    (Visibility::Never, "never", "Never"),
];
const SIZES: [(OverlaySize, &str, &str); 3] = [
    (OverlaySize::Small, "small", "Small"),
    (OverlaySize::Medium, "medium", "Medium"),
    (OverlaySize::Large, "large", "Large"),
];
const BACKGROUNDS: [(PillBackground, &str, &str); 2] = [
    (PillBackground::Solid, "solid", "Solid"),
    (PillBackground::Clear, "none", "None"),
];

type Item = Box<dyn IsMenuItem<Wry>>;

pub struct Tray {
    icon: TrayIcon,
    home: PathBuf,
    state: TrayState,
    tooltip: String,
    snoozed: bool,
}

fn image(state: TrayState) -> Image<'static> {
    let size = tray_icon_size();
    Image::new_owned(
        render_tray(state, size, theme::taskbar()).straight_rgba(),
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
    .checking(pill::position())
}

fn choices<T: Copy + PartialEq>(
    kind: &str,
    table: &[(T, &str, &str)],
    current: T,
) -> Vec<MenuEntry> {
    table
        .iter()
        .map(|(value, key, label)| MenuEntry {
            id: format!("{kind}:{key}"),
            label: label.to_string(),
            checked: Some(*value == current),
            enabled: true,
        })
        .collect()
}

fn plain(app: &AppHandle, id: &str, label: &str, enabled: bool) -> tauri::Result<Item> {
    Ok(Box::new(MenuItem::with_id(
        app,
        id,
        label,
        enabled,
        None::<&str>,
    )?))
}

fn checkable(
    app: &AppHandle,
    id: &str,
    label: &str,
    enabled: bool,
    checked: bool,
) -> tauri::Result<Item> {
    Ok(Box::new(CheckMenuItem::with_id(
        app,
        id,
        label,
        enabled,
        checked,
        None::<&str>,
    )?))
}

fn separator(app: &AppHandle) -> tauri::Result<Item> {
    Ok(Box::new(PredefinedMenuItem::separator(app)?))
}

fn submenu(app: &AppHandle, title: &str, items: Vec<Item>) -> tauri::Result<Item> {
    let items: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Ok(Box::new(Submenu::with_items(app, title, true, &items)?))
}

fn entries(app: &AppHandle, title: &str, entries: &[MenuEntry]) -> tauri::Result<Item> {
    submenu(
        app,
        title,
        entries
            .iter()
            .map(|entry| match entry.checked {
                Some(checked) => checkable(app, &entry.id, &entry.label, entry.enabled, checked),
                None => plain(app, &entry.id, &entry.label, entry.enabled),
            })
            .collect::<tauri::Result<_>>()?,
    )
}

fn menu(app: &AppHandle, home: &Path, snoozed: bool) -> tauri::Result<Menu<Wry>> {
    let settings = positions::ui_settings(home);
    let displays = display_menu(home);
    let mut snooze = SNOOZE_MINUTES
        .iter()
        .map(|(minutes, label)| plain(app, &format!("{SNOOZE}:{minutes}"), label, true))
        .collect::<tauri::Result<Vec<_>>>()?;
    snooze.push(separator(app)?);
    snooze.push(plain(app, "end-snooze", "End snooze", snoozed)?);
    let mut overlay = vec![entries(app, "Style", &displays.style)?];
    if positions::style(home) == Style::Pill {
        overlay.push(entries(
            app,
            "Pill background",
            &choices(BACKGROUND, &BACKGROUNDS, settings.pill_background),
        )?);
    }
    overlay.extend([
        entries(
            app,
            "Show it",
            &choices(VISIBILITY, &VISIBILITIES, settings.visibility),
        )?,
        entries(app, "Size", &choices(SIZE, &SIZES, settings.size))?,
        entries(app, "Display", &displays.show_on)?,
        entries(app, "Never show on", &displays.never_show_on)?,
        entries(app, "Position", &displays.position)?,
        checkable(
            app,
            "reveal-on-hover",
            "Reveal on hover",
            true,
            settings.reveal_on_hover,
        )?,
        checkable(
            app,
            "open-on-hover",
            "Open panel on hover",
            true,
            settings.open_on_hover,
        )?,
    ]);
    let attention = vec![
        checkable(
            app,
            "notifications",
            "Notifications",
            true,
            settings.toasts_enabled,
        )?,
        checkable(
            app,
            "also-finished",
            "Also when finished",
            true,
            settings.toast_on_finished,
        )?,
        checkable(app, "sounds", "Sounds", true, settings.sounds_enabled)?,
    ];
    let items = [
        plain(app, "open", "Open Beacon", true)?,
        separator(app)?,
        submenu(app, "Snooze", snooze)?,
        separator(app)?,
        submenu(app, "Overlay", overlay)?,
        submenu(app, "Attention", attention)?,
        separator(app)?,
        plain(app, "settings", "Settings…", true)?,
        plain(app, "quit", "Quit Beacon", true)?,
    ];
    let items: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &items)
}

pub fn build(app: &AppHandle, home: PathBuf) -> tauri::Result<Tray> {
    let menu = menu(app, &home, false)?;
    let menu_home = home.clone();
    let icon = TrayIconBuilder::with_id("beacon")
        .icon(image(TrayState::Quiet))
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
        state: TrayState::Quiet,
        tooltip: tooltip(None),
        snoozed: false,
    })
}

pub fn menu_settings_changed(before: &UiSettings, after: &UiSettings) -> bool {
    let shown = |settings: &UiSettings| {
        (
            settings.pill_background,
            settings.visibility,
            settings.size,
            settings.reveal_on_hover,
            settings.open_on_hover,
            settings.toasts_enabled,
            settings.toast_on_finished,
            settings.sounds_enabled,
        )
    };
    shown(before) != shown(after)
}

fn flipped(home: &Path, id: &str) -> Option<UiSettingsChange> {
    let current = positions::ui_settings(home);
    let change = UiSettingsChange::default();
    Some(match id {
        "reveal-on-hover" => UiSettingsChange {
            reveal_on_hover: Some(!current.reveal_on_hover),
            ..change
        },
        "open-on-hover" => UiSettingsChange {
            open_on_hover: Some(!current.open_on_hover),
            ..change
        },
        "notifications" => UiSettingsChange {
            toasts_enabled: Some(!current.toasts_enabled),
            ..change
        },
        "also-finished" => UiSettingsChange {
            toast_on_finished: Some(!current.toast_on_finished),
            ..change
        },
        "sounds" => UiSettingsChange {
            sounds_enabled: Some(!current.sounds_enabled),
            ..change
        },
        _ => return None,
    })
}

fn chosen(kind: &str, key: &str) -> Option<UiSettingsChange> {
    let change = UiSettingsChange::default();
    match kind {
        VISIBILITY => find_key(&VISIBILITIES, key).map(|visibility| UiSettingsChange {
            visibility: Some(visibility),
            ..change
        }),
        SIZE => find_key(&SIZES, key).map(|size| UiSettingsChange {
            size: Some(size),
            ..change
        }),
        BACKGROUND => find_key(&BACKGROUNDS, key).map(|background| UiSettingsChange {
            pill_background: Some(background),
            ..change
        }),
        _ => None,
    }
}

fn handle_menu(app: &AppHandle, home: &Path, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "open" => shell::post(app, Action::OpenPanel),
        "settings" => shell::post(app, Action::OpenSettings),
        "end-snooze" => feed::send(home.to_path_buf(), Request::Unsnooze { session_id: None }),
        "quit" => shell::post(app, Action::Quit),
        _ => {
            if let Some(change) = flipped(home, id) {
                return shell::post(app, Action::ChangeSettings(change));
            }
            let Some((kind, key)) = id.split_once(':') else {
                return;
            };
            if let Some(change) = chosen(kind, key) {
                return shell::post(app, Action::ChangeSettings(change));
            }
            match kind {
                SNOOZE => {
                    if let Ok(minutes) = key.parse() {
                        feed::send(
                            home.to_path_buf(),
                            Request::Snooze {
                                session_id: None,
                                minutes,
                            },
                        );
                    }
                }
                SHOW_ON => shell::post(app, Action::ShowOn(key.to_string())),
                AVOID => shell::post(app, Action::ToggleAvoided(key.to_string())),
                CORNER => {
                    if let Some(corner) = corner(key) {
                        shell::post(app, Action::MoveToCorner(corner));
                    }
                }
                EDGE => {
                    if let Some(edge) = edge(key) {
                        shell::post(app, Action::MoveToEdge(edge));
                    }
                }
                STYLE => {
                    if let Some(style) = style(key) {
                        shell::post(app, Action::SetStyle(style));
                    }
                }
                _ => {}
            }
        }
    }
}

impl Tray {
    pub fn show(&mut self, snapshot: Option<&Snapshot>) {
        let state = tray_state(snapshot);
        if state != self.state {
            let redraw = std::mem::discriminant(&state) != std::mem::discriminant(&self.state);
            self.state = state;
            if redraw {
                self.retheme();
            }
        }
        let tooltip = tooltip(snapshot);
        if tooltip != self.tooltip {
            if let Err(error) = self.icon.set_tooltip(Some(&tooltip)) {
                log_error!("cannot update the tray tooltip: {error}");
            }
            self.tooltip = tooltip;
        }
        let snoozed = snapshot.is_some_and(|snapshot| snapshot.snoozed_until_ms.is_some());
        if snoozed != self.snoozed {
            self.snoozed = snoozed;
            self.refresh_menu();
        }
    }

    pub fn retheme(&self) {
        if let Err(error) = self.icon.set_icon(Some(image(self.state))) {
            log_error!("cannot update the tray icon: {error}");
        }
    }

    pub fn refresh_menu(&self) {
        let rebuilt = menu(self.icon.app_handle(), &self.home, self.snoozed);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("beacon-tray-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    #[test]
    fn a_checkable_item_flips_the_current_value_of_its_setting() {
        let home = home("flip");
        let change = flipped(&home, "reveal-on-hover").unwrap();
        assert_eq!(change.reveal_on_hover, Some(false));
        positions::save_ui_settings(&home, change).unwrap();
        assert_eq!(
            flipped(&home, "reveal-on-hover").unwrap().reveal_on_hover,
            Some(true)
        );
        assert_eq!(
            flipped(&home, "notifications").unwrap().toasts_enabled,
            Some(false)
        );
        assert_eq!(
            flipped(&home, "also-finished").unwrap().toast_on_finished,
            Some(false)
        );
        assert_eq!(flipped(&home, "sounds").unwrap().sounds_enabled, Some(true));
        assert_eq!(
            flipped(&home, "open-on-hover").unwrap().open_on_hover,
            Some(false)
        );
        assert!(flipped(&home, "quit").is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn only_the_settings_the_menu_shows_rebuild_it() {
        let before = UiSettings::default();
        let changed = |edit: fn(&mut UiSettings)| {
            let mut after = before.clone();
            edit(&mut after);
            menu_settings_changed(&before, &after)
        };
        assert!(changed(
            |settings| settings.sounds_enabled = !settings.sounds_enabled
        ));
        assert!(changed(|settings| settings.visibility = Visibility::Never));
        assert!(changed(|settings| settings.size = OverlaySize::Large));
        assert!(!changed(|settings| settings.idle_opacity = 0.2));
        assert!(!changed(
            |settings| settings.hotkey = "Ctrl+Alt+K".to_string()
        ));
        assert!(!changed(|settings| settings.quit_when_idle_minutes = 99));
    }

    #[test]
    fn a_choice_maps_to_one_setting_and_unknown_keys_to_none() {
        assert_eq!(
            chosen(VISIBILITY, "attention").unwrap().visibility,
            Some(Visibility::Attention)
        );
        assert_eq!(
            chosen(SIZE, "large").unwrap().size,
            Some(OverlaySize::Large)
        );
        assert_eq!(
            chosen(BACKGROUND, "none").unwrap().pill_background,
            Some(PillBackground::Clear)
        );
        assert!(chosen(SIZE, "huge").is_none());
        assert!(chosen("colour", "red").is_none());
    }

    #[test]
    fn the_choices_check_exactly_the_current_value() {
        let entries = choices(VISIBILITY, &VISIBILITIES, Visibility::Active);
        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.id.as_str(), entry.label.as_str(), entry.checked))
                .collect::<Vec<_>>(),
            [
                ("visibility:always", "Always", Some(false)),
                ("visibility:active", "While Claude is active", Some(true)),
                (
                    "visibility:attention",
                    "Only when Claude needs you",
                    Some(false)
                ),
                ("visibility:never", "Never", Some(false)),
            ]
        );
    }
}
