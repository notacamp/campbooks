//! Sanitising untrusted email HTML before it leaves the server.
//!
//! The client shows the result only in a sandboxed frame with its own CSP
//! (`lib/ui/safe-html`); this is the first of the two layers, so the frame
//! never receives markup that could run, load or submit anything unasked:
//!
//! - **style sheets** are inlined into `style` attributes first
//!   ([`styles`]): mail is written for clients that drop `<style>`, and the
//!   frame's inherited policy blocks it too. Their `@media` rules come back
//!   separately, filtered ([`css::media_rules`]), for the client to apply to
//!   the frame's document; the `<body>`'s own look moves to a wrapper `div`;
//! - **ammonia** keeps a fixed allow-list of tags and attributes, drops
//!   `<script>`, `<style>` and `<title>` (with their content), forms,
//!   frames, event handlers and comments, allows only `http`, `https`,
//!   `mailto` and `tel` links, and makes every link open in a new tab with
//!   `rel="noopener noreferrer"`;
//! - **style attributes** keep only declarations a real CSS parser shows to
//!   load and run nothing ([`css`]);
//! - **images**: remote images lose their `src` unless the member asked to
//!   see them (and are counted, so the reader can offer "Show images"), and
//!   tracking pixels (an image of at most 2×2, by attribute or style, or
//!   one hidden by its style) are removed either way. An inline image
//!   (`cid:`) shows the message's own part as a `data:` URL the caller
//!   built; every image left without a source shows [`PLACEHOLDER`], so the
//!   reader never shows the browser's broken-image icon. Those two are the
//!   only `data:` sources that leave.

pub mod css;
pub mod styles;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write as _,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use ammonia::{Builder, UrlRelative};
use campbooks_kernel::mail::escape_html;
use regex::Regex;

use super::plain_text::{self, Quotes};

/// Sanitised HTML and what was held back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    /// Safe to show in the sandboxed frame.
    pub html: String,
    /// Remote images in the message (blocked unless shown).
    pub remote_images: u32,
    /// The message's `@media` rules, filtered, for the client to apply to
    /// the frame's document only.
    pub css: Option<String>,
}

/// What an image shows when its source is held back or unusable: a light
/// panel with a small picture glyph, centred, in the design system's
/// neutrals (mail is always on a white page). No intrinsic size, so it
/// takes the image's declared size; it loads nothing.
pub const PLACEHOLDER: &str = "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%3E%3Crect%20width='100%25'%20height='100%25'%20rx='6'%20fill='%23f1f0ee'/%3E%3Csvg%20x='50%25'%20y='50%25'%20overflow='visible'%3E%3Cg%20transform='translate(-10%20-10)'%20fill='none'%20stroke='%238f8d89'%20stroke-width='1.5'%20stroke-linecap='round'%20stroke-linejoin='round'%3E%3Crect%20x='2'%20y='3'%20width='16'%20height='14'%20rx='2'/%3E%3Ccircle%20cx='7'%20cy='8'%20r='1.5'/%3E%3Cpath%20d='m2.5%2015%204.5-4.5%203.5%203.5%202-2%204.5%204.5'/%3E%3C/g%3E%3C/svg%3E%3C/svg%3E";

/// The size a placeholder takes when its image declares none, in px.
const PLACEHOLDER_SIZE: u32 = 48;

/// The message's inline images: content id (lower-case, no angle brackets)
/// to the `data:` URL of its part, built by the caller from stored bytes of
/// a raster type.
pub type InlineImages = HashMap<String, String>;

/// Tags email layouts use, on top of ammonia's defaults.
const EXTRA_TAGS: [&str; 2] = ["center", "font"];

/// Presentational attributes allowed on any tag. `class` stays so the
/// message's `@media` rules can match; nothing else reads it.
const GENERIC_ATTRIBUTES: [&str; 9] = [
    "style", "align", "valign", "width", "height", "bgcolor", "dir", "border", "class",
];

/// What the second pass does with every image source that survived the
/// first: `data:` and relative URLs are already gone, so each one left is a
/// remote image, whatever its case or surrounding whitespace.
struct Images {
    /// Keep the sources (the member chose "Show images").
    show: bool,
    /// How many remote sources there were.
    seen: Arc<AtomicU32>,
    /// The inline images `cid:` sources may name.
    inline: Arc<InlineImages>,
}

impl Images {
    /// The source to keep for `value`, if any.
    fn source<'a>(&self, value: &'a str) -> Option<Cow<'a, str>> {
        if let Some(id) = content_id(value) {
            // Not remote: the message's own part, or nothing.
            return self.inline.get(&id).cloned().map(Cow::Owned);
        }
        self.seen.fetch_add(1, Ordering::Relaxed);
        self.show.then_some(Cow::Borrowed(value))
    }
}

