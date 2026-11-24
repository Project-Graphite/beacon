use super::registry;
use crate::raster::Theme;
use windows_sys::Win32::Foundation::LPARAM;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const COLOURS_CHANGED: &str = "ImmersiveColorSet";

fn from_value(value: Option<u32>) -> Theme {
    if value == Some(0) {
        Theme::Dark
    } else {
        Theme::Light
    }
}

fn read(name: &str) -> Theme {
    from_value(registry::read_dword(KEY, name))
}

pub fn apps() -> Theme {
    read("AppsUseLightTheme")
}

pub fn taskbar() -> Theme {
    read("SystemUsesLightTheme")
}

pub fn colours_changed(setting: LPARAM) -> bool {
    named(setting as *const u16, COLOURS_CHANGED)
}

fn named(setting: *const u16, expected: &str) -> bool {
    !setting.is_null()
        && expected
            .encode_utf16()
            .chain([0])
            .enumerate()
            .all(|(index, unit)| unsafe { *setting.add(index) } == unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    #[test]
    fn a_missing_theme_value_is_light_like_the_windows_default() {
        assert_eq!(from_value(None), Theme::Light);
        assert_eq!(from_value(Some(1)), Theme::Light);
        assert_eq!(from_value(Some(0)), Theme::Dark);
    }

    #[test]
    fn only_the_immersive_colour_broadcast_counts_as_a_theme_change() {
        let matches = |text: &str| colours_changed(wide(text).as_ptr() as LPARAM);
        assert!(matches("ImmersiveColorSet"));
        assert!(!matches("ImmersiveColorSe"));
        assert!(!matches("ImmersiveColorSetX"));
        assert!(!matches("WindowMetrics"));
        assert!(!matches(""));
        assert!(!colours_changed(0));
    }
}
