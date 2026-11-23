use crate::displays::{self, Overlay, Position};
use crate::feed::Feed;
use crate::platform::{self, pill, registry};
use crate::positions::{self, StartMode, Style, UiSettings, UiSettingsChange};
use crate::shell::{self, Action};
use beacon_cli::doctor::{self, HooksStatus};
use beacon_cli::installer::{self, Paths};
use beacon_core::AUTOSTART_FILE;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Beacon";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HooksAction {
    Install,
    Uninstall,
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    home: PathBuf,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}

fn change_hooks(paths: &Paths, action: HooksAction, apply: bool) -> Result<String, String> {
    match action {
        HooksAction::Install => installer::install(paths, apply),
        HooksAction::Uninstall => installer::uninstall(paths, apply),
    }
}

pub fn is_installed(executable: &Path) -> bool {
    !executable.to_string_lossy().contains(r"\target\")
}

fn login_command(executable: &Path) -> String {
    format!("\"{}\"", executable.display())
}

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(hotkey, |app, _, event| {
            if event.state() == ShortcutState::Pressed {
                shell::post(app, Action::TogglePanel);
            }
        })
        .map_err(|error| format!("cannot register {hotkey}: {error}"))
}

fn change_hotkey(app: &AppHandle, current: &str, wanted: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    if shortcuts.is_registered(wanted) {
        return Ok(());
    }
    register_hotkey(app, wanted)?;
    if shortcuts.is_registered(current) {
        shortcuts
            .unregister(current)
            .map_err(|error| format!("cannot release {current}: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hooks_status() -> Result<HooksStatus, String> {
    blocking(|| Ok(doctor::hooks_status(&Paths::resolve(None)?))).await
}

#[tauri::command]
pub async fn hooks_plan(action: HooksAction) -> Result<String, String> {
    blocking(move || change_hooks(&Paths::resolve(None)?, action, false)).await
}

#[tauri::command]
pub async fn hooks_apply(action: HooksAction) -> Result<HooksStatus, String> {
    blocking(move || {
        let paths = Paths::resolve(None)?;
        change_hooks(&paths, action, true)?;
        Ok(doctor::hooks_status(&paths))
    })
    .await
}

pub fn apply_start_mode(home: &Path, mode: StartMode, sync_registry: bool) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let marker = home.join(AUTOSTART_FILE);
    if mode == StartMode::Claude {
        std::fs::create_dir_all(home)?;
        std::fs::write(&marker, executable.to_string_lossy().as_bytes())?;
    } else if let Err(error) = std::fs::remove_file(&marker)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(error);
    }
    match (sync_registry, mode) {
        (false, _) => Ok(()),
        (true, StartMode::Login) => {
            registry::write_string(RUN_KEY, RUN_VALUE, &login_command(&executable))
        }
        (true, _) => registry::delete_value(RUN_KEY, RUN_VALUE),
    }
}

fn change_settings(
    app: &AppHandle,
    home: &Path,
    change: UiSettingsChange,
) -> Result<UiSettings, String> {
    let start_mode_changed = change.start_mode.is_some();
    positions::save_ui_settings(home, change)
        .map_err(|error| format!("cannot save the settings: {error}"))?;
    let settings = positions::ui_settings(home);
    let applied = if start_mode_changed {
        std::env::current_exe()
            .and_then(|executable| {
                apply_start_mode(home, settings.start_mode, is_installed(&executable))
            })
            .map_err(|error| format!("cannot apply the start mode: {error}"))
    } else {
        Ok(())
    };
    shell::post(app, Action::SettingsChanged);
    applied.map(|()| settings)
}

#[tauri::command]
pub async fn set_ui_settings(
    app: AppHandle,
    feed: State<'_, Arc<Feed>>,
    change: UiSettingsChange,
) -> Result<UiSettings, String> {
    if let Some(hotkey) = &change.hotkey {
        change_hotkey(&app, &positions::hotkey(&feed.home), hotkey)?;
    }
    change_settings(&app, &feed.home, change)
}

#[tauri::command]
pub async fn get_login_start(feed: State<'_, Arc<Feed>>) -> Result<bool, String> {
    Ok(positions::ui_settings(&feed.home).start_mode == StartMode::Login)
}

#[tauri::command]
pub async fn set_login_start(
    app: AppHandle,
    feed: State<'_, Arc<Feed>>,
    enabled: bool,
) -> Result<(), String> {
    change_settings(
        &app,
        &feed.home,
        UiSettingsChange {
            start_mode: Some(if enabled {
                StartMode::Login
            } else {
                StartMode::Claude
            }),
            ..UiSettingsChange::default()
        },
    )
    .map(drop)
}

#[tauri::command]
pub async fn preview_attention(app: AppHandle) {
    shell::post(&app, Action::PreviewAttention);
}

#[tauri::command]
pub async fn open_logs_folder(feed: State<'_, Arc<Feed>>) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(feed.home.join("logs"))
        .spawn()
        .map(drop)
        .map_err(|error| format!("cannot open the logs folder: {error}"))
}

#[tauri::command]
pub fn get_overlay(feed: State<'_, Arc<Feed>>) -> Overlay {
    displays::overlay(
        &platform::monitors(),
        &positions::display_choice(&feed.home),
        positions::style(&feed.home),
        pill::position(),
    )
}

#[tauri::command]
pub fn set_style(style: Style) {
    shell::dispatch(Action::SetStyle(style));
}

#[tauri::command]
pub fn set_display(id: String) {
    shell::dispatch(Action::ShowOn(id));
}

#[tauri::command]
pub fn set_display_avoided(feed: State<'_, Arc<Feed>>, id: String, avoided: bool) {
    if positions::display_choice(&feed.home)
        .avoid_displays
        .contains(&id)
        != avoided
    {
        shell::dispatch(Action::ToggleAvoided(id));
    }
}

#[tauri::command]
pub fn set_overlay_position(position: String) -> Result<(), String> {
    shell::dispatch(
        match displays::position(&position)
            .ok_or_else(|| format!("unknown overlay position {position}"))?
        {
            Position::Edge(edge) => Action::MoveToEdge(edge),
            Position::Corner(corner) => Action::MoveToCorner(corner),
        },
    );
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    shell::post(&app, Action::Quit);
}

#[tauri::command]
pub async fn app_info(feed: State<'_, Arc<Feed>>) -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        home: feed.home.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_cli::doctor::HooksState;

    #[test]
    fn the_login_command_quotes_the_executable_path() {
        assert_eq!(
            login_command(Path::new(r"C:\Program Files\Beacon\beacon-app.exe")),
            r#""C:\Program Files\Beacon\beacon-app.exe""#
        );
    }

    #[test]
    fn a_build_under_target_is_not_the_installed_beacon() {
        assert!(!is_installed(Path::new(
            r"C:\src\beacon\target\fast\beacon-app.exe"
        )));
        assert!(is_installed(Path::new(
            r"C:\Users\amr\AppData\Local\Beacon\beacon-app.exe"
        )));
    }

    #[test]
    fn the_start_mode_keeps_exactly_one_of_the_marker_and_the_run_value() {
        assert!(
            !registry::tests::has_value(RUN_KEY, RUN_VALUE).unwrap(),
            "a Beacon Run value already exists; this test would replace it"
        );
        let home = std::env::temp_dir().join(format!("beacon-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let marker = home.join(AUTOSTART_FILE);
        apply_start_mode(&home, StartMode::Claude, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            std::env::current_exe().unwrap().to_string_lossy()
        );
        apply_start_mode(&home, StartMode::Login, true).unwrap();
        let registered = registry::tests::has_value(RUN_KEY, RUN_VALUE);
        let marker_after_login = marker.exists();
        apply_start_mode(&home, StartMode::Claude, true).unwrap();
        assert!(registered.unwrap());
        assert!(!marker_after_login);
        assert!(!registry::tests::has_value(RUN_KEY, RUN_VALUE).unwrap());
        assert!(marker.exists());
        apply_start_mode(&home, StartMode::Manual, true).unwrap();
        assert!(!marker.exists());
        assert!(!registry::tests::has_value(RUN_KEY, RUN_VALUE).unwrap());
        apply_start_mode(&home, StartMode::Manual, false).unwrap();
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_plan_writes_nothing_and_apply_installs_then_restores_the_file() {
        let root = std::env::temp_dir().join(format!("beacon-app-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths {
            settings: root.join("claude").join("settings.json"),
            home: root.join("home"),
            helper_source: root.join("beacon-hook.exe"),
            managed_dir: root.join("managed"),
        };
        std::fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        std::fs::write(&paths.helper_source, b"stand-in helper").unwrap();
        let original = b"{\n  \"model\": \"opus\"\n}\n".to_vec();
        std::fs::write(&paths.settings, &original).unwrap();
        let plan = change_hooks(&paths, HooksAction::Install, false).unwrap();
        assert!(plan.contains("+  \"hooks\": {"), "{plan}");
        assert!(plan.ends_with("Nothing was changed. Run again with --yes to install.\n"));
        assert_eq!(std::fs::read(&paths.settings).unwrap(), original);
        assert_eq!(doctor::hooks_status(&paths).state, HooksState::NotInstalled);
        change_hooks(&paths, HooksAction::Install, true).unwrap();
        assert_eq!(doctor::hooks_status(&paths).state, HooksState::Installed);
        assert_ne!(std::fs::read(&paths.settings).unwrap(), original);
        let plan = change_hooks(&paths, HooksAction::Uninstall, false).unwrap();
        assert!(plan.ends_with("Nothing was changed. Run again with --yes to uninstall.\n"));
        assert_eq!(doctor::hooks_status(&paths).state, HooksState::Installed);
        change_hooks(&paths, HooksAction::Uninstall, true).unwrap();
        assert_eq!(std::fs::read(&paths.settings).unwrap(), original);
        assert_eq!(doctor::hooks_status(&paths).state, HooksState::NotInstalled);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