/// The content id a `cid:` URL names (RFC 2392: percent-encoded), lower-case.
fn content_id(url: &str) -> Option<String> {
    let url = url.trim_matches(|c: char| c.is_ascii_whitespace());
    let rest = url
        .get(..4)?
        .eq_ignore_ascii_case("cid:")
        .then(|| &url[4..])?;
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                i += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    let id = String::from_utf8(decoded).ok()?;
    let id = id.trim().trim_start_matches('<').trim_end_matches('>');
    (!id.is_empty()).then(|| id.to_lowercase())
}

/// Sanitise `html`. Remote images keep their `src` only when
/// `show_remote_images` is set; `cid:` images show their part from `inline`
/// either way.
///
/// Six steps: the HTML is measured first ([`styles::measure`]: a parse as a
/// document and one as a fragment, as the inliner and ammonia parse it, each
/// counting the tree builder's work and stopped at a fixed budget), and
/// HTML past the budget is shown as its text alone; then the style sheets
/// are inlined ([`styles::inline`]); ammonia
/// cleans (allow-lists, schemes, links, style declarations) and the body's
/// look moves to a wrapper; a best-effort pass drops tracking pixels from
/// its canonical output; ammonia runs again with the image policy in its
/// attribute filter, so every `img[src]` the HTML parser sees is counted
/// and, unless shown, removed, and every `cid:` source becomes its part's
/// `data:` URL; then every image left without a source gets the
/// placeholder. A last check strips every image if any source other than
/// those survived a blocked render.
pub fn sanitize(html: &str, show_remote_images: bool, inline: &InlineImages) -> Sanitized {
    let Some(tree) = styles::measure(html) else {
        return as_text(html);
    };
    let inlined = styles::inline(html, &tree);
    let cleaned = builder(None).clean(&inlined.html).to_string();
    let wrapped = wrap_body(&cleaned, &inlined.body);
    let without_pixels = drop_tracking_pixels(&wrapped);
    let seen = Arc::new(AtomicU32::new(0));
    let policy = Images {
        show: show_remote_images,
        seen: Arc::clone(&seen),
        inline: Arc::new(inline.clone()),
    };
    let filtered = builder(Some(policy)).clean(&without_pixels).to_string();
    let mut html = fill_missing_sources(&filtered);
    if !show_remote_images && !only_own_sources(&html, inline) {
        // Unreachable with the filter above; fail closed if it ever is not.
        let mut strict = builder(None);
        strict.rm_tags(["img"]);
        html = strict.clean(&html).to_string();
    }
    Sanitized {
        html,
        remote_images: seen.load(Ordering::Relaxed),
        css: inlined.media,
    }
}

/// The words of `html` in one `div` that keeps its line breaks, for HTML
/// past the parse budget. Nothing from the message's markup survives, so
/// nothing needs sanitising: the reader still shows the words.
fn as_text(html: &str) -> Sanitized {
    let text = plain_text::text_of(html, Quotes::Keep);
    Sanitized {
        html: format!(
            r#"<div style="white-space: pre-wrap">{}</div>"#,
            escape_html(&text)
        ),
        remote_images: 0,
        css: None,
    }
}

