use crate::displays::DisplayChoice;
use crate::geometry::{Rect, TabPlacement};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

const UI_FILE: &str = "ui.json";
const DEFAULT_HOTKEY: &str = "Ctrl+Alt+Space";
const IDLE_OPACITY: (f64, f64, f64) = (0.2, 0.6, 1.0);
const PEEK_SECONDS: (f64, f64, f64) = (2.0, 4.0, 10.0);
const REPEAT_SECONDS: [u64; 5] = [0, 30, 60, 120, 300];
const DEFAULT_REPEAT_SECONDS: u64 = 60;
const QUIT_MINUTES: [u64; 4] = [0, 5, 15, 60];
const DEFAULT_QUIT_MINUTES: u64 = 15;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    #[default]
    Tab,
    Pill,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Always,
    #[default]
    Active,
    Attention,
    Never,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlaySize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionStyle {
    Colour,
    Pulse,
    #[default]
    PulsePeek,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartMode {
    Login,
    #[default]
    Claude,
    Manual,
}

impl OverlaySize {
    pub fn tab_length(self) -> f64 {
        match self {
            OverlaySize::Small => 56.0,
            OverlaySize::Medium => 72.0,
            OverlaySize::Large => 96.0,
        }
    }

    pub fn tab_factor(self) -> f64 {
        match self {
            OverlaySize::Small => 0.75,
            OverlaySize::Medium => 1.0,
            OverlaySize::Large => 1.5,
        }
    }

    pub fn pill_factor(self) -> f64 {
        match self {
            OverlaySize::Small => 0.85,
            OverlaySize::Medium => 1.0,
            OverlaySize::Large => 1.2,
        }
    }
}

fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Ok(serde_json::from_value(serde_json::Value::deserialize(deserializer)?).ok())
}

fn clamped(value: Option<f64>, (low, default, high): (f64, f64, f64)) -> f64 {
    value
        .filter(|value| value.is_finite())
        .map_or(default, |value| value.clamp(low, high))
}

fn one_of(value: Option<u64>, allowed: &[u64], default: u64) -> u64 {
    value
        .filter(|value| allowed.contains(value))
        .unwrap_or(default)
}

#[derive(Default, Serialize, Deserialize)]
struct UiFile {
    #[serde(default)]
    pill_positions: BTreeMap<String, Rect>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    display_positions: BTreeMap<String, Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    style: Option<Style>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tab_positions: BTreeMap<String, TabPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hotkey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toasts_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toast_on_finished: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sounds_enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    visibility: Option<Visibility>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    reveal_on_hover: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    size: Option<OverlaySize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    idle_opacity: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    attention_style: Option<AttentionStyle>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    attention_repeat_seconds: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    peek_seconds: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    start_mode: Option<StartMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient"
    )]
    quit_when_idle_minutes: Option<u64>,
    #[serde(flatten)]
    choice: DisplayChoice,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiSettings {
    pub hotkey: String,
    pub toasts_enabled: bool,
    pub toast_on_finished: bool,
    pub sounds_enabled: bool,
    pub visibility: Visibility,
    pub reveal_on_hover: bool,
    pub size: OverlaySize,
    pub idle_opacity: f64,
    pub attention_style: AttentionStyle,
    pub attention_repeat_seconds: u64,
    pub peek_seconds: f64,
    pub start_mode: StartMode,
    pub quit_when_idle_minutes: u64,
}

#[derive(Debug, Default, Deserialize)]
pub struct UiSettingsChange {
    pub hotkey: Option<String>,
    pub toasts_enabled: Option<bool>,
    pub toast_on_finished: Option<bool>,
    pub sounds_enabled: Option<bool>,
    pub visibility: Option<Visibility>,
    pub reveal_on_hover: Option<bool>,
    pub size: Option<OverlaySize>,
    pub idle_opacity: Option<f64>,
    pub attention_style: Option<AttentionStyle>,
    pub attention_repeat_seconds: Option<u64>,
    pub peek_seconds: Option<f64>,
    pub start_mode: Option<StartMode>,
    pub quit_when_idle_minutes: Option<u64>,
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
    settings_of(read(home))
}

impl Default for UiSettings {
    fn default() -> UiSettings {
        settings_of(UiFile::default())
    }
}

