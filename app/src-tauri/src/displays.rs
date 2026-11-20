use crate::geometry::{
    Corner, Edge, Monitor, Rect, TabPlacement, monitor_for, near_edge, nearest_edge, primary, slide,
};
use crate::positions::Style;
use serde::{Deserialize, Serialize};

pub const SHOW_ON: &str = "show-on";
pub const AVOID: &str = "avoid";
pub const CORNER: &str = "corner";
pub const EDGE: &str = "edge";
pub const STYLE: &str = "style";

const CORNERS: [(Corner, &str, &str); 4] = [
    (Corner::TopLeft, "top-left", "Top left"),
    (Corner::TopRight, "top-right", "Top right"),
    (Corner::BottomLeft, "bottom-left", "Bottom left"),
    (Corner::BottomRight, "bottom-right", "Bottom right"),
];

const EDGES: [(Edge, &str, &str); 4] = [
    (Edge::Left, "left", "Left edge"),
    (Edge::Right, "right", "Right edge"),
    (Edge::Top, "top", "Top edge"),
    (Edge::Bottom, "bottom", "Bottom edge"),
];

const STYLES: [(Style, &str, &str); 2] = [
    (Style::Tab, "tab", "Edge tab"),
    (Style::Pill, "pill", "Pill"),
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayChoice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avoid_displays: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub checked: Option<bool>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMenu {
    pub show_on: Vec<MenuEntry>,
    pub never_show_on: Vec<MenuEntry>,
    pub position: Vec<MenuEntry>,
    pub style: Vec<MenuEntry>,
}

impl DisplayChoice {
    fn avoids(&self, monitor: &Monitor) -> bool {
        self.avoid_displays.contains(&monitor.id)
    }

    pub fn choose(&mut self, display: &str) {
        self.display = Some(display.to_string());
        self.avoid_displays.retain(|id| id != display);
    }

    pub fn toggle_avoided(&mut self, display: &str) {
        if self.avoid_displays.iter().any(|id| id == display) {
            self.avoid_displays.retain(|id| id != display);
        } else {
            self.avoid_displays.push(display.to_string());
        }
    }
}

pub fn target<'a>(monitors: &'a [Monitor], choice: &DisplayChoice) -> Option<&'a Monitor> {
    let allowed = |monitor: &&Monitor| !choice.avoids(monitor);
    monitors
        .iter()
        .find(|monitor| choice.display.as_ref() == Some(&monitor.id))
        .filter(allowed)
        .or_else(|| primary(monitors).filter(allowed))
        .or_else(|| monitors.iter().find(allowed))
}

pub fn landing<'a>(
    rect: &Rect,
    monitors: &'a [Monitor],
    choice: &DisplayChoice,
) -> Option<&'a Monitor> {
    monitor_for(rect, monitors).filter(|monitor| !choice.avoids(monitor))
}

pub fn dock<'a>(
    pointer: (i32, i32),
    from: &'a Monitor,
    edge: Edge,
    monitors: &'a [Monitor],
    choice: &DisplayChoice,
) -> Option<(&'a Monitor, TabPlacement)> {
    if near_edge(from, edge, pointer) {
        return Some((from, slide(&from.bounds, edge, pointer)));
    }
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(pointer))
        .filter(|monitor| !choice.avoids(monitor))?;
    Some((
        monitor,
        slide(
            &monitor.bounds,
            nearest_edge(&monitor.bounds, pointer),
            pointer,
        ),
    ))
}

fn find_key<T: Copy>(entries: &[(T, &str, &str)], key: &str) -> Option<T> {
    entries
        .iter()
        .find(|(_, name, _)| *name == key)
        .map(|(value, _, _)| *value)
}

pub fn corner(key: &str) -> Option<Corner> {
    find_key(&CORNERS, key)
}

pub fn edge(key: &str) -> Option<Edge> {
    find_key(&EDGES, key)
}

pub fn style(key: &str) -> Option<Style> {
    find_key(&STYLES, key)
}

fn label(monitor: &Monitor) -> String {
    format!(
        "{} ({}x{}{})",
        monitor.name,
        monitor.bounds.width(),
        monitor.bounds.height(),
        if monitor.primary { ", primary" } else { "" }
    )
}

