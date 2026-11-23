#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

struct Options {
    tray_only: bool,
    open_panel: bool,
    list_displays: bool,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        tray_only: false,
        open_panel: false,
        list_displays: false,
    };
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--tray-only" => options.tray_only = true,
            "--open-panel" => options.open_panel = true,
            "--list-displays" => options.list_displays = true,
            BACKGROUND_ARG => {}
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
            settings::get_login_start,
            settings::set_login_start,
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
            let tray = tray::build(&handle, feed.home.clone(), pill_shown)?;
            platform::pill::create(feed.home.clone(), pill_shown, |event| {
                shell::dispatch(Action::Pill(event))
            })?;
            shell::install(Shell::new(handle.clone(), feed.clone(), tray));
            if let Err(error) = settings::register_hotkey(&handle, &positions::hotkey(&feed.home)) {
                eprintln!("{error}");
            }
            let installed = settings::is_installed(&std::env::current_exe()?);
            if installed
                && let Err(error) =
                    toasts::register_identity(&feed.home, &handle.config().identifier)
            {
                eprintln!("cannot register the notification identity: {error}");
            }
            if let Err(error) = settings::apply_start_mode(
                &feed.home,
                positions::ui_settings(&feed.home).start_mode,
                installed,
            ) {
                eprintln!("cannot apply the start mode: {error}");
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
