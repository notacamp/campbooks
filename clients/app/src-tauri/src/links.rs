//! Links out of the app (campbooks-cloud#199).
//!
//! A link in a mail opens a new window (`target=_blank`, from the reader's
//! sandboxed frame). The shell never lets that become a webview: a remote
//! page in a Campbooks window could reach the IPC the app's own pages use.
//! A web or mail link opens in the system's browser or mail app instead,
//! and anything else (`javascript:`, `file:`, `data:`, the app's own
//! schemes) opens nowhere.

use url::Url;

/// What a link that asks for a new window may open outside the app: the
/// URL of a web or mail link, else nothing.
pub fn outside(url: &Url) -> Option<&str> {
    matches!(url.scheme(), "https" | "http" | "mailto").then_some(url.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(url: &str) -> Url {
        Url::parse(url).unwrap_or_else(|e| panic!("{url}: {e}"))
    }

    #[test]
    fn web_and_mail_links_open_outside() {
        for url in [
            "https://weekly-ledger.example/articles/tax-dates",
            "http://plain.example/page",
            "mailto:hello@weekly-ledger.example",
        ] {
            assert_eq!(outside(&parse(url)), Some(parse(url).as_str()), "{url}");
        }
    }

    #[test]
    fn nothing_else_opens_anywhere() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<script>alert(1)</script>",
            "blob:https://weekly-ledger.example/0192f1a0",
            "about:blank",
            "tauri://localhost/index.html",
            "ipc://localhost/api_request",
            "http+unix://socket/",
            "ftp://files.example/report.pdf",
        ] {
            assert_eq!(outside(&parse(url)), None, "{url}");
        }
    }

    /// The webview's grants: the main window only, no remote URL, and
    /// nothing beyond the core defaults and the API bridge (the opener is
    /// used from Rust only).
    #[test]
    fn no_remote_page_is_granted_ipc() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .unwrap_or_else(|e| panic!("capabilities/default.json: {e}"));
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert!(capability.get("remote").is_none(), "{capability}");
        assert!(capability.get("webviews").is_none(), "{capability}");
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["core:default", "allow-api-request"])
        );
        let others = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities"))
            .unwrap_or_else(|e| panic!("capabilities/: {e}"))
            .count();
        assert_eq!(others, 1, "one capability file only");
    }

    /// The page's policy loosens only what HTML mail needs (campbooks-cloud
    /// #199): inline `style` attributes and `data:`, `blob:` and `https:`
    /// images; never inline scripts or style sheets, never plain `http:`.
    #[test]
    fn the_page_policy_stays_narrow() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .unwrap_or_else(|e| panic!("tauri.conf.json: {e}"));
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .unwrap_or_default();
        let directive = |name: &str| {
            csp.split(';')
                .map(str::trim)
                .find(|d| d.split_whitespace().next() == Some(name))
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(directive("default-src"), "default-src 'self'");
        assert_eq!(directive("script-src"), "script-src 'self'");
        assert_eq!(directive("style-src"), "style-src 'self'");
        assert_eq!(
            directive("style-src-attr"),
            "style-src-attr 'unsafe-inline'"
        );
        assert_eq!(directive("img-src"), "img-src 'self' data: blob: https:");
        assert_eq!(directive("frame-src"), "", "frames stay under default-src");
        for kept in [
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ] {
            assert!(csp.contains(kept), "{kept} in {csp}");
        }
        assert!(!csp.contains("http: "), "{csp}");
        assert!(!csp.ends_with("http:"), "{csp}");
    }
}
