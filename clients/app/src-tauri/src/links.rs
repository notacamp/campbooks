//! Links out of the app (campbooks-cloud#199).
//!
//! A link in a mail opens a new window (`target=_blank`, from the reader's
//! sandboxed frame). The shell never lets that become a webview, and never
//! lets the main webview leave the app's own pages: a remote page in a
//! Campbooks window could pass for the app, or reach the IPC its pages use.
//! A web or mail link opens in the system's browser or mail app instead,
//! and anything else (`javascript:`, `file:`, `data:`, the app's own
//! schemes) opens nowhere.

use std::fmt::Write as _;

use url::Url;

/// The `mailto:` fields a mail app is handed; anything else (`attach=`,
/// which some mail apps have honoured) is dropped.
const MAILTO_FIELDS: [&str; 5] = ["to", "cc", "bcc", "subject", "body"];

/// What a link may open outside the app: a web link as it is, a mail link
/// with only [`MAILTO_FIELDS`] and no fragment, else nothing.
///
/// A mail link's kept values are decoded and encoded again with everything
/// but RFC 3986's unreserved characters as `%XX` ([`strict`]): a space goes
/// out as `%20` (never `+`, which RFC 6068 reads as a plus), a plus as
/// `%2B`, a line break as `%0D%0A`, and a `;`, `=`, `/` or `#` inside a
/// value as itself encoded (campbooks-cloud#243). The fragment goes: some
/// mail apps split on `&` without minding `#` (CVE-2020-11880). The address
/// part and `to`, `cc` and `bcc` must be plain addresses, or nothing opens;
/// a subject or body holding `&` or `?` once decoded is dropped, since a
/// mail app that decodes before it splits would read another field there
/// (campbooks-cloud#247).
pub fn outside(url: &Url) -> Option<String> {
    match url.scheme() {
        "https" | "http" => Some(url.as_str().to_owned()),
        "mailto" => {
            // `mailto:` then its addresses, before `?`: never an authority
            // (`mailto://user:password@host`, whose parts would reach the
            // mail app unchecked), and plain addresses only, or it opens
            // nowhere (campbooks-cloud#247).
            if url.path().starts_with('/')
                || url.has_host()
                || !url.username().is_empty()
                || url.password().is_some()
                || !addresses_only(url.path())
            {
                return None;
            }
            let mut kept: Vec<String> = Vec::new();
            for field in url.query().unwrap_or_default().split('&') {
                let (key, value) = field.split_once('=').unwrap_or((field, ""));
                let name = key.to_ascii_lowercase();
                if !MAILTO_FIELDS.contains(&name.as_str()) {
                    continue;
                }
                match name.as_str() {
                    // More addresses: held to the same rule, or nothing opens.
                    "to" | "cc" | "bcc" if !addresses_only(value) => return None,
                    // A mail app that decodes before it splits would read a
                    // `&` or `?` in the subject or body as the start of
                    // another field (`&attach=…`): such a field is dropped.
                    "subject" | "body"
                        if decode(value).iter().any(|b| matches!(b, b'&' | b'?')) =>
                    {
                        continue;
                    }
                    _ => {}
                }
                kept.push(format!(
                    "{}={}",
                    strict(&decode(key)),
                    strict(&decode(value))
                ));
            }
            let mut clean = url.clone();
            clean.set_query((!kept.is_empty()).then(|| kept.join("&")).as_deref());
            clean.set_fragment(None);
            Some(clean.into())
        }
        _ => None,
    }
}

