//! Access-token source for API calls made by the native shell.
//!
//! TODO(ADR 0004): tokens live in the OS keychain via the `keyring` crate
//! (Keychain on macOS/iOS, Credential Manager on Windows, Secret Service on
//! Linux, Keystore on Android), obtained with OAuth 2.1 authorization code +
//! PKCE in the system browser and refreshed here on the Rust side. Until that
//! lands there is no token: requests go out unauthenticated, which is enough
//! for the public `system` endpoints. The token must never be returned to the
//! webview.

/// The bearer token to attach to an API request, if the user is signed in.
pub fn access_token() -> Option<String> {
    None
}
