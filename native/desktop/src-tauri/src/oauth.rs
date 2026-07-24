//! Navigation + OAuth handoff — the desktop port of the mobile
//! `OAuthRouteDecisionHandler` (`native/ios/Campbooks/OAuthRouteDecisionHandler.swift`).
//!
//! Two halves:
//!   1. [`is_external_link`] — any navigation that leaves the app's own origin
//!      (an external site, or an OAuth provider's authorize page — providers
//!      reject embedded webviews) must open in the **system browser**, never
//!      hijack the wrapper's webview.
//!   2. [`resolve`] — when the server finishes the OAuth dance it redirects to
//!      `campbooks://oauth?…`; we translate that deep link into the in-window URL
//!      to navigate to, mirroring the server's `OauthNativeHandoff` contract.

use tauri::Url;

/// True when navigating to `url` should open the **system browser** rather than
/// stay in the app's webview: any `http(s)` URL whose host differs from the app's
/// own (external sites *and* OAuth provider authorize pages), plus `mailto:` /
/// `tel:` links. Same-origin app pages — and internal schemes like `blob:` /
/// `data:` / `about:` (used for in-app downloads/previews) — stay in the webview.
pub fn is_external_link(url: &Url, app: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => url.host_str() != app.host_str(),
        "mailto" | "tel" => true,
        _ => false,
    }
}

/// Map a `campbooks://oauth?…` deep link to the app URL the window should load,
/// against `base` (the configured app origin). Returns `None` for flows that
/// should leave the user where they are (`status=error`, `add_sign_in`, unknown).
///
///   flow=signin  + token → `<base>/session/native?token=…`  (plants the cookie)
///   flow=connect          → `<base>/email_messages?inbox_settings=accounts`
pub fn resolve(deep_link: &Url, base: &Url) -> Option<Url> {
    if deep_link.scheme() != "campbooks" {
        return None;
    }

    let params: std::collections::HashMap<String, String> =
        deep_link.query_pairs().into_owned().collect();

    match params.get("flow").map(String::as_str) {
        Some("signin") => {
            let token = params.get("token")?;
            let mut dest = base.join("/session/native").ok()?;
            dest.query_pairs_mut().append_pair("token", token);
            Some(dest)
        }
        Some("connect") => {
            let mut dest = base.join("/email_messages").ok()?;
            dest.query_pairs_mut()
                .append_pair("inbox_settings", "accounts");
            Some(dest)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://app.campbooks.not-a-camp.com").unwrap()
    }

    #[test]
    fn opens_external_links() {
        let app = base();
        // OAuth providers + any other external host → system browser
        assert!(is_external_link(&Url::parse("https://accounts.google.com/o/oauth2/auth?x=1").unwrap(), &app));
        assert!(is_external_link(&Url::parse("https://login.microsoftonline.com/common/oauth2/v2.0/authorize").unwrap(), &app));
        assert!(is_external_link(&Url::parse("https://github.com/notacamp/campbooks").unwrap(), &app));
        assert!(is_external_link(&Url::parse("https://not-a-camp.com/pricing").unwrap(), &app));
        assert!(is_external_link(&Url::parse("mailto:hi@example.com").unwrap(), &app));
        // the app's own pages stay in the webview
        assert!(!is_external_link(&Url::parse("https://app.campbooks.not-a-camp.com/email_messages").unwrap(), &app));
        assert!(!is_external_link(&Url::parse("https://app.campbooks.not-a-camp.com/documents?x=1").unwrap(), &app));
        // internal schemes (in-app downloads/previews) stay in the webview
        assert!(!is_external_link(&Url::parse("about:blank").unwrap(), &app));
    }

    #[test]
    fn signin_resolves_to_native_session() {
        let link = Url::parse("campbooks://oauth?flow=signin&token=abc.def").unwrap();
        let dest = resolve(&link, &base()).unwrap();
        assert_eq!(dest.path(), "/session/native");
        assert_eq!(
            dest.query_pairs().find(|(k, _)| k == "token").unwrap().1,
            "abc.def"
        );
    }

    #[test]
    fn connect_resolves_to_accounts() {
        let link = Url::parse("campbooks://oauth?flow=connect&status=success").unwrap();
        let dest = resolve(&link, &base()).unwrap();
        assert_eq!(dest.path(), "/email_messages");
        assert_eq!(
            dest.query_pairs()
                .find(|(k, _)| k == "inbox_settings")
                .unwrap()
                .1,
            "accounts"
        );
    }

    #[test]
    fn error_and_unknown_flows_stay_put() {
        assert!(resolve(
            &Url::parse("campbooks://oauth?flow=signin&status=error").unwrap(),
            &base()
        )
        .is_none());
        assert!(resolve(
            &Url::parse("campbooks://oauth?flow=add_sign_in&status=success").unwrap(),
            &base()
        )
        .is_none());
        assert!(resolve(&Url::parse("https://example.com/?flow=signin&token=x").unwrap(), &base()).is_none());
    }
}
