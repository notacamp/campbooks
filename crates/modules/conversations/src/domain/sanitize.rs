//! Sanitising untrusted email HTML before it leaves the server.
//!
//! The client shows the result only in a sandboxed frame with its own CSP
//! (`lib/ui/safe-html`); this is the first of the two layers, so the frame
//! never receives markup that could run, load or submit anything unasked:
//!
//! - **ammonia** keeps a fixed allow-list of tags and attributes, drops
//!   `<script>`, `<style>` (with their content), forms, frames, event
//!   handlers and comments, allows only `http`, `https`, `mailto` and `tel`
//!   links, and makes every link open in a new tab with
//!   `rel="noopener noreferrer"`;
//! - **style attributes** keep only declarations that cannot load or run
//!   anything (no `url(`, `@import`, `expression`, escapes);
//! - **images**: remote images lose their `src` unless the member asked to
//!   see them (and are counted, so the reader can offer "Show images"), and
//!   tracking pixels (an image of at most 2×2, or one hidden by its style)
//!   are removed either way.

use std::{borrow::Cow, collections::HashSet, fmt::Write as _, sync::LazyLock};

use ammonia::{Builder, UrlRelative};
use regex::Regex;

/// Sanitised HTML and what was held back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    /// Safe to show in the sandboxed frame.
    pub html: String,
    /// Remote images in the message (blocked unless shown).
    pub remote_images: u32,
}

/// Tags email layouts use, on top of ammonia's defaults.
const EXTRA_TAGS: [&str; 2] = ["center", "font"];

/// Presentational attributes allowed on any tag.
const GENERIC_ATTRIBUTES: [&str; 8] = [
    "style", "align", "valign", "width", "height", "bgcolor", "dir", "border",
];

/// Declarations in a `style` attribute that can load or run something, or
/// hide what they do behind CSS escapes.
const UNSAFE_CSS: [&str; 7] = [
    "url(",
    "image-set(",
    "@import",
    "expression",
    "behavior",
    "-moz-binding",
    "\\",
];

/// Sanitise `html`. Remote images keep their `src` only when
/// `show_remote_images` is set.
pub fn sanitize(html: &str, show_remote_images: bool) -> Sanitized {
    let cleaned = builder().clean(html).to_string();
    images(&cleaned, show_remote_images)
}

fn builder() -> Builder<'static> {
    let mut builder = Builder::default();
    builder
        .add_tags(EXTRA_TAGS)
        .add_generic_attributes(GENERIC_ATTRIBUTES)
        .add_tag_attributes("img", ["src", "alt", "width", "height"])
        .add_tag_attributes("font", ["color", "face", "size"])
        .add_tag_attributes("td", ["colspan", "rowspan"])
        .add_tag_attributes("th", ["colspan", "rowspan"])
        .add_tag_attributes("table", ["cellpadding", "cellspacing"])
        .url_schemes(HashSet::from(["http", "https", "mailto", "tel"]))
        // Mail has no base URL: a relative link or image goes nowhere.
        .url_relative(UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank")
        .attribute_filter(|_element, attribute, value| {
            if attribute == "style" {
                safe_style(value).map(Cow::Owned)
            } else {
                Some(Cow::Borrowed(value))
            }
        });
    builder
}

/// The declarations of a `style` attribute that are safe to keep, or `None`
/// when none are.
fn safe_style(style: &str) -> Option<String> {
    let kept: Vec<&str> = style
        .split(';')
        .map(str::trim)
        .filter(|declaration| !declaration.is_empty())
        .filter(|declaration| {
            let lower = declaration.to_ascii_lowercase();
            !UNSAFE_CSS.iter().any(|bad| lower.contains(bad))
        })
        .collect();
    (!kept.is_empty()).then(|| kept.join("; "))
}

/// An `<img>` tag as ammonia serialises it: lower-case attribute names,
/// double-quoted values (a `"` inside a value is always `&quot;`).
static IMG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<img((?:\s+[a-z-]+="[^"]*")*)\s*/?>"#).unwrap_or_else(|e| unreachable!("{e}"))
});
static ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-z-]+)="([^"]*)""#).unwrap_or_else(|e| unreachable!("{e}")));

/// Apply the image policy to ammonia's output.
fn images(html: &str, show_remote: bool) -> Sanitized {
    let mut remote_images = 0_u32;
    let html = IMG.replace_all(html, |tag: &regex::Captures<'_>| {
        let attributes: Vec<(&str, &str)> = ATTRIBUTE
            .captures_iter(tag.get(1).map_or("", |m| m.as_str()))
            .filter_map(|c| Some((c.get(1)?.as_str(), c.get(2)?.as_str())))
            .collect();
        if is_tracking_pixel(&attributes) {
            return String::new();
        }
        let remote = attribute(&attributes, "src")
            .is_some_and(|src| src.starts_with("http://") || src.starts_with("https://"));
        if remote {
            remote_images += 1;
        }
        let mut out = String::from("<img");
        for (name, value) in &attributes {
            if *name == "src" && (!remote || !show_remote) {
                continue;
            }
            let _ = write!(out, " {name}=\"{value}\"");
        }
        if remote && !show_remote {
            out.push_str(" data-blocked=\"remote\"");
        }
        out.push('>');
        out
    });
    Sanitized {
        html: html.into_owned(),
        remote_images,
    }
}

fn attribute<'a>(attributes: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value)| *value)
}

/// An image too small to see, or hidden: there to report that the message
/// was opened.
fn is_tracking_pixel(attributes: &[(&str, &str)]) -> bool {
    let tiny = |name: &str| {
        attribute(attributes, name).is_some_and(|value| {
            value
                .trim()
                .trim_end_matches("px")
                .parse::<f32>()
                .is_ok_and(|n| n <= 2.0)
        })
    };
    let hidden = attribute(attributes, "style").is_some_and(|style| {
        let style = style.to_ascii_lowercase().replace(' ', "");
        style.contains("display:none")
            || style.contains("visibility:hidden")
            || style.contains("opacity:0;")
            || style.ends_with("opacity:0")
    });
    (tiny("width") && tiny("height")) || hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> String {
        sanitize(html, false).html
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
        let blocked = sanitize(html, false);
        assert_eq!(blocked.remote_images, 2);
        assert!(!blocked.html.contains("cdn.example"), "{}", blocked.html);
        assert!(blocked.html.contains(r#"alt="Logo""#), "{}", blocked.html);
        assert!(
            blocked.html.contains(r#"data-blocked="remote""#),
            "{}",
            blocked.html
        );

        let shown = sanitize(html, true);
        assert_eq!(shown.remote_images, 2);
        assert!(
            shown.html.contains(r#"src="https://cdn.example/logo.png""#),
            "{}",
            shown.html
        );
        assert!(!shown.html.contains("data-blocked"), "{}", shown.html);
    }

    #[test]
    fn tracking_pixels_are_removed_even_when_images_are_shown() {
        let html = r#"<p>Hello</p><img src="https://track.example/open.gif" width="1" height="1"><img src="https://track.example/o2.gif" width="0" height="0" alt=""><img src="https://track.example/o3.gif" style="display: none"><img src="https://track.example/o4.gif" width="1px" height="1px"><img src="https://cdn.example/real.png" width="300" height="1">"#;
        for show in [false, true] {
            let result = sanitize(html, show);
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
        assert_eq!(sanitize(&html, false).remote_images, 0);
    }
}
