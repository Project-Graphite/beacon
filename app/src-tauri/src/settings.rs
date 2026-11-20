use crate::feed::Feed;
use crate::platform::registry;
use crate::positions::{self, UiSettings, UiSettingsChange};
use crate::shell::{self, Action};
use beacon_cli::doctor::{self, HooksStatus};
use beacon_cli::installer::{self, Paths};
use serde::{Deserialize, Serialize};
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

#[tauri::command]
pub async fn get_ui_settings(feed: State<'_, Arc<Feed>>) -> Result<UiSettings, String> {
    Ok(positions::ui_settings(&feed.home))
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
    positions::save_ui_settings(&feed.home, change)
        .map_err(|error| format!("cannot save the settings: {error}"))?;
    Ok(positions::ui_settings(&feed.home))
}

#[tauri::command]
pub async fn get_login_start() -> Result<bool, String> {
    registry::has_value(RUN_KEY, RUN_VALUE).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_login_start(enabled: bool) -> Result<(), String> {
    if enabled {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        registry::write_string(RUN_KEY, RUN_VALUE, &login_command(&executable))
    } else {
        registry::delete_value(RUN_KEY, RUN_VALUE)
    }
    .map_err(|error| error.to_string())
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