/// `html` inside a `div` carrying the message body's `style` and `bgcolor`
/// (ammonia drops `<body>`), or `html` itself when it had neither. The
/// values are escaped here and filtered by the next ammonia pass.
fn wrap_body(html: &str, body: &[(String, String)]) -> String {
    if body.is_empty() {
        return html.to_owned();
    }
    let mut wrapped = String::from("<div");
    for (name, value) in body {
        let _ = write!(wrapped, r#" {name}="{}""#, escape_attribute(value));
    }
    let _ = write!(wrapped, ">{html}</div>");
    wrapped
}

/// An attribute value with every character that could end it or start
/// markup escaped.
fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn builder(images: Option<Images>) -> Builder<'static> {
    let mut builder = Builder::default();
    builder
        .add_tags(EXTRA_TAGS)
        // A document's `<title>` is not part of what the message says.
        .add_clean_content_tags(["title"])
        .add_generic_attributes(GENERIC_ATTRIBUTES)
        .add_tag_attributes("img", ["src", "alt", "width", "height"])
        .add_tag_attributes("font", ["color", "face", "size"])
        .add_tag_attributes("td", ["colspan", "rowspan"])
        .add_tag_attributes("th", ["colspan", "rowspan"])
        .add_tag_attributes("table", ["cellpadding", "cellspacing"])
        // `cid` survives only on images (the filter drops it elsewhere).
        .url_schemes(HashSet::from(["http", "https", "mailto", "tel", "cid"]))
        // Mail has no base URL: a relative link or image goes nowhere.
        .url_relative(UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank")
        .attribute_filter(move |element, attribute, value| match (element, attribute) {
            (_, "style") => css::style_attribute(value).map(Cow::Owned),
            ("img", "src") => match &images {
                None => Some(Cow::Borrowed(value)),
                Some(images) => images.source(value),
            },
            (_, "href" | "src" | "cite") if content_id(value).is_some() => None,
            _ => Some(Cow::Borrowed(value)),
        });
    builder
}

/// An `<img>` tag as ammonia serialises it: lower-case attribute names,
/// double-quoted values (a `"` inside a value is always `&quot;`). Used only
/// to find tracking pixels, best effort: blocking does not depend on it.
static IMG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<img((?:\s+[a-z-]+="[^"]*")*)\s*/?>"#).unwrap_or_else(|e| unreachable!("{e}"))
});
static ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-z-]+)="([^"]*)""#).unwrap_or_else(|e| unreachable!("{e}")));
/// Any `src` attribute, however written, with its value: the final check of
/// a blocked render (a false positive only strips the images).
static SRC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bsrc\s*=\s*("[^"]*"|'[^']*'|[^\s>]*)"#)
        .unwrap_or_else(|e| unreachable!("{e}"))
});

/// Whether every source in `html` is the placeholder or one of `inline`'s
/// URLs, written as ammonia writes them.
fn only_own_sources(html: &str, inline: &InlineImages) -> bool {
    SRC.captures_iter(html).all(|source| {
        let quoted = source.get(1).map_or("", |m| m.as_str());
        quoted
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .is_some_and(|value| value == PLACEHOLDER || inline.values().any(|url| url == value))
    })
}

/// Give every image ammonia left without a source the placeholder, and the
/// placeholder's size when the image declares none.
fn fill_missing_sources(html: &str) -> String {
    IMG.replace_all(html, |tag: &regex::Captures<'_>| {
        let raw = tag.get(1).map_or("", |m| m.as_str());
        let attributes: Vec<(&str, &str)> = ATTRIBUTE
            .captures_iter(raw)
            .filter_map(|c| Some((c.get(1)?.as_str(), c.get(2)?.as_str())))
            .collect();
        if attribute(&attributes, "src").is_some() {
            return tag[0].to_owned();
        }
        let size = if attribute(&attributes, "width").is_none()
            && attribute(&attributes, "height").is_none()
        {
            format!(r#" width="{PLACEHOLDER_SIZE}" height="{PLACEHOLDER_SIZE}""#)
        } else {
            String::new()
        };
        format!(r#"<img{raw} src="{PLACEHOLDER}"{size}>"#)
    })
    .into_owned()
}

/// Remove the images ammonia's output shows to be tracking pixels.
fn drop_tracking_pixels(html: &str) -> String {
    IMG.replace_all(html, |tag: &regex::Captures<'_>| {
        let attributes: Vec<(&str, &str)> = ATTRIBUTE
            .captures_iter(tag.get(1).map_or("", |m| m.as_str()))
            .filter_map(|c| Some((c.get(1)?.as_str(), c.get(2)?.as_str())))
            .collect();
        if is_tracking_pixel(&attributes) {
            String::new()
        } else {
            tag[0].to_owned()
        }
    })
    .into_owned()
}

fn attribute<'a>(attributes: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value)| *value)
}

/// An image too small to see, or hidden: there to report that the message
/// was opened. Read from its attributes and its (already filtered) style.
fn is_tracking_pixel(attributes: &[(&str, &str)]) -> bool {
    let style = attribute(attributes, "style")
        .map(css::declarations)
        .unwrap_or_default();
    let declared = |name: &str| {
        style
            .iter()
            .rev()
            .find(|d| d.name == name)
            .map(|d| d.value.to_ascii_lowercase())
    };
    let tiny = |name: &str| {
        attribute(attributes, name).is_some_and(tiny_length)
            || declared(name).is_some_and(|value| tiny_length(&value))
    };
    let zero =
        |name: &str| declared(name).is_some_and(|value| length(&value).is_some_and(|n| n <= 0.0));
    let hidden = declared("display").is_some_and(|v| keyword(&v) == "none")
        || declared("visibility").is_some_and(|v| matches!(keyword(&v), "hidden" | "collapse"))
        || declared("opacity").is_some_and(|v| length(&v).is_some_and(|n| n <= 0.01));
    (tiny("width") && tiny("height")) || hidden || zero("max-width") || zero("max-height")
}