fn settings_of(file: UiFile) -> UiSettings {
    UiSettings {
        hotkey: file.hotkey.unwrap_or_else(|| DEFAULT_HOTKEY.to_string()),
        toasts_enabled: file.toasts_enabled.unwrap_or(true),
        toast_on_finished: file.toast_on_finished.unwrap_or(true),
        sounds_enabled: file.sounds_enabled.unwrap_or(false),
        visibility: file.visibility.unwrap_or_default(),
        reveal_on_hover: file.reveal_on_hover.unwrap_or(true),
        size: file.size.unwrap_or_default(),
        idle_opacity: clamped(file.idle_opacity, IDLE_OPACITY),
        attention_style: file.attention_style.unwrap_or_default(),
        attention_repeat_seconds: one_of(
            file.attention_repeat_seconds,
            &REPEAT_SECONDS,
            DEFAULT_REPEAT_SECONDS,
        ),
        peek_seconds: clamped(file.peek_seconds, PEEK_SECONDS),
        start_mode: file.start_mode.unwrap_or_default(),
        quit_when_idle_minutes: one_of(
            file.quit_when_idle_minutes,
            &QUIT_MINUTES,
            DEFAULT_QUIT_MINUTES,
        ),
    }
}

pub fn save_ui_settings(home: &Path, change: UiSettingsChange) -> io::Result<()> {
    update(home, |file| {
        file.hotkey = change.hotkey.or(file.hotkey.take());
        file.toasts_enabled = change.toasts_enabled.or(file.toasts_enabled);
        file.toast_on_finished = change.toast_on_finished.or(file.toast_on_finished);
        file.sounds_enabled = change.sounds_enabled.or(file.sounds_enabled);
        file.visibility = change.visibility.or(file.visibility);
        file.reveal_on_hover = change.reveal_on_hover.or(file.reveal_on_hover);
        file.size = change.size.or(file.size);
        file.idle_opacity = change.idle_opacity.or(file.idle_opacity);
        file.attention_style = change.attention_style.or(file.attention_style);
        file.attention_repeat_seconds = change
            .attention_repeat_seconds
            .or(file.attention_repeat_seconds);
        file.peek_seconds = change.peek_seconds.or(file.peek_seconds);
        file.start_mode = change.start_mode.or(file.start_mode);
        file.quit_when_idle_minutes = change
            .quit_when_idle_minutes
            .or(file.quit_when_idle_minutes);
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

pub fn style(home: &Path) -> Style {
    read(home).style.unwrap_or_default()
}

pub fn save_style(home: &Path, style: Style) -> io::Result<()> {
    update(home, |file| file.style = Some(style))
}

pub fn load_tab(home: &Path, display: &str) -> Option<TabPlacement> {
    read(home).tab_positions.remove(display)
}

pub fn save_tab(home: &Path, display: &str, placement: TabPlacement) -> io::Result<()> {
    update(home, |file| {
        file.tab_positions.insert(display.to_string(), placement);
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
                sounds_enabled: false,
                ..UiSettings::default()
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
                sounds_enabled: false,
                ..UiSettings::default()
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

    #[test]
    fn sounds_are_off_by_default_and_round_trip_beside_every_existing_key() {
        let home = std::env::temp_dir().join(format!("beacon-sounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let existing = r#"{"pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}},"display_positions":{"philips":{"left":5,"top":6,"right":12,"bottom":14}},"style":"pill","tab_positions":{"philips":{"edge":"top","offset":0.25}},"hotkey":"Ctrl+Shift+B","toasts_enabled":false,"toast_on_finished":true,"display":"philips","avoid_displays":["oled"]}"#;
        std::fs::write(home.join(UI_FILE), existing).unwrap();
        assert!(!ui_settings(&home).sounds_enabled);
        save_ui_settings(
            &home,
            UiSettingsChange {
                sounds_enabled: Some(true),
                ..UiSettingsChange::default()
            },
        )
        .unwrap();
        save(&home, "a", Rect::sized(1, 2, 3, 4)).unwrap();
        assert_eq!(
            ui_settings(&home),
            UiSettings {
                hotkey: "Ctrl+Shift+B".to_string(),
                toasts_enabled: false,
                toast_on_finished: true,
                sounds_enabled: true,
                ..UiSettings::default()
            }
        );
        let mut expected: serde_json::Value = serde_json::from_str(existing).unwrap();
        expected["sounds_enabled"] = true.into();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(home.join(UI_FILE)).unwrap()
            )
            .unwrap(),
            expected
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn overlay_settings_round_trip_beside_every_existing_key_and_bad_values_fall_back() {
        let home = std::env::temp_dir().join(format!("beacon-overlay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let existing = r#"{"pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}},"display_positions":{"philips":{"left":5,"top":6,"right":12,"bottom":14}},"style":"pill","tab_positions":{"philips":{"edge":"top","offset":0.25}},"hotkey":"Ctrl+Shift+B","toasts_enabled":false,"toast_on_finished":true,"sounds_enabled":true,"display":"philips","avoid_displays":["oled"]}"#;
        std::fs::write(home.join(UI_FILE), existing).unwrap();
        let defaults = ui_settings(&home);
        assert_eq!(
            (
                defaults.visibility,
                defaults.reveal_on_hover,
                defaults.size,
                defaults.idle_opacity,
                defaults.attention_style,
                defaults.attention_repeat_seconds,
                defaults.peek_seconds,
                defaults.start_mode,
                defaults.quit_when_idle_minutes,
            ),
            (
                Visibility::Active,
                true,
                OverlaySize::Medium,
                0.6,
                AttentionStyle::PulsePeek,
                60,
                4.0,
                StartMode::Claude,
                15,
            )
        );
        let change: UiSettingsChange = serde_json::from_str(
            r#"{"visibility":"attention","reveal_on_hover":false,"size":"large","idle_opacity":0.35,"attention_style":"pulse","attention_repeat_seconds":120,"peek_seconds":7.5,"start_mode":"manual","quit_when_idle_minutes":60}"#,
        )
        .unwrap();
        save_ui_settings(&home, change).unwrap();
        save_style(&home, Style::Pill).unwrap();
        assert_eq!(
            ui_settings(&home),
            UiSettings {
                hotkey: "Ctrl+Shift+B".to_string(),
                toasts_enabled: false,
                toast_on_finished: true,
                sounds_enabled: true,
                visibility: Visibility::Attention,
                reveal_on_hover: false,
                size: OverlaySize::Large,
                idle_opacity: 0.35,
                attention_style: AttentionStyle::Pulse,
                attention_repeat_seconds: 120,
                peek_seconds: 7.5,
                start_mode: StartMode::Manual,
                quit_when_idle_minutes: 60,
            }
        );
        let mut expected: serde_json::Value = serde_json::from_str(existing).unwrap();
        for (key, value) in serde_json::json!({"visibility":"attention","reveal_on_hover":false,"size":"large","idle_opacity":0.35,"attention_style":"pulse","attention_repeat_seconds":120,"peek_seconds":7.5,"start_mode":"manual","quit_when_idle_minutes":60}).as_object().unwrap() {
            expected[key] = value.clone();
        }
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.join(UI_FILE)).unwrap()).unwrap();
        assert_eq!(written, expected);
        assert_eq!(
            serde_json::to_value(ui_settings(&home)).unwrap()["attention_style"],
            "pulse"
        );
        expected["visibility"] = "sometimes".into();
        expected["size"] = 3.into();
        expected["idle_opacity"] = 5.0.into();
        expected["attention_style"] = "pulse_peek".into();
        expected["attention_repeat_seconds"] = 45.into();
        expected["peek_seconds"] = 0.5.into();
        expected["start_mode"] = serde_json::Value::Null;
        expected["quit_when_idle_minutes"] = (-5).into();
        std::fs::write(home.join(UI_FILE), expected.to_string()).unwrap();
        let fallen_back = ui_settings(&home);
        assert_eq!(fallen_back.visibility, Visibility::Active);
        assert_eq!(fallen_back.size, OverlaySize::Medium);
        assert_eq!(fallen_back.idle_opacity, 1.0);
        assert_eq!(fallen_back.attention_style, AttentionStyle::PulsePeek);
        assert_eq!(fallen_back.attention_repeat_seconds, 60);
        assert_eq!(fallen_back.peek_seconds, 2.0);
        assert_eq!(fallen_back.start_mode, StartMode::Claude);
        assert_eq!(fallen_back.quit_when_idle_minutes, 15);
        assert_eq!(fallen_back.hotkey, "Ctrl+Shift+B");
        assert!(fallen_back.sounds_enabled);
        assert_eq!(load(&home, "a"), Some(Rect::sized(1, 2, 3, 4)));
        assert_eq!(style(&home), Style::Pill);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_style_and_tab_placements_round_trip_beside_every_existing_key() {
        use crate::geometry::Edge;
        let home = std::env::temp_dir().join(format!("beacon-tab-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let existing = r#"{"pill_positions":{"a":{"left":1,"top":2,"right":4,"bottom":6}},"display_positions":{"philips":{"left":5,"top":6,"right":12,"bottom":14}},"hotkey":"Ctrl+Shift+B","toasts_enabled":false,"toast_on_finished":true,"display":"philips","avoid_displays":["oled"]}"#;
        std::fs::write(home.join(UI_FILE), existing).unwrap();
        assert_eq!(style(&home), Style::Tab);
        assert_eq!(load_tab(&home, "philips"), None);
        save_style(&home, Style::Pill).unwrap();
        let placement = TabPlacement {
            edge: Edge::Top,
            offset: 0.25,
        };
        save_tab(&home, "philips", placement).unwrap();
        assert_eq!(style(&home), Style::Pill);
        assert_eq!(load_tab(&home, "philips"), Some(placement));
        assert_eq!(load_tab(&home, "oled"), None);
        let mut expected: serde_json::Value = serde_json::from_str(existing).unwrap();
        expected["style"] = "pill".into();
        expected["tab_positions"] = serde_json::json!({"philips": {"edge": "top", "offset": 0.25}});
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
