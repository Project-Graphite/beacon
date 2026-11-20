#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod feed;
mod geometry;
mod model;
mod panel;
mod platform;
mod positions;
mod raster;
mod shell;
mod toasts;
mod tray;

use feed::Feed;
use shell::{Action, Shell};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

struct Options {
    tray_only: bool,
    open_panel: bool,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        tray_only: false,
        open_panel: false,
    };
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--tray-only" => options.tray_only = true,
            "--open-panel" => options.open_panel = true,
            other => {
                return Err(format!(
                    "unknown option {other}; expected --tray-only or --open-panel"
                ));
            }
        }
    }
    Ok(options)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options()?;
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
            let hotkey = positions::hotkey(&feed.home);
            if let Err(error) =
                handle
                    .global_shortcut()
                    .on_shortcut(hotkey.as_str(), |app, _, event| {
                        if event.state() == ShortcutState::Pressed {
                            shell::post(app, Action::TogglePanel);
                        }
                    })
            {
                eprintln!("cannot register {hotkey}: {error}");
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
