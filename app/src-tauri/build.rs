fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_snapshot",
            "get_detail",
            "acknowledge",
            "decide",
            "snooze",
            "unsnooze",
            "refresh_usage",
            "close_panel",
            "set_panel_height",
            "jump",
        ]),
    ))
    .expect("the Tauri build script failed");
}