/// Whether a mail link's address part (`path`, still percent-encoded) is
/// nothing but addresses: none, or comma-separated `local@domain`s with no
/// control character, blank, or delimiter a mail app could read as another
/// field or header (CR, LF, `&`, `;`, `?`, `#`, `<`, `>`, `"`, `\`), even
/// encoded. A mail app that decodes the whole link before splitting it then
/// finds no field hidden in it.
fn addresses_only(path: &str) -> bool {
    let decoded = decode(path);
    if decoded.is_empty() {
        return true;
    }
    // Never a path a mail app could take for an authority or a file.
    if decoded.first() == Some(&b'/') {
        return false;
    }
    let refused = |b: &u8| {
        b.is_ascii_control()
            || b.is_ascii_whitespace()
            || matches!(
                b,
                b'&' | b';' | b'?' | b'#' | b'<' | b'>' | b'"' | b'\\' | b'%'
            )
    };
    if decoded.iter().any(refused) {
        return false;
    }
    // Other characters may be any Unicode (an internationalised address or
    // domain), but valid UTF-8, and never a control or an invisible format
    // character (a bidi override, a zero-width space) that could make an
    // address read as another.
    let Ok(text) = std::str::from_utf8(&decoded) else {
        return false;
    };
    if text
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || is_invisible(c))
    {
        return false;
    }
    decoded.split(|b| *b == b',').all(|address| {
        let mut parts = address.split(|b| *b == b'@');
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(local), Some(domain), None) if !local.is_empty() && !domain.is_empty()
        )
    })
}

