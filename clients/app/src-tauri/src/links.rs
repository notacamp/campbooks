//! Links out of the app (campbooks-cloud#199).
//!
//! A link in a mail opens a new window (`target=_blank`, from the reader's
//! sandboxed frame). The shell never lets that become a webview, and never
//! lets the main webview leave the app's own pages: a remote page in a
//! Campbooks window could pass for the app, or reach the IPC its pages use.
//! A web or mail link opens in the system's browser or mail app instead,
//! and anything else (`javascript:`, `file:`, `data:`, the app's own
//! schemes) opens nowhere.

use url::Url;

/// The `mailto:` fields a mail app is handed; anything else (`attach=`,
/// which some mail apps have honoured) is dropped.
const MAILTO_FIELDS: [&str; 5] = ["to", "cc", "bcc", "subject", "body"];

/// What a link may open outside the app: a web link as it is, a mail link
/// with only [`MAILTO_FIELDS`], else nothing.
pub fn outside(url: &Url) -> Option<String> {
    match url.scheme() {
        "https" | "http" => Some(url.as_str().to_owned()),
        "mailto" => {
            let kept: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(key, _)| MAILTO_FIELDS.contains(&key.to_ascii_lowercase().as_str()))
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            let mut clean = url.clone();
            clean.set_query(None);
            if !kept.is_empty() {
                clean.query_pairs_mut().extend_pairs(kept);
            }
            Some(clean.into())
        }
        _ => None,
    }
}

/// Whether the main webview may load `url` itself: the app's own pages
/// (`tauri://localhost`, or `http(s)://tauri.localhost` on Windows), the mail
/// frame's `about:srcdoc` (some engines ask about frames too), and in a
/// debug build the dev server (`dev`).
pub fn inside(url: &Url, dev: Option<&Url>) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" if url.host_str() == Some("tauri.localhost") => true,
        "about" => matches!(url.path(), "srcdoc" | "blank"),
        _ => dev.is_some_and(|dev| dev.origin() == url.origin()),
    }
}

/// Hand `link` to the system ([`outside`]) off the UI thread (on macOS the
/// call waits for the opener). Nowhere to say it failed: it simply does not
/// open.
pub fn open_outside(url: &Url) {
    if let Some(link) = outside(url) {
        std::thread::spawn(move || {
            let _ = open::that_detached(link);
        });
    }
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
            assert_eq!(outside(&parse(url)).as_deref(), Some(url), "{url}");
        }
    }

    #[test]
    fn a_mail_link_keeps_only_its_mail_fields() {
        let link = outside(&parse(
            "mailto:hello@weekly-ledger.example?subject=Hi&attach=/etc/passwd&body=Thanks",
        ));
        assert_eq!(
            link.as_deref(),
            Some("mailto:hello@weekly-ledger.example?subject=Hi&body=Thanks")
        );
        let bare = outside(&parse(
            "mailto:hello@weekly-ledger.example?attach=~/.ssh/id_ed25519",
        ));
        assert_eq!(bare.as_deref(), Some("mailto:hello@weekly-ledger.example"));
    }

    #[test]
    fn the_main_webview_stays_on_the_app() {
        let dev = parse("http://localhost:5173/");
        for url in [
            "tauri://localhost/index.html",
            "http://tauri.localhost/inbox",
            "https://tauri.localhost/inbox",
            "about:srcdoc",
            "about:blank",
        ] {
            assert!(inside(&parse(url), None), "{url}");
        }
        assert!(inside(&parse("http://localhost:5173/inbox"), Some(&dev)));
        for url in [
            "https://weekly-ledger.example/",
            "http://localhost:5173/inbox",
            "tauri://evil.example/",
            "https://tauri.localhost.evil.example/",
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
        ] {
            assert!(!inside(&parse(url), None), "{url}");
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
    /// nothing beyond the core defaults and the API bridge (links open from
    /// Rust, outside the webview).
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

    /// The directives of `csp`, by name, each as its list of sources.
    fn directives(csp: &str) -> Vec<(String, Vec<String>)> {
        csp.split(';')
            .filter_map(|d| {
                let mut words = d.split_whitespace();
                let name = words.next()?.to_owned();
                Some((name, words.map(str::to_owned).collect()))
            })
            .collect()
    }

    /// The page's policies (production and dev) loosen only what HTML mail
    /// needs (campbooks-cloud#199): inline `style` attributes and `data:`,
    /// `blob:` and `https:` images; never inline scripts, `eval`, wildcards,
    /// plain `http:` sources or other frame sources.
    #[test]
    fn the_page_policy_stays_narrow() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .unwrap_or_else(|e| panic!("tauri.conf.json: {e}"));
        for key in ["csp", "devCsp"] {
            let csp = config["app"]["security"][key].as_str().unwrap_or_default();
            let all = directives(csp);
            let get = |name: &str| {
                all.iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, sources)| sources.join(" "))
                    .unwrap_or_default()
            };
            assert_eq!(get("default-src"), "'self'", "{key}");
            assert_eq!(get("script-src"), "'self'", "{key}");
            assert_eq!(get("style-src-attr"), "'unsafe-inline'", "{key}");
            assert_eq!(get("img-src"), "'self' data: blob: https:", "{key}");
            // Only the dev server's own style injection, in dev.
            let style = if key == "csp" {
                "'self'"
            } else {
                "'self' 'unsafe-inline'"
            };
            assert_eq!(get("style-src"), style, "{key}");
            for frames in ["frame-src", "child-src"] {
                assert_eq!(get(frames), "", "{key}: frames stay under default-src");
            }
            for (name, value) in [
                ("object-src", "'none'"),
                ("base-uri", "'none'"),
                ("form-action", "'none'"),
                ("frame-ancestors", "'none'"),
            ] {
                assert_eq!(get(name), value, "{key}: {name}");
            }
            for (name, sources) in &all {
                for source in sources {
                    assert!(
                        !matches!(source.as_str(), "http:" | "*" | "'unsafe-eval'"),
                        "{key}: {name} {source}"
                    );
                    assert!(
                        source != "'unsafe-inline'"
                            || name == "style-src-attr"
                            || (key == "devCsp" && name == "style-src"),
                        "{key}: {name} {source}"
                    );
                }
            }
        }
    }
}