pub fn menu(monitors: &[Monitor], choice: &DisplayChoice, style: Style) -> DisplayMenu {
    let target = target(monitors, choice);
    let entries = |kind: &str, checked: &dyn Fn(&Monitor) -> bool| -> Vec<MenuEntry> {
        monitors
            .iter()
            .map(|monitor| MenuEntry {
                id: format!("{kind}:{}", monitor.id),
                label: label(monitor),
                checked: Some(checked(monitor)),
                enabled: true,
            })
            .collect()
    };
    DisplayMenu {
        show_on: entries(SHOW_ON, &|monitor| {
            target.is_some_and(|target| target.id == monitor.id)
        }),
        never_show_on: entries(AVOID, &|monitor| choice.avoids(monitor)),
        position: match style {
            Style::Tab => EDGES
                .iter()
                .map(|(_, key, label)| (EDGE, *key, *label))
                .collect::<Vec<_>>(),
            Style::Pill => CORNERS
                .iter()
                .map(|(_, key, label)| (CORNER, *key, *label))
                .collect(),
        }
        .into_iter()
        .map(|(kind, key, label)| MenuEntry {
            id: format!("{kind}:{key}"),
            label: label.to_string(),
            checked: None,
            enabled: target.is_some(),
        })
        .collect(),
        style: STYLES
            .iter()
            .map(|(value, key, label)| MenuEntry {
                id: format!("{STYLE}:{key}"),
                label: label.to_string(),
                checked: Some(*value == style),
                enabled: true,
            })
            .collect(),
    }
}