/// Whether `c` is invisible: a Unicode format character (general category
/// Cf, which changes how the text around it shows) or another
/// Default_Ignorable_Code_Point (fillers, variation selectors).
fn is_invisible(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x034F
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFFA0
            | 0xE0100..=0xE01EF
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// `text` with its `%XX` escapes decoded, as bytes; a `%` not followed by
/// two hex digits stays as it is.
fn decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push(u8::try_from(high * 16 + low).unwrap_or_default());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// `bytes` with everything but RFC 3986's unreserved characters (letters,
/// digits, `-`, `.`, `_`, `~`) written as `%XX`.
fn strict(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// What the build may treat as the app's own pages besides
/// `tauri://localhost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    /// `http(s)://tauri.localhost` is the app: on Windows and Android, where
    /// the webview serves the app from it. Elsewhere `*.localhost` is the
    /// machine itself, where any local process could answer and pass for
    /// the app (campbooks-cloud#243).
    pub localhost_app: bool,
    /// A debug build, which may load the dev server.
    pub debug: bool,
}

impl Build {
    /// This build.
    pub const CURRENT: Self = Self {
        localhost_app: cfg!(any(windows, target_os = "android")),
        debug: cfg!(debug_assertions),
    };
}

/// Whether the main webview may load `url` itself ([`inside_for`] this
/// build).
pub fn inside(url: &Url, dev: Option<&Url>) -> bool {
    inside_for(url, dev, Build::CURRENT)
}

/// Whether the main webview of `build` may load `url` itself: the app's own
/// pages (`tauri://localhost`, and `http(s)://tauri.localhost` where the
/// build serves the app from it), the mail frame's `about:srcdoc` (some
/// engines ask about frames too), and in a debug build only the dev server
/// (`dev`).
pub fn inside_for(url: &Url, dev: Option<&Url>, build: Build) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" if build.localhost_app && url.host_str() == Some("tauri.localhost") => {
            true
        }
        "about" => matches!(url.path(), "srcdoc" | "blank"),
        _ => build.debug && dev.is_some_and(|dev| dev.origin() == url.origin()),
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
    fn a_mail_link_keeps_its_spaces_and_line_breaks_encoded() {
        // A space stays `%20` (RFC 6068 reads `+` as a plus), a plus is
        // `%2B`, a line break `%0D%0A`, a comma `%2C`; `attach=` still goes.
        let link = outside(&parse(
            "mailto:hello@weekly-ledger.example?Subject=Tax%20dates%2Bfees\
             &body=Hi%20Rita,%0D%0AThanks&attach=/etc/passwd&x-mailer=a+b",
        ));
        assert_eq!(
            link.as_deref(),
            Some(
                "mailto:hello@weekly-ledger.example?Subject=Tax%20dates%2Bfees\
                 &body=Hi%20Rita%2C%0D%0AThanks"
            )
        );
        let plus = outside(&parse("mailto:hello@weekly-ledger.example?subject=a+b"));
        assert_eq!(
            plus.as_deref(),
            Some("mailto:hello@weekly-ledger.example?subject=a%2Bb")
        );
        let spaced = outside(&parse(
            "mailto:hello@weekly-ledger.example?subject=Tax dates",
        ));
        assert_eq!(
            spaced.as_deref(),
            Some("mailto:hello@weekly-ledger.example?subject=Tax%20dates")
        );
        let link = link.unwrap_or_default();
        assert!(!link.contains('+') && !link.contains('\r') && !link.contains('\n'));
    }

    #[test]
    fn no_field_hides_in_a_value_or_the_fragment() {
        for (link, kept) in [
            // A fragment some mail apps split on `&` anyway (CVE-2020-11880).
            (
                "mailto:x@weekly-ledger.example?subject=hi#&attach=/home/u/.ssh/id_rsa",
                "mailto:x@weekly-ledger.example?subject=hi",
            ),
            // Another separator inside a value.
            (
                "mailto:x@weekly-ledger.example?subject=x;attach=/etc/passwd",
                "mailto:x@weekly-ledger.example?subject=x%3Battach%3D%2Fetc%2Fpasswd",
            ),
            (
                "mailto:x@weekly-ledger.example?subject=x?attach=/etc/passwd",
                "mailto:x@weekly-ledger.example",
            ),
            // A `?` or an encoded `&` in a subject or body: the field goes.
            (
                "mailto:x@weekly-ledger.example?body=a%26attach%3D%2Fetc%2Fpasswd",
                "mailto:x@weekly-ledger.example",
            ),
        ] {
            assert_eq!(outside(&parse(link)).as_deref(), Some(kept), "{link}");
        }
    }

    #[test]
    fn a_mail_link_whose_addresses_hide_more_opens_nowhere() {
        for link in [
            "mailto:a@b.example%0D%0Abcc:evil@x.example",
            "mailto:a@b.example%0Abcc:evil@x.example",
            "mailto:a@b.example%0d%0abcc:evil@x.example",
            "mailto:a@b.example&bcc=evil@x.example",
            "mailto:a@b.example;evil@x.example",
            "mailto:a@b.example%3Fattach=/etc/passwd",
            "mailto:a@b.example%26attach=/etc/passwd",
            "mailto:a@b.example%2526attach=/etc/passwd",
            "mailto:a%20b@x.example",
            "mailto:not-an-address",
            "mailto:a@b@c.example",
            "mailto:a@b.example,,c@d.example",
            "mailto:%3Cevil@x.example%3E",
            // An authority: its user, password and host would pass unchecked.
            "mailto://x%0D%0Abcc:evil@b.example",
            "mailto://x:y%0D%0Abcc%3Aevil@host.example?subject=a",
            "mailto://rita@silva-contas.example",
            "mailto:///rita@silva-contas.example",
            // Invisible or control characters, and bytes that are not UTF-8.
            "mailto:rita@silva%E2%80%AEcontas.example",
            "mailto:rita%E2%80%8B@silva-contas.example",
            "mailto:rita@silva-contas.example%C2%85bcc",
            "mailto:rita@silva-contas%FF.example",
            "mailto:rita@silva-contas.example%E2%80",
            // A truncated escape.
            "mailto:rita@silva-contas.example%2",
            // A path a mail app could take for an authority or a file.
            "mailto:/x@y.example",
            "mailto:%2F%2Fx@y.example",
            "MAILTO://x@y.example",
            // A backslash, raw or encoded.
            "mailto:a\\b@y.example",
            "mailto:a%5Cb@y.example",
            // Unicode blanks and other invisible characters.
            "mailto:rita@silva-contas.example%C2%A0",
            "mailto:rita%E3%80%80@silva-contas.example",
            "mailto:rita%CD%8F@silva-contas.example",
            "mailto:rita@silva-contas.example%EF%B8%8F",
            // More addresses in a field, hiding a header.
            "mailto:?to=a@b.example%0D%0Abcc:evil@x.example",
            "mailto:rita@silva-contas.example?cc=x@y.example%26attach%3D/etc/passwd",
            "mailto:rita@silva-contas.example?bcc=not-an-address",
        ] {
            assert_eq!(outside(&parse(link)), None, "{link}");
        }
        for link in [
            "mailto:hello@weekly-ledger.example",
            "mailto:a@x.example,b@y.example",
            "mailto:?subject=Hi",
            "mailto:rita.gomes+tax@silva-contas.example",
            // Internationalised: any other Unicode still opens.
            "mailto:ana@ex%C3%A4mple.example",
            "mailto:ana@exämple.example",
        ] {
            assert!(outside(&parse(link)).is_some(), "{link}");
        }
    }

    #[test]
    fn a_subject_or_body_that_could_split_into_another_field_is_dropped() {
        // A mail app that decodes before it splits would read
        // `&attach=…` out of these.
        let link = outside(&parse(
            "mailto:rita@silva-contas.example?subject=Hi\
             &body=x%26attach%3D/etc/passwd&cc=joao@silva-contas.example",
        ));
        assert_eq!(
            link.as_deref(),
            Some("mailto:rita@silva-contas.example?subject=Hi&cc=joao%40silva-contas.example")
        );
        let asked = outside(&parse(
            "mailto:rita@silva-contas.example?subject=Free%3F&body=Thanks",
        ));
        assert_eq!(
            asked.as_deref(),
            Some("mailto:rita@silva-contas.example?body=Thanks")
        );
        // More addresses that are addresses still go.
        let copied = outside(&parse(
            "mailto:?to=rita@silva-contas.example,joao@silva-contas.example&subject=Hi",
        ));
        assert_eq!(
            copied.as_deref(),
            Some("mailto:?to=rita%40silva-contas.example%2Cjoao%40silva-contas.example&subject=Hi")
        );
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

    /// Windows and Android (the app on `tauri.localhost`), the other
    /// platforms, each as a debug and a release build.
    const WINDOWS: Build = Build {
        localhost_app: true,
        debug: false,
    };
    const MAC_OR_LINUX: Build = Build {
        localhost_app: false,
        debug: false,
    };

    #[test]
    fn tauri_localhost_is_the_app_only_where_the_app_is_served_from_it() {
        for url in [
            "http://tauri.localhost/inbox",
            "https://tauri.localhost/inbox",
        ] {
            assert!(inside_for(&parse(url), None, WINDOWS), "{url}");
            // On macOS and Linux it is the machine: a local process could
            // answer it.
            assert!(!inside_for(&parse(url), None, MAC_OR_LINUX), "{url}");
        }
        for build in [WINDOWS, MAC_OR_LINUX] {
            assert!(inside_for(
                &parse("tauri://localhost/index.html"),
                None,
                build
            ));
        }
        assert_eq!(
            Build::CURRENT.localhost_app,
            cfg!(any(windows, target_os = "android"))
        );
    }

    #[test]
    fn only_a_debug_build_loads_the_dev_server() {
        let dev = parse("http://localhost:5173/");
        let page = parse("http://localhost:5173/inbox");
        for build in [WINDOWS, MAC_OR_LINUX] {
            assert!(!inside_for(&page, Some(&dev), build), "{build:?}");
            let debug = Build {
                debug: true,
                ..build
            };
            assert!(inside_for(&page, Some(&dev), debug), "{debug:?}");
            assert!(!inside_for(
                &parse("http://localhost:5174/"),
                Some(&dev),
                debug
            ));
        }
        assert_eq!(Build::CURRENT.debug, cfg!(debug_assertions));
    }

    #[test]
    fn the_main_webview_stays_on_the_app() {
        for url in [
            "tauri://localhost/index.html",
            "http://tauri.localhost/inbox",
            "https://tauri.localhost/inbox",
            "about:srcdoc",
            "about:blank",
        ] {
            assert!(inside_for(&parse(url), None, WINDOWS), "{url}");
        }
        for url in [
            "https://weekly-ledger.example/",
            "http://localhost:5173/inbox",
            "tauri://evil.example/",
            "https://tauri.localhost.evil.example/",
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
        ] {
            for build in [WINDOWS, MAC_OR_LINUX] {
                assert!(!inside_for(&parse(url), None, build), "{url}");
            }
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