/// A value without `!important`, trimmed.
fn keyword(value: &str) -> &str {
    value.trim().trim_end_matches("!important").trim()
}

/// A length in pixels (or a plain number), if `value` is one.
fn length(value: &str) -> Option<f32> {
    let value = keyword(value).to_ascii_lowercase();
    let number = value.strip_suffix("px").unwrap_or(&value).trim();
    number.parse::<f32>().ok().or_else(|| {
        // Any zero length is zero, whatever its unit.
        let digits = value.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
        digits.parse::<f32>().ok().filter(|n| *n == 0.0)
    })
}

/// At most 2 px.
fn tiny_length(value: &str) -> bool {
    length(value).is_some_and(|n| n <= 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str, show: bool) -> Sanitized {
        sanitize(html, show, &InlineImages::new())
    }

    fn clean(html: &str) -> String {
        run(html, false).html
    }

    #[test]
    fn scripts_and_their_content_are_removed() {
        let html = clean(
            r#"<p>Hi</p><script>alert(1)</script><img src=x onerror="alert(2)"><a href="javascript:alert(3)">x</a><svg><script>alert(4)</script></svg>"#,
        );
        assert!(!html.contains("script"), "{html}");
        assert!(!html.contains("alert"), "{html}");
        assert!(!html.contains("onerror"), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(html.contains("<p>Hi</p>"), "{html}");
    }

    #[test]
    fn handlers_frames_forms_and_objects_are_removed() {
        let html = clean(
            r#"<div onclick="x()" onmouseover="y()">a</div><iframe src="https://evil.example"></iframe><form action="https://evil.example"><input name="password"></form><object data="x"></object><embed src="x"><base href="https://evil.example/"><meta http-equiv="refresh" content="0;url=https://evil.example">"#,
        );
        for gone in [
            "onclick",
            "onmouseover",
            "iframe",
            "form",
            "input",
            "object",
            "embed",
            "base",
            "meta",
            "evil",
        ] {
            assert!(!html.contains(gone), "{gone} in {html}");
        }
        assert!(html.contains("<div>a</div>"), "{html}");
    }

    #[test]
    fn style_sheets_are_removed_and_style_attributes_cannot_load_anything() {
        let html = clean(
            r#"<style>body{background:url(https://track.example/a.png)} @import "https://x.example/a.css";</style><p style="color: red; background: url(https://track.example/b.png); font-weight: bold">x</p><p style="background-image: u\72l(https://track.example/c)">y</p><p style="width: expression(alert(1))">z</p>"#,
        );
        assert!(!html.contains("track.example"), "{html}");
        assert!(!html.contains("@import"), "{html}");
        assert!(!html.contains("expression"), "{html}");
        assert!(!html.contains("<style"), "{html}");
        assert!(
            html.contains(r#"<p style="color: red; font-weight: bold">x</p>"#),
            "{html}"
        );
        assert!(html.contains("<p>y</p>"), "{html}");
    }

    #[test]
    fn links_open_in_a_new_tab_without_an_opener() {
        let html = clean(
            r#"<a href="https://example.com/a">a</a><a href="mailto:ana@example.com">m</a><a href="/relative">r</a><a href="data:text/html,x">d</a>"#,
        );
        assert!(
            html.contains(
                r#"<a href="https://example.com/a" target="_blank" rel="noopener noreferrer">a</a>"#
            ),
            "{html}"
        );
        assert!(html.contains(r#"href="mailto:ana@example.com""#), "{html}");
        assert!(!html.contains("/relative"), "{html}");
        assert!(!html.contains("data:"), "{html}");
    }

    #[test]
    fn remote_images_are_blocked_until_shown_and_counted() {
        let html = r#"<p><img src="https://cdn.example/logo.png" alt="Logo" width="120" height="40"></p><img src="http://cdn.example/b.png" alt="B">"#;
        let blocked = run(html, false);
        assert_eq!(blocked.remote_images, 2);
        assert!(!blocked.html.contains("cdn.example"), "{}", blocked.html);
        assert!(blocked.html.contains(r#"alt="Logo""#), "{}", blocked.html);
        // The placeholder takes the declared size, or its own.
        assert_eq!(
            blocked.html.matches(PLACEHOLDER).count(),
            2,
            "{}",
            blocked.html
        );
        assert!(blocked.html.contains(r#"width="120""#), "{}", blocked.html);
        assert_eq!(
            blocked.html.matches(r#"width="48" height="48""#).count(),
            1,
            "{}",
            blocked.html
        );
        assert!(only_own_sources(&blocked.html, &InlineImages::new()));

        let shown = run(html, true);
        assert_eq!(shown.remote_images, 2);
        assert!(
            shown.html.contains(r#"src="https://cdn.example/logo.png""#),
            "{}",
            shown.html
        );
    }

    #[test]
    fn tracking_pixels_are_removed_even_when_images_are_shown() {
        let html = r#"<p>Hello</p><img src="https://track.example/open.gif" width="1" height="1"><img src="https://track.example/o2.gif" width="0" height="0" alt=""><img src="https://track.example/o3.gif" style="display: none"><img src="https://track.example/o4.gif" width="1px" height="1px"><img src="https://cdn.example/real.png" width="300" height="1">"#;
        for show in [false, true] {
            let result = run(html, show);
            assert!(!result.html.contains("track.example"), "{}", result.html);
            assert_eq!(result.remote_images, 1, "{}", result.html);
            assert!(result.html.contains("<p>Hello</p>"));
        }
    }

    #[test]
    fn email_layout_survives() {
        let html = clean(
            r##"<table width="100%" cellpadding="0" bgcolor="#ffffff"><tr><td align="center" style="padding: 8px"><font color="#333333" face="Georgia">Hi</font></td></tr></table><center>c</center>"##,
        );
        assert!(html.contains(r#"cellpadding="0""#), "{html}");
        assert!(html.contains(r##"bgcolor="#ffffff""##), "{html}");
        assert!(html.contains(r#"style="padding: 8px""#), "{html}");
        assert!(
            html.contains(r##"<font color="#333333" face="Georgia">Hi</font>"##),
            "{html}"
        );
        assert!(html.contains("<center>c</center>"), "{html}");
    }

    #[test]
    fn an_img_lookalike_inside_an_attribute_is_left_alone() {
        let html = clean(
            r#"<a title="<img src=&quot;https://t.example/p&quot;>" href="https://example.com">x</a>"#,
        );
        assert!(html.contains("title="), "{html}");
        assert_eq!(run(&html, false).remote_images, 0);
    }

    /// Sources `Url::parse` accepts but a prefix check would miss.
    #[test]
    fn every_remote_source_is_blocked_whatever_its_case_or_whitespace() {
        let sources = [
            "HTTPS://t.example/a.gif",
            "hTtPs://t.example/b.gif",
            "Http://t.example/c.gif",
            " https://t.example/d.gif",
            "\thttps://t.example/e.gif",
            "\nhttps://t.example/f.gif",
            "https://t.example/g.gif ",
        ];
        for src in sources {
            let html = format!(r#"<p>x</p><img src="{src}" alt="a" width="300" height="80">"#);
            let blocked = run(&html, false);
            assert!(
                !blocked.html.contains("t.example"),
                "{src:?}: {}",
                blocked.html
            );
            assert!(
                only_own_sources(&blocked.html, &InlineImages::new()),
                "{src:?}: {}",
                blocked.html
            );
            assert!(blocked.html.contains(PLACEHOLDER), "{src:?}");
            assert_eq!(blocked.remote_images, 1, "{src:?}: {}", blocked.html);
            let shown = run(&html, true);
            assert!(shown.html.contains("t.example"), "{src:?}: {}", shown.html);
            assert_eq!(shown.remote_images, 1, "{src:?}");
        }
    }

    /// Markup a regex over the input would not recognise as an image.
    #[test]
    fn images_written_any_way_are_blocked() {
        let variants = [
            "<IMG SRC=https://t.example/a.gif>",
            "<img src='https://t.example/b.gif' alt='x'>",
            "<img/src=\"https://t.example/c.gif\">",
            "<img\nsrc=\"https://t.example/d.gif\"\n>",
            "<img alt=\"a>b\" src=\"https://t.example/e.gif\">",
            "<svg><image href=\"https://t.example/f.gif\"></image></svg>",
            "<picture><source srcset=\"https://t.example/g.gif\"><img src=\"https://t.example/h.gif\"></picture>",
        ];
        for html in variants {
            let blocked = run(html, false);
            assert!(
                !blocked.html.contains("t.example"),
                "{html:?}: {}",
                blocked.html
            );
            assert!(
                only_own_sources(&blocked.html, &InlineImages::new()),
                "{html:?}: {}",
                blocked.html
            );
        }
    }

    #[test]
    fn data_relative_and_unknown_inline_sources_become_the_placeholder() {
        let html =
            r#"<img src="data:image/png;base64,AAAA"><img src="/logo.png"><img src="cid:logo@x">"#;
        for show in [false, true] {
            let result = run(html, show);
            assert!(!result.html.contains("AAAA"), "{}", result.html);
            assert!(!result.html.contains("logo"), "{}", result.html);
            assert_eq!(
                result.html.matches(PLACEHOLDER).count(),
                3,
                "{}",
                result.html
            );
            assert!(only_own_sources(&result.html, &InlineImages::new()));
            assert_eq!(result.remote_images, 0, "{}", result.html);
        }
    }

    #[test]
    fn inline_images_show_their_part_without_asking() {
        let png = "data:image/png;base64,iVBORw0KGgo=".to_owned();
        let inline = InlineImages::from([("logo@example.com".to_owned(), png.clone())]);
        let html = r#"<p><img src="cid:Logo@Example.com" alt="Logo" width="120" height="40"></p><img src=" CID:%3Clogo%40example.com%3E "><img src="https://cdn.example/b.png"><a href="cid:logo@example.com">x</a>"#;
        let result = sanitize(html, false, &inline);
        assert_eq!(result.html.matches(&png).count(), 2, "{}", result.html);
        assert_eq!(result.remote_images, 1, "{}", result.html);
        assert!(!result.html.contains("cdn.example"), "{}", result.html);
        assert!(!result.html.contains("href"), "{}", result.html);
        assert!(!result.html.contains("cid:"), "{}", result.html);
        assert!(only_own_sources(&result.html, &inline));
        // A part's URL from another message is not one of this one's.
        assert!(!only_own_sources(&result.html, &InlineImages::new()));
    }

    #[test]
    fn a_source_the_filter_did_not_write_fails_closed() {
        let inline = InlineImages::new();
        for leaked in [
            r#"<img src="https://t.example/a.gif">"#,
            r"<img src='data:image/png;base64,AAAA'>",
            r"<img src=https://t.example/b.gif>",
        ] {
            assert!(!only_own_sources(leaked, &inline), "{leaked}");
        }
        assert!(only_own_sources(
            &format!(r#"<p>x</p><img src="{PLACEHOLDER}">"#),
            &inline
        ));
    }

    /// The fixture emails (campbooks-cloud#160): real-world-shaped mail and
    /// a hostile message.
    fn fixture(name: &str) -> &'static str {
        match name {
            "newsletter" => include_str!("../../../fixtures/email-html/newsletter.html"),
            "receipt" => include_str!("../../../fixtures/email-html/receipt.html"),
            "outlook" => include_str!("../../../fixtures/email-html/outlook.html"),
            "gmail" => include_str!("../../../fixtures/email-html/gmail.html"),
            "marketing-600" => include_str!("../../../fixtures/email-html/marketing-600.html"),
            "marketing-fluid" => include_str!("../../../fixtures/email-html/marketing-fluid.html"),
            "hostile" => include_str!("../../../fixtures/email-html/hostile.html"),
            _ => unreachable!("no fixture {name}"),
        }
    }

    const FIXTURES: [&str; 7] = [
        "newsletter",
        "receipt",
        "outlook",
        "gmail",
        "marketing-600",
        "marketing-fluid",
        "hostile",
    ];

    static HANDLER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)<[^>]*\son[a-z]+\s*=").unwrap_or_else(|e| unreachable!("{e}"))
    });

    /// What no output may ever hold, for any fixture, images shown or not.
    fn assert_inert(name: &str, result: &Sanitized, show: bool) {
        let html = result.html.to_ascii_lowercase();
        for gone in [
            "<script",
            "<style",
            "<title",
            "<form",
            "<input",
            "<button",
            "<iframe",
            "<object",
            "<embed",
            "<meta",
            "<base",
            "<link",
            "<svg",
            "<math",
            "<video",
            "<audio",
            "javascript:",
            "vbscript:",
            "data:text",
            "url(",
            "image-set",
            "@import",
            "expression(",
            "behavior",
            "-moz-binding",
            "alert",
            "evil.example",
            "exfil.example",
            "track.example",
            "<!--",
        ] {
            assert!(!html.contains(gone), "{name} (show {show}): {gone}");
        }
        assert!(!HANDLER.is_match(&html), "{name} (show {show}): a handler");
        if !show {
            assert!(
                only_own_sources(&result.html, &InlineImages::new()),
                "{name}: a remote source while blocked"
            );
        }
        if let Some(css) = &result.css {
            let css = css.to_ascii_lowercase();
            for gone in [
                "url(",
                "@import",
                "@font-face",
                "expression",
                "prefers-color-scheme",
                "\\",
                "<",
                "behavior",
                "image-set",
            ] {
                assert!(!css.contains(gone), "{name}: {gone} in css");
            }
        }
    }

    #[test]
    fn no_fixture_can_run_load_or_submit_anything() {
        for name in FIXTURES {
            for show in [false, true] {
                assert_inert(name, &run(fixture(name), show), show);
            }
        }
    }

    #[test]
    fn a_newsletter_keeps_its_style_sheet_as_inline_styles() {
        let result = run(fixture("newsletter"), false);
        let html = &result.html;
        // The document title is not part of the message.
        assert!(!html.contains("Simple Transactional Email"), "{html}");
        // The preheader stays hidden, the button keeps its look.
        assert!(
            html.contains(r#"<span class="preheader" style="color: transparent; display: none"#),
            "{html}"
        );
        assert!(html.contains("background-color: #0867ec"), "{html}");
        // The body's background moves to the wrapper.
        assert!(
            html.starts_with(r#"<div style="font-family: Helvetica"#),
            "{html}"
        );
        assert!(html.contains("background-color: #f4f5f6"), "{html}");
        // The responsive rules come back separately.
        let css = result.css.unwrap_or_default();
        assert!(
            css.contains("@media only screen and (max-width: 640px)"),
            "{css}"
        );
        assert!(css.contains(".container { padding: 0 !important"), "{css}");
    }

    #[test]
    fn a_receipt_loses_its_web_font_and_dark_rules_but_keeps_its_tables() {
        let result = run(fixture("receipt"), true);
        assert!(!result.html.contains("fonts.googleapis"), "{}", result.html);
        assert!(result.html.contains("<table"), "{}", result.html);
        assert!(result.html.contains("R-20418"));
        let css = result.css.unwrap_or_default();
        assert!(!css.contains("prefers-color-scheme"), "{css}");
        assert!(css.contains("max-width: 600px"), "{css}");
    }

    #[test]
    fn outlook_and_gmail_replies_keep_their_shape() {
        let outlook = clean(fixture("outlook"));
        assert!(!outlook.contains("o:p"), "{outlook}");
        assert!(
            !outlook.contains("mso-hide") && !outlook.contains("[if"),
            "{outlook}"
        );
        // `p.MsoNormal { margin: 0 }` from Word's sheet.
        assert!(
            outlook.contains(r#"<p class="MsoNormal" style="margin: 0cm"#),
            "{outlook}"
        );
        let gmail = clean(fixture("gmail"));
        assert!(
            gmail.contains(r#"<blockquote class="gmail_quote" style="margin: 0px 0px 0px 0.8ex; border-left: 1px solid rgb(204,204,204); padding-left: 1ex">"#),
            "{gmail}"
        );
    }

    #[test]
    fn the_hostile_fixture_keeps_only_its_harmless_parts() {
        for show in [false, true] {
            let result = run(fixture("hostile"), show);
            let html = &result.html;
            assert!(
                html.contains(r#"<p class="fine" style="color: #0a7d32; font-weight: bold">"#),
                "{html}"
            );
            // The class beacon's colour is kept, its background is not.
            assert!(
                html.contains(r#"<p class="beacon" style="color: #123456">"#),
                "{html}"
            );
            assert!(
                html.contains(r#"<p style="color: red; font-size: 14px">"#),
                "{html}"
            );
            assert!(
                html.contains(
                    r#"href="https://example.com/ok" target="_blank" rel="noopener noreferrer""#
                ),
                "{html}"
            );
            assert!(!html.contains("ping="), "{html}");
            // Only the real logo is a remote image; every pixel is gone.
            assert_eq!(result.remote_images, 1, "{html}");
            assert_eq!(result.css, None);
        }
    }

    #[test]
    fn pixels_sized_or_hidden_by_style_are_removed() {
        let html = r#"<p>x</p><img src="https://t.example/a.gif" style="width:1px;height:1px"><img src="https://t.example/b.gif" width="200" height="50" style="max-height:0"><img src="https://t.example/c.gif" width="200" height="50" style="visibility: hidden"><img src="https://t.example/d.gif" width="200" height="50" style="opacity: 0"><img src="https://t.example/e.gif" style="WIDTH: 0PX; height: 0 !important"><img src="https://cdn.example/real.png" width="200" height="50" style="opacity: 1">"#;
        for show in [false, true] {
            let result = run(html, show);
            assert!(!result.html.contains("t.example"), "{}", result.html);
            assert_eq!(result.remote_images, 1, "{}", result.html);
        }
    }

    #[test]
    fn a_fixed_width_layout_keeps_its_widths_for_the_reader_to_fit() {
        let html = clean(fixture("marketing-600"));
        assert!(html.contains(r#"width="600""#), "{html}");
        assert!(html.contains("width: 600px"), "{html}");
    }

    /// The regex passes run on ammonia's output, which escapes `<` and `>`
    /// inside attribute values (html5ever): an image written inside an
    /// attribute must never be touched by them.
    #[test]
    fn an_image_inside_an_attribute_value_survives_unchanged() {
        for show in [false, true] {
            let result = run(
                r#"<p title="<img>" class="<img/> x">a</p><a href="https://example.com" title='<img src="x" onerror="y">'>b</a>"#,
                show,
            );
            assert!(
                result
                    .html
                    .contains(r#"<p title="&lt;img&gt;" class="&lt;img/&gt; x">a</p>"#),
                "{}",
                result.html
            );
            assert!(!result.html.contains(PLACEHOLDER), "{}", result.html);
            assert!(!result.html.contains("<img"), "{}", result.html);
        }
    }

    #[test]
    fn a_body_style_cannot_smuggle_declarations_into_the_wrapper() {
        let result = run(
            r"<body style='x\3b background-image\3a url\28 https\3a \2f \2f t\2e example\2f p\29 \3b y:red; color: #333'><p>x</p></body>",
            true,
        );
        assert!(!result.html.contains("t.example"), "{}", result.html);
        assert!(
            result.html.starts_with(r#"<div style="color: #333">"#),
            "{}",
            result.html
        );
    }

    #[test]
    fn html_built_to_be_slow_is_shown_as_its_text() {
        let html = format!("<p>Hello</p>{}", "<div></span>".repeat(170_000));
        assert!(html.len() < 2 * 1024 * 1024);
        let result = run(&html, true);
        assert!(
            result
                .html
                .starts_with(r#"<div style="white-space: pre-wrap">"#),
            "{}",
            &result.html[..80]
        );
        assert!(result.html.contains("Hello"));
        assert_eq!(result.html.matches('<').count(), 2, "only the wrapper");
        assert_eq!(result.remote_images, 0);
        assert_eq!(result.css, None);
    }

    #[test]
    fn a_leading_frameset_is_shown_as_text_too() {
        // A document drops what follows; ammonia's fragment parse would not.
        let html = format!("<frameset><p>Hi</p>{}", "<div></span>".repeat(170_000));
        let result = run(&html, false);
        assert!(
            result
                .html
                .starts_with(r#"<div style="white-space: pre-wrap">"#)
        );
        assert_eq!(result.html.matches('<').count(), 2, "only the wrapper");
    }

    #[test]
    fn a_deep_but_cheap_body_is_sanitised_on_a_small_stack() {
        let html = format!(
            "<style>font {{ color: red }}</style>{}<img src=\"https://t.example/a.png\">",
            "<font>x".repeat(4_000)
        );
        // Tokio's blocking threads have 2 MiB of stack.
        let result = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || run(&html, false))
            .expect("spawn")
            .join()
            .expect("sanitised without overflowing the stack");
        assert!(result.html.contains('x'));
    }

    /// The sanitised fixtures the client's policy test (`test:csp`) renders
    /// under the desktop and web policies (campbooks-cloud#199) are this sanitiser's
    /// output, held back and shown: a change here shows up there.
    /// `UPDATE_SNAPSHOTS=1 cargo test -p campbooks-conversations webkit_snapshots`
    /// writes them again.
    #[test]
    fn the_webkit_snapshots_are_this_sanitiser_s_output() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../clients/app/test/webkit/fixtures"
        );
        let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
        let mut stale = Vec::new();
        for name in ["newsletter", "marketing-600", "outlook", "hostile"] {
            for show in [false, true] {
                let html = sanitize(fixture(name), show, &InlineImages::new()).html;
                let kind = if show { "shown" } else { "held" };
                let path = format!("{dir}/{name}.{kind}.html");
                if update {
                    std::fs::write(&path, format!("{html}\n")).expect("snapshot written");
                } else if std::fs::read_to_string(&path).ok().as_deref()
                    != Some(format!("{html}\n").as_str())
                {
                    stale.push(format!("===== {name}.{kind}.html\n{html}\n===== end"));
                }
            }
        }
        assert!(
            stale.is_empty(),
            "stale WebKit snapshots (UPDATE_SNAPSHOTS=1 cargo test -p campbooks-conversations \
             webkit_snapshots):\n{}",
            stale.join("\n")
        );
    }
}