pub fn listing(monitors: &[Monitor]) -> Vec<String> {
    monitors
        .iter()
        .map(|monitor| {
            format!(
                "{}\t{}\t{}x{}\t{},{}\t{}",
                monitor.id,
                monitor.name,
                monitor.bounds.width(),
                monitor.bounds.height(),
                monitor.bounds.left,
                monitor.bounds.top,
                if monitor.primary {
                    "primary"
                } else {
                    "secondary"
                }
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::tests::{monitor, owner_desk};

    fn desk() -> Vec<Monitor> {
        vec![
            monitor("oled", Rect::sized(0, 0, 2560, 1440), 48, 1.0, true),
            monitor(
                "philips",
                Rect::sized(-1080, -300, 1080, 1920),
                48,
                1.0,
                false,
            ),
            monitor("bar", Rect::sized(0, 1440, 1920, 464), 0, 1.0, false),
        ]
    }

    fn choice(display: Option<&str>, avoid: &[&str]) -> DisplayChoice {
        DisplayChoice {
            display: display.map(str::to_string),
            avoid_displays: avoid.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn target_id(choice: &DisplayChoice) -> Option<String> {
        target(&desk(), choice).map(|monitor| monitor.id.clone())
    }

    #[test]
    fn the_target_follows_the_choice_then_the_primary_then_any_allowed_display() {
        let id = |id: &str| Some(id.to_string());
        assert_eq!(target_id(&choice(None, &[])), id("oled"));
        assert_eq!(target_id(&choice(Some("philips"), &[])), id("philips"));
        assert_eq!(target_id(&choice(Some("gone"), &[])), id("oled"));
        assert_eq!(
            target_id(&choice(Some("philips"), &["philips"])),
            id("oled")
        );
        assert_eq!(target_id(&choice(None, &["oled"])), id("philips"));
        assert_eq!(target_id(&choice(Some("gone"), &["oled"])), id("philips"));
        assert_eq!(target_id(&choice(None, &["oled", "philips", "bar"])), None);
        assert_eq!(target(&[], &choice(None, &[])), None);
    }

    #[test]
    fn a_drop_lands_only_on_a_display_that_is_not_avoided() {
        let monitors = desk();
        let avoiding_oled = choice(Some("philips"), &["oled"]);
        let on_oled = Rect::sized(1000, 500, 100, 28);
        let on_philips = Rect::sized(-600, 500, 100, 28);
        assert_eq!(landing(&on_oled, &monitors, &avoiding_oled), None);
        assert_eq!(
            landing(&on_philips, &monitors, &avoiding_oled).map(|monitor| monitor.id.as_str()),
            Some("philips")
        );
        assert_eq!(
            landing(&on_oled, &monitors, &choice(None, &[])).map(|monitor| monitor.id.as_str()),
            Some("oled")
        );
    }

    #[test]
    fn corners_are_found_by_their_menu_key() {
        assert_eq!(corner("bottom-left"), Some(Corner::BottomLeft));
        assert_eq!(corner("middle"), None);
    }

    #[test]
    fn the_menu_labels_each_display_and_checks_the_target_and_the_avoided() {
        let menu = menu(&desk(), &choice(Some("philips"), &["oled"]), Style::Pill);
        let entry = |id: &str, label: &str, checked: Option<bool>, enabled: bool| MenuEntry {
            id: id.to_string(),
            label: label.to_string(),
            checked,
            enabled,
        };
        assert_eq!(
            menu.show_on,
            vec![
                entry(
                    "show-on:oled",
                    "oled (2560x1440, primary)",
                    Some(false),
                    true
                ),
                entry("show-on:philips", "philips (1080x1920)", Some(true), true),
                entry("show-on:bar", "bar (1920x464)", Some(false), true),
            ]
        );
        assert_eq!(
            menu.never_show_on,
            vec![
                entry("avoid:oled", "oled (2560x1440, primary)", Some(true), true),
                entry("avoid:philips", "philips (1080x1920)", Some(false), true),
                entry("avoid:bar", "bar (1920x464)", Some(false), true),
            ]
        );
        assert_eq!(
            menu.position,
            vec![
                entry("corner:top-left", "Top left", None, true),
                entry("corner:top-right", "Top right", None, true),
                entry("corner:bottom-left", "Bottom left", None, true),
                entry("corner:bottom-right", "Bottom right", None, true),
            ]
        );
    }

    #[test]
    fn without_a_target_nothing_is_shown_on_and_positions_are_disabled() {
        let menu = menu(
            &desk(),
            &choice(None, &["oled", "philips", "bar"]),
            Style::Pill,
        );
        assert!(
            menu.show_on
                .iter()
                .all(|entry| entry.checked == Some(false))
        );
        assert!(
            menu.never_show_on
                .iter()
                .all(|entry| entry.checked == Some(true))
        );
        assert!(menu.position.iter().all(|entry| !entry.enabled));
    }

    #[test]
    fn a_released_tab_slides_redocks_moves_display_or_is_undone() {
        let monitors = owner_desk();
        let philips = &monitors[1];
        let free = choice(Some(&philips.id), &[]);
        let docked = |pointer, choice: &DisplayChoice| {
            dock(pointer, philips, Edge::Top, &monitors, choice)
                .map(|(monitor, placement)| (monitor.id.clone(), placement))
        };
        let placed = |id: &str, edge, offset| Some((id.to_string(), TabPlacement { edge, offset }));
        assert_eq!(docked((-540, 70), &free), placed("philips", Edge::Top, 0.5));
        assert_eq!(
            docked((-1000, 960), &free),
            placed("philips", Edge::Left, 0.5)
        );
        assert_eq!(docked((2500, 720), &free), placed("oled", Edge::Right, 0.5));
        assert_eq!(
            docked((2500, 720), &choice(Some("philips"), &["oled"])),
            None
        );
    }

    #[test]
    fn the_tab_style_offers_edges_and_checks_the_active_style() {
        let menu = menu(&desk(), &choice(None, &[]), Style::Tab);
        let ids = |entries: &[MenuEntry]| -> Vec<String> {
            entries.iter().map(|entry| entry.id.clone()).collect()
        };
        assert_eq!(
            ids(&menu.position),
            ["edge:left", "edge:right", "edge:top", "edge:bottom"]
        );
        assert_eq!(menu.position[0].label, "Left edge");
        assert_eq!(ids(&menu.style), ["style:tab", "style:pill"]);
        assert_eq!(
            menu.style
                .iter()
                .map(|entry| entry.checked)
                .collect::<Vec<_>>(),
            [Some(true), Some(false)]
        );
        assert_eq!(edge("bottom"), Some(Edge::Bottom));
        assert_eq!(style("pill"), Some(Style::Pill));
        assert_eq!(style("square"), None);
    }

    #[test]
    fn the_listing_has_one_tab_separated_line_per_display() {
        assert_eq!(
            listing(&desk()[..2]),
            vec![
                "oled\toled\t2560x1440\t0,0\tprimary",
                "philips\tphilips\t1080x1920\t-1080,-300\tsecondary",
            ]
        );
    }
}
