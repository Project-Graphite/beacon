#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod applog;
mod displays;
mod feed;
mod geometry;
mod jump;
mod model;
mod motion;
mod panel;
mod platform;
mod positions;
mod raster;
mod settings;
mod shell;
mod sounds;
mod toasts;
mod tray;

use beacon_core::BACKGROUND_ARG;
use feed::Feed;
use shell::{Action, Shell};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_notification::NotificationExt;

#[derive(Debug)]
struct Options {
    tray_only: bool,
    open_panel: bool,
    list_displays: bool,
}

const TOAST_ACTIVATED_ARGS: [&str; 2] = ["-toastactivated", "-embedding"];

fn options() -> Result<Options, String> {
    parse_options(std::env::args().skip(1))
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        tray_only: false,
        open_panel: false,
        list_displays: false,
    };
    for argument in arguments {
        match argument.as_str() {
            "--tray-only" => options.tray_only = true,
            "--open-panel" => options.open_panel = true,
            "--list-displays" => options.list_displays = true,
            BACKGROUND_ARG => {}
            other if TOAST_ACTIVATED_ARGS.contains(&other.to_ascii_lowercase().as_str()) => {
                options.open_panel = true;
            }
            other => {
                return Err(format!(
                    "unknown option {other}; expected --tray-only, --open-panel, --background or --list-displays"
                ));
            }
        }
    }
    Ok(options)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options()?;
    if options.list_displays {
        for line in displays::listing(&platform::monitors()) {
            println!("{line}");
        }
        return Ok(());
    }
    let feed = Feed::new(
        beacon_core::home_dir()
            .ok_or("cannot locate the Beacon home: set BEACON_HOME or LOCALAPPDATA")?,
    );
    if let Err(error) = applog::init(&feed.home) {
        eprintln!("cannot open the app log: {error}");
    }
    let mut builder = tauri::Builder::default();
    if std::env::var_os("BEACON_HOME").is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            if !arguments.iter().any(|argument| argument == BACKGROUND_ARG) {
                shell::post(app, Action::OpenPanel)
            }
        }));
    }
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(feed.clone())
        .invoke_handler(tauri::generate_handler![
            panel::get_detail,
            panel::get_ended,
            panel::acknowledge,
            panel::dismiss,
            panel::undismiss,
            panel::decide,
            panel::snooze,
            panel::unsnooze,
            panel::refresh_usage,
            panel::close_panel,
            panel::set_panel_height,
            jump::jump,
            jump::send_prompt_to_window,
            panel::queue_prompt,
            panel::cancel_prompt,
            panel::set_config,
            settings::hooks_status,
            settings::hooks_plan,
            settings::hooks_apply,
            settings::set_ui_settings,
            settings::preview_attention,
            settings::open_logs_folder,
            settings::app_info,
            settings::get_overlay,
            settings::set_style,
            settings::set_display,
            settings::set_display_avoided,
            settings::set_overlay_position,
            settings::quit_app,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                shell::post(window.app_handle(), Action::ClosePanel);
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let pill_shown = !options.tray_only;
            let tray = tray::build(&handle, feed.home.clone())?;
            platform::pill::create(feed.home.clone(), pill_shown, |event| {
                shell::dispatch(Action::Pill(event))
            })?;
            shell::install(Shell::new(handle.clone(), feed.clone(), tray));
            let clicked = handle.clone();
            handle
                .notification()
                .on_action(move |_| shell::post(&clicked, Action::OpenPanel))?;
            if let Err(error) = settings::register_hotkey(&handle, &positions::hotkey(&feed.home)) {
                log_error!("{error}");
            }
            let installed = settings::is_installed(&std::env::current_exe()?);
            if installed
                && let Err(error) =
                    toasts::register_identity(&feed.home, &handle.config().identifier)
            {
                log_error!("cannot register the notification identity: {error}");
            }
            if let Err(error) = settings::apply_start_mode(
                &feed.home,
                positions::ui_settings(&feed.home).start_mode,
                installed,
            ) {
                log_error!("cannot apply the start mode: {error}");
            }
            let publisher = handle.clone();
            feed.start(move |snapshot| shell::post(&publisher, Action::Snapshot(snapshot)))?;
            shell::dispatch(if options.open_panel {
                Action::OpenPanel
            } else {
                Action::Recheck
            });
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|_, event| {
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Options, String> {
        parse_options(arguments.iter().map(|argument| argument.to_string()))
    }

    #[test]
    fn toast_activation_arguments_open_the_panel_instead_of_failing() {
        for argument in ["-ToastActivated", "-toastactivated", "-Embedding"] {
            assert!(parsed(&[argument]).unwrap().open_panel, "{argument}");
        }
    }

    #[test]
    fn known_options_still_parse_and_unknown_ones_still_fail() {
        let options = parsed(&["--tray-only", "--background"]).unwrap();
        assert!(options.tray_only && !options.open_panel && !options.list_displays);
        assert!(parsed(&["--list-displays"]).unwrap().list_displays);
        assert!(
            parsed(&["--wat"])
                .unwrap_err()
                .contains("unknown option --wat")
        );
    }
}
