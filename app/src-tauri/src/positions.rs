use crate::displays::DisplayChoice;
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

const UI_FILE: &str = "ui.json";
const DEFAULT_HOTKEY: &str = "Ctrl+Alt+Space";

#[derive(Default, Serialize, Deserialize)]
struct UiFile {
    #[serde(default)]
    pill_positions: BTreeMap<String, Rect>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    display_positions: BTreeMap<String, Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hotkey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toasts_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toast_on_finished: Option<bool>,
    #[serde(flatten)]
    choice: DisplayChoice,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiSettings {
    pub hotkey: String,
    pub toasts_enabled: bool,
    pub toast_on_finished: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct UiSettingsChange {
    pub hotkey: Option<String>,
    pub toasts_enabled: Option<bool>,
    pub toast_on_finished: Option<bool>,
}

fn read(home: &Path) -> UiFile {
    std::fs::read(home.join(UI_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn update(home: &Path, change: impl FnOnce(&mut UiFile)) -> io::Result<()> {
    let mut file = read(home);
    change(&mut file);
    std::fs::create_dir_all(home)?;
    std::fs::write(home.join(UI_FILE), serde_json::to_vec_pretty(&file)?)
}

pub fn hotkey(home: &Path) -> String {
    ui_settings(home).hotkey
}

pub fn ui_settings(home: &Path) -> UiSettings {
    let file = read(home);
    UiSettings {
        hotkey: file.hotkey.unwrap_or_else(|| DEFAULT_HOTKEY.to_string()),
        toasts_enabled: file.toasts_enabled.unwrap_or(true),
        toast_on_finished: file.toast_on_finished.unwrap_or(true),
    }
}

pub fn save_ui_settings(home: &Path, change: UiSettingsChange) -> io::Result<()> {
    update(home, |file| {
        file.hotkey = change.hotkey.or(file.hotkey.take());
        file.toasts_enabled = change.toasts_enabled.or(file.toasts_enabled);
        file.toast_on_finished = change.toast_on_finished.or(file.toast_on_finished);
    })
}

pub fn load(home: &Path, signature: &str) -> Option<Rect> {
    read(home).pill_positions.remove(signature)
}

pub fn save(home: &Path, signature: &str, rect: Rect) -> io::Result<()> {
    update(home, |file| {
        file.pill_positions.insert(signature.to_string(), rect);
    })
}

pub fn load_for_display(home: &Path, display: &str) -> Option<Rect> {
    read(home).display_positions.remove(display)
}

pub fn save_for_display(home: &Path, display: &str, rect: Rect) -> io::Result<()> {
    update(home, |file| {
        file.display_positions.insert(display.to_string(), rect);
    })
}

pub fn display_choice(home: &Path) -> DisplayChoice {
    read(home).choice
}

pub fn choose_display(home: &Path, display: &str) -> io::Result<()> {
    update(home, |file| file.choice.choose(display))
}

pub fn toggle_avoided(home: &Path, display: &str) -> io::Result<()> {
    update(home, |file| file.choice.toggle_avoided(display))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_kept_per_layout_signature() {
        let home = std::env::temp_dir().join(format!("beacon-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        assert_eq!(load(&home, "a"), None);
        save(&home, "a", Rect::sized(1, 2, 3, 4)).unwrap();
        save(&home, "b", Rect::sized(5, 6, 7, 8)).unwrap();
        assert_eq!(load(&home, "a"), Some(Rect::sized(1, 2, 3, 4)));
        assert_eq!(load(&home, "b"), Some(Rect::sized(5, 6, 7, 8)));
        std::fs::write(home.join(UI_FILE), "not json").unwrap();
        assert_eq!(load(&home, "a"), None);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_hotkey_is_read_from_the_ui_file_and_survives_saving_positions() {
        let home = std::env::temp_dir().join(format!("beacon-hotkey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        assert_eq!(hotkey(&home), "Ctrl+Alt+Space");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(UI_FILE), r#"{"pill_positions":{}}"#).unwrap();
        assert_eq!(hotkey(&home), "Ctrl+Alt+Space");
        std::fs::write(
            home.join(UI_FILE),
            r#"{"hotkey":"Ctrl+Shift+B","pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}}}"#,
        )
        .unwrap();
        assert_eq!(hotkey(&home), "Ctrl+Shift+B");
        assert_eq!(load(&home, "a"), Some(Rect::sized(1, 2, 3, 4)));
        save(&home, "b", Rect::sized(5, 6, 7, 8)).unwrap();
        assert_eq!(hotkey(&home), "Ctrl+Shift+B");
        assert_eq!(load(&home, "a"), Some(Rect::sized(1, 2, 3, 4)));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn display_settings_round_trip_beside_the_hotkey_and_positions() {
        let home = std::env::temp_dir().join(format!("beacon-displays-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let legacy = r#"{"pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}},"hotkey":"Ctrl+Shift+B"}"#;
        std::fs::write(home.join(UI_FILE), legacy).unwrap();
        assert_eq!(display_choice(&home), DisplayChoice::default());
        save(&home, "a", Rect::sized(1, 2, 3, 4)).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(home.join(UI_FILE)).unwrap()
            )
            .unwrap(),
            serde_json::from_str::<serde_json::Value>(legacy).unwrap()
        );
        choose_display(&home, "philips").unwrap();
        toggle_avoided(&home, "oled").unwrap();
        save_for_display(&home, "philips", Rect::sized(-500, 400, 100, 28)).unwrap();
        assert_eq!(
            display_choice(&home),
            DisplayChoice {
                display: Some("philips".to_string()),
                avoid_displays: vec!["oled".to_string()],
            }
        );
        assert_eq!(
            load_for_display(&home, "philips"),
            Some(Rect::sized(-500, 400, 100, 28))
        );
        assert_eq!(load_for_display(&home, "oled"), None);
        assert_eq!(hotkey(&home), "Ctrl+Shift+B");
        assert_eq!(load(&home, "a"), Some(Rect::sized(1, 2, 3, 4)));
        toggle_avoided(&home, "philips").unwrap();
        choose_display(&home, "philips").unwrap();
        assert_eq!(
            display_choice(&home).avoid_displays,
            vec!["oled".to_string()]
        );
        toggle_avoided(&home, "oled").unwrap();
        assert!(display_choice(&home).avoid_displays.is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn ui_settings_round_trip_beside_the_existing_keys() {
        let home = std::env::temp_dir().join(format!("beacon-ui-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let existing = r#"{"pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}},"display_positions":{"philips":{"left":5,"top":6,"right":12,"bottom":14}},"hotkey":"Ctrl+Shift+B","display":"philips","avoid_displays":["oled"]}"#;
        std::fs::write(home.join(UI_FILE), existing).unwrap();
        assert_eq!(
            ui_settings(&home),
            UiSettings {
                hotkey: "Ctrl+Shift+B".to_string(),
                toasts_enabled: true,
                toast_on_finished: true,
            }
        );
        save_ui_settings(
            &home,
            UiSettingsChange {
                toast_on_finished: Some(false),
                ..UiSettingsChange::default()
            },
        )
        .unwrap();
        save_ui_settings(
            &home,
            UiSettingsChange {
                hotkey: Some("Ctrl+Alt+B".to_string()),
                toasts_enabled: Some(false),
                ..UiSettingsChange::default()
            },
        )
        .unwrap();
        assert_eq!(
            ui_settings(&home),
            UiSettings {
                hotkey: "Ctrl+Alt+B".to_string(),
                toasts_enabled: false,
                toast_on_finished: false,
            }
        );
        let mut expected: serde_json::Value = serde_json::from_str(existing).unwrap();
        expected["hotkey"] = "Ctrl+Alt+B".into();
        expected["toasts_enabled"] = false.into();
        expected["toast_on_finished"] = false.into();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(home.join(UI_FILE)).unwrap()
            )
            .unwrap(),
            expected
        );
        std::fs::remove_dir_all(&home).unwrap();
    }
}
