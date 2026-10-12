//! Campbooks native shell.
//!
//! The webview never sees an API token. Every API call from the UI goes
//! through the [`api::api_request`] command, which attaches credentials on the
//! Rust side (ADR 0004) and forwards the request to the Campbooks API.
#![forbid(unsafe_code)]

mod api;
mod auth;

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
            .run(tauri::generate_context!())
            .map_err(api::ApiError::from)
    });
    if let Err(error) = result {
        panic!("Campbooks failed to start: {error}");
    }
}
