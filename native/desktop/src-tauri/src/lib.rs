//! Campbooks desktop shell (Tauri v2) — the desktop twin of the Hotwire Native
//! iOS/Android shells in `native/`. A thin native window around the hosted
//! Campbooks web app: it sends the `Hotwire Native` User-Agent (so the server
//! applies the same native treatment), opens OAuth provider pages in the system
//! browser, and catches the `campbooks://oauth` deep-link handoff. The web app
//! is Tauri-unaware — all native logic lives here, Rust-side.

mod config;
mod oauth;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

pub fn run() {
    tauri::Builder::default()
        // Single-instance MUST be registered first. With the `deep-link`
        // feature, a second launch carrying a `campbooks://…` URL (the Windows /
        // Linux deep-link delivery model) is funnelled back into the running
        // instance rather than opening a second window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let base = config::app_url();

            // The single main window. Built here (not in tauri.conf.json) so we
            // can attach the UA override and the navigation interceptor.
            let nav_handle = app.handle().clone();
            let nav_base = base.clone();
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(base.clone()))
                .title("Campbooks")
                .inner_size(1200.0, 820.0)
                .min_inner_size(720.0, 600.0)
                .user_agent(config::USER_AGENT)
                .on_navigation(move |url| {
                    // Any off-origin link (external sites + OAuth provider pages)
                    // opens in the system browser instead of hijacking the webview.
                    // For OAuth the provider then redirects to `campbooks://oauth?…`,
                    // caught by the deep-link handler below.
                    if oauth::is_external_link(url, &nav_base) {
                        let _ = nav_handle.opener().open_url(url.as_str(), None::<&str>);
                        return false;
                    }
                    true
                })
                .build()?;

            // Deep-link handler: translate `campbooks://oauth?…` into an in-window
            // navigation per the mobile handoff contract.
            let ev_window = window.clone();
            let ev_base = base.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    if let Some(dest) = oauth::resolve(&url, &ev_base) {
                        let _ = ev_window.navigate(dest);
                    }
                }
            });

            // Cold start: the app may have been launched *by* the deep link.
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    if let Some(dest) = oauth::resolve(&url, &base) {
                        let _ = window.navigate(dest);
                    }
                }
            }

            // In dev there's no installer to register the scheme on Windows /
            // Linux, so do it at runtime. (On macOS the scheme is baked into
            // Info.plist at build time.)
            #[cfg(any(windows, target_os = "linux"))]
            {
                let _ = app.deep_link().register_all();
            }

            // Check for a newer signed release in the background and prompt.
            check_for_updates(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Campbooks desktop app");
}

/// Background update check: if a newer signed release exists at the configured
/// endpoint, prompt natively and — on confirm — download, install, and relaunch.
/// Runs Rust-side (the webview is the remote app, which knows nothing of Tauri).
fn check_for_updates(handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = handle.updater() else {
            return;
        };
        if let Ok(Some(update)) = updater.check().await {
            let msg = format!(
                "Campbooks {} is available. Install it now? The app will restart.",
                update.version
            );
            let restart_handle = handle.clone();
            handle
                .dialog()
                .message(msg)
                .title("Update available")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Install".to_string(),
                    "Later".to_string(),
                ))
                .show(move |install| {
                    if install {
                        tauri::async_runtime::spawn(async move {
                            if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
                                restart_handle.restart();
                            }
                        });
                    }
                });
        }
    });
}
