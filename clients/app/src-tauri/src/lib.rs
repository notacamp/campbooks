//! Campbooks native shell.
//!
//! The webview never sees an API token. Every API call from the UI goes
//! through the [`api::api_request`] command, which attaches credentials on the
//! Rust side (ADR 0004) and forwards the request to the Campbooks API.
#![forbid(unsafe_code)]

mod api;
mod auth;
mod links;

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
/// it never shows anything but the app: a link that asks for a new window
/// never becomes a webview, and the window never navigates away from the
/// app's own pages. A web or mail link opens in the system's browser or mail
/// app, outside the app and its IPC; anything else opens nowhere
/// ([`links`]).
fn main_window(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .cloned()
    else {
        return Err("tauri.conf.json has no main window".into());
    };
    // The dev server, in a debug build only.
    let dev = app
        .config()
        .build
        .dev_url
        .clone()
        .filter(|_| cfg!(debug_assertions));
    tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?
        .on_new_window(|url, _features| {
            links::open_outside(&url);
            tauri::webview::NewWindowResponse::Deny
        })
        .on_navigation(move |url| {
            if links::inside(url, dev.as_ref()) {
                return true;
            }
            links::open_outside(url);
            false
        })
        .build()?;
    Ok(())
}
