//! Campbooks native shell.
//!
//! The webview never sees an API token. Every API call from the UI goes
//! through the [`api::api_request`] command, which attaches credentials on the
//! Rust side (ADR 0004) and forwards the request to the Campbooks API.
#![forbid(unsafe_code)]

mod api;
mod auth;
mod links;

use tauri_plugin_opener::OpenerExt as _;

/// Starts the app.
///
/// # Panics
///
/// If `CAMPBOOKS_API_URL` is not a valid URL or the webview runtime fails to
/// start; there is no window yet to report either to.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = api::ApiForwarder::from_env().and_then(|forwarder| {
        tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .manage(forwarder)
            .invoke_handler(tauri::generate_handler![api::api_request])
            .setup(main_window)
            .run(tauri::generate_context!())
            .map_err(api::ApiError::from)
    });
    if let Err(error) = result {
        panic!("Campbooks failed to start: {error}");
    }
}

/// The main window, built from its configuration here (`create: false`) so
/// a link that asks for a new window never becomes a webview: a web or mail
/// link opens in the system's browser or mail app, outside the app and its
/// IPC, and anything else opens nowhere ([`links::outside`]).
fn main_window(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = app.config().app.windows.first().cloned() else {
        return Err("tauri.conf.json has no window".into());
    };
    let handle = app.handle().clone();
    tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?
        .on_new_window(move |url, _features| {
            if let Some(link) = links::outside(&url) {
                // Nowhere to say it failed: the link simply does not open.
                let _ = handle.opener().open_url(link, None::<&str>);
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}
