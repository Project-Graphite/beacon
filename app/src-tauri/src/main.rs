#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod displays;
mod feed;
mod geometry;
mod jump;
mod model;
mod panel;
mod platform;
mod positions;
mod raster;
mod settings;
mod shell;
mod toasts;
mod tray;

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
            other => {
                return Err(format!(
                    "unknown option {other}; expected --tray-only, --open-panel or --list-displays"
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
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
            shell::post(app, Action::OpenPanel)
        }));
    }
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(feed.clone())
        .invoke_handler(tauri::generate_handler![
            panel::get_snapshot,
            panel::get_detail,
            panel::acknowledge,
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
            settings::get_ui_settings,
            settings::set_ui_settings,
            settings::get_login_start,
            settings::set_login_start,
            settings::open_logs_folder,
            settings::app_info,
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
            if !std::env::current_exe()?
                .to_string_lossy()
                .contains(r"\target\")
                && let Err(error) =
                    toasts::register_identity(&feed.home, &handle.config().identifier)
            {
                eprintln!("cannot register the notification identity: {error}");
            }
            let publisher = handle.clone();
            feed.start(move |snapshot| shell::post(&publisher, Action::Snapshot(snapshot)))?;
            if options.open_panel {
                shell::dispatch(Action::OpenPanel);
            }
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
