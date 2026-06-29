//! App-wide configuration for the desktop shell — the desktop counterpart of
//! `native/ios/Campbooks/Config.swift` and `native/android/.../Config.kt`.

use tauri::Url;

/// Where the window loads from. Debug builds point at a local Rails server
/// (mirrors the iOS Simulator convention); release builds always point at
/// production.
#[cfg(debug_assertions)]
const APP_URL: &str = "http://localhost:3000";
#[cfg(not(debug_assertions))]
const APP_URL: &str = "https://app.campbooks.not-a-camp.com";

/// Parsed start URL for the main window.
pub fn app_url() -> Url {
    Url::parse(APP_URL).expect("APP_URL is a valid absolute URL")
}

/// The webview User-Agent.
///
/// Two requirements pull on this string:
///   1. It must contain `Hotwire Native` so turbo-rails' `hotwire_native_app?`
///      fires server-side — that's what gives the desktop app the same native
///      treatment (hidden web topbar, direct provider sign-in links, and the
///      `campbooks://oauth` deep-link handoff) as the mobile shells.
///   2. It must still read as a *modern* browser, because the Rails app guards
///      every request with `allow_browser versions: :modern`. A bare token would
///      be rejected with 406, so we present a real, recent browser UA per OS and
///      append the Hotwire Native marker. (The `:modern` bar is a static set of
///      Rails feature-gates, so these pinned versions stay "modern".)
#[cfg(target_os = "macos")]
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15 Campbooks-Desktop Hotwire Native";

#[cfg(target_os = "windows")]
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0 Campbooks-Desktop Hotwire Native";

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15 Campbooks-Desktop Hotwire Native";
