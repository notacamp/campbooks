fn main() {
    // Declaring the app's commands in an app manifest generates an
    // `allow-<command>` permission per command, so a command is callable only
    // from windows whose capability grants it (see `capabilities/default.json`)
    // instead of from every webview by default.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["api_request"])),
    )
    .unwrap_or_else(|error| panic!("tauri build script failed: {error:#}"));
}
