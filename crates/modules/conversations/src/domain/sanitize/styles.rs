//! A message's style sheets, made to survive sanitising: their rules are
//! inlined into `style` attributes (css-inline, on html5ever and Servo's
//! selector matching) before ammonia cleans the HTML, the way senders
//! inline their own mail for clients that drop `<style>`. The page's policy
//! blocks `<style>` in the frame, and that stays so.
//!
//! What cannot be inlined is not lost when it is a `@media` rule: those
//! come back separately ([`super::css::media_rules`]) for the client to
//! apply to the frame's document. The `<body>`'s own look (its inlined
//! style and `bgcolor`) comes back too, for the wrapper that replaces it.
//!
//! The inliner never reads the message's own CSS: the sheets are filtered
//! first ([`super::css::plain_rules`]: inert selectors, inert declarations,
//! plain property names) and only that is handed to it, so what it writes
//! into `style` attributes was already checked (and is filtered again by
//! ammonia's pass). Nothing here fetches or reads anything: remote style
//! sheets are never loaded (css-inline without its `http` and `file`
//! features, `load_remote_stylesheets(false)`, and a resolver that refuses
//! every location).
//!
//! Inlining is skipped (the sheets are dropped, as before) when the work
//! could get out of hand: more than [`MAX_STYLE_BYTES`] of style text,
//! elements nested deeper than [`MAX_NESTING`] (the inliner's serialiser
//! recurses), more than [`MAX_ELEMENTS`] elements, or more than
//! [`MAX_WORK`] elements times distinct properties (the size of what it
//! could write).

use std::{borrow::Cow, cell::RefCell, sync::Arc};

use super::css;
use css_inline::{CSSInliner, InlineError, StylesheetResolver};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        states::RawKind,
    },
};

/// Most bytes of `<style>` text inlined.
pub const MAX_STYLE_BYTES: usize = 256 * 1024;
/// Deepest element nesting inlined.
pub const MAX_NESTING: usize = 256;
/// Most elements inlined.
pub const MAX_ELEMENTS: usize = 20_000;
/// Most elements times distinct properties inlined.
pub const MAX_WORK: usize = 1_000_000;

/// Elements that never take content, so never nest.
const VOID: [&str; 15] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr", "param", "keygen",
];

/// The HTML with its style sheets inlined, and what inlining left over.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inlined {
    /// The HTML to sanitise: rules inlined, `<style>` and `<link>` gone
    /// (or the input unchanged when it had no sheet, or inlining failed).
    pub html: String,
    /// The `<body>` element's `style` (with the rules that matched it) and
    /// `bgcolor`, unfiltered.
    pub body: Vec<(String, String)>,
    /// The sheets' `@media` rules, filtered ([`css::media_rules`]).
    pub media: Option<String>,
}

/// Inline the style sheets of `html`.
pub fn inline(html: &str) -> Inlined {
    let found = scan(html);
    let unchanged = |media: Option<String>| Inlined {
        html: html.to_owned(),
        body: found.body.clone(),
        media,
    };
    if found.styles.trim().is_empty()
        || found.styles.len() > MAX_STYLE_BYTES
        || found.deepest > MAX_NESTING
        || found.elements > MAX_ELEMENTS
    {
        return unchanged(None);
    }
    let media = css::media_rules(&found.styles);
    let rules = css::plain_rules(&found.styles);
    if rules.css.is_empty() || rules.properties.saturating_mul(found.elements) > MAX_WORK {
        return unchanged(media);
    }
    let inliner = CSSInliner::options()
        // The message's own `<style>` text is never read: only the
        // filtered rules.
        .inline_style_tags(false)
        .keep_at_rules(false)
        .keep_style_tags(false)
        .keep_link_tags(false)
        .load_remote_stylesheets(false)
        .resolver(Arc::new(Refuse))
        .extra_css(Some(Cow::Owned(rules.css)))
        .build();
    match inliner.inline(html) {
        Ok(output) => Inlined {
            body: scan(&output).body,
            html: output,
            media,
        },
        Err(_) => unchanged(media),
    }
}

/// A resolver that loads nothing: no network, no file.
struct Refuse;

impl StylesheetResolver for Refuse {
    fn retrieve(&self, _location: &str) -> Result<String, InlineError> {
        Err(self.unsupported("style sheets are never loaded"))
    }
}

/// What one pass of the tokenizer finds.
#[derive(Debug, Default)]
struct Found {
    /// The text of every `<style>` element, joined.
    styles: String,
    /// The first `<body>` start tag's `style` and `bgcolor`.
    body: Vec<(String, String)>,
    /// Elements seen.
    elements: usize,
    /// The deepest nesting of elements (counted from start and end tags, an
    /// upper bound in practice for what the tree builder makes).
    deepest: usize,
}

/// Read `html`'s style text and body attributes with html5ever's tokenizer,
/// switching to raw text where the tree builder would.
fn scan(html: &str) -> Found {
    let tokenizer = Tokenizer::new(Scanner::default(), TokenizerOpts::default());
    let queue = BufferQueue::default();
    queue.push_back(StrTendril::from_slice(html));
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    tokenizer.sink.found.take()
}

#[derive(Default)]
struct Scanner {
    found: RefCell<Found>,
    in_style: RefCell<bool>,
    seen_body: RefCell<bool>,
    depth: RefCell<usize>,
}

impl TokenSink for Scanner {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        match token {
            Token::TagToken(tag) => {
                let name = &*tag.name;
                if tag.kind == TagKind::EndTag {
                    if name == "style" {
                        *self.in_style.borrow_mut() = false;
                    }
                    if !VOID.contains(&name) {
                        let mut depth = self.depth.borrow_mut();
                        *depth = depth.saturating_sub(1);
                    }
                    return TokenSinkResult::Continue;
                }
                {
                    let mut found = self.found.borrow_mut();
                    found.elements += 1;
                    if !VOID.contains(&name) {
                        let mut depth = self.depth.borrow_mut();
                        *depth += 1;
                        found.deepest = found.deepest.max(*depth);
                    }
                }
                if name == "body" && !*self.seen_body.borrow() {
                    *self.seen_body.borrow_mut() = true;
                    self.found.borrow_mut().body = tag
                        .attrs
                        .iter()
                        .filter(|a| matches!(&*a.name.local, "style" | "bgcolor"))
                        .map(|a| (a.name.local.to_string(), a.value.to_string()))
                        .collect();
                }
                match name {
                    "style" => {
                        *self.in_style.borrow_mut() = true;
                        self.found.borrow_mut().styles.push('\n');
                        TokenSinkResult::RawData(RawKind::Rawtext)
                    }
                    "xmp" | "iframe" | "noembed" | "noframes" | "noscript" => {
                        TokenSinkResult::RawData(RawKind::Rawtext)
                    }
                    "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
                    "script" => TokenSinkResult::RawData(RawKind::ScriptData),
                    "plaintext" => TokenSinkResult::Plaintext,
                    _ => TokenSinkResult::Continue,
                }
            }
            Token::CharacterTokens(text) if *self.in_style.borrow() => {
                let mut found = self.found.borrow_mut();
                if found.styles.len() <= MAX_STYLE_BYTES {
                    found.styles.push_str(&text);
                }
                TokenSinkResult::Continue
            }
            _ => TokenSinkResult::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_inlined_and_sheets_removed() {
        let result = inline(
            r#"<html><head><style>.lead { color: #3d4451; } p.lead { font-size: 16px }</style></head><body><p class="lead">Hi</p></body></html>"#,
        );
        assert!(!result.html.contains("<style"), "{}", result.html);
        assert!(result.html.contains("color: #3d4451"), "{}", result.html);
        assert!(result.html.contains("font-size: 16px"), "{}", result.html);
    }

    #[test]
    fn body_rules_and_attributes_come_back_for_the_wrapper() {
        let result = inline(
            r##"<html><head><style>body { background-color: #f4f5f6; }</style></head><body bgcolor="#eeeeee" onload="x()"><p>Hi</p></body></html>"##,
        );
        assert!(
            result
                .body
                .iter()
                .any(|(n, v)| n == "style" && v.contains("#f4f5f6")),
            "{:?}",
            result.body
        );
        assert!(result.body.contains(&("bgcolor".into(), "#eeeeee".into())));
        assert!(!result.body.iter().any(|(n, _)| n == "onload"));
    }

    #[test]
    fn media_rules_are_kept_back_not_inlined() {
        let result = inline(
            r"<style>.c { width: 600px } @media (max-width: 600px) { .c { width: 100% !important } }</style><div class=c>x</div>",
        );
        assert_eq!(
            result.media.as_deref(),
            Some("@media (max-width: 600px) {\n.c { width: 100% !important }\n}")
        );
        assert!(
            result
                .html
                .contains(r#"<div class="c" style="width: 600px;">"#),
            "{}",
            result.html
        );
    }

    #[test]
    fn nothing_is_ever_loaded() {
        // A remote sheet, a local file and an import: none is read, and
        // inlining still happens for the rest.
        let result = inline(
            r#"<link rel="stylesheet" href="https://t.example/a.css"><link rel="stylesheet" href="file:///etc/passwd"><link rel="stylesheet" href="/etc/passwd"><style>@import url("file:///etc/passwd"); p { color: red }</style><p>x</p>"#,
        );
        assert!(!result.html.contains("<link"), "{}", result.html);
        assert!(!result.html.contains("root:"), "{}", result.html);
        assert!(result.html.contains("color: red"), "{}", result.html);
        assert!(Refuse.retrieve("/etc/passwd").is_err());
        assert!(Refuse.retrieve("https://t.example/a.css").is_err());
    }

    #[test]
    fn mail_without_a_sheet_is_left_alone() {
        let html = "<div dir=ltr>Hi <b>Sam</b></div>";
        assert_eq!(inline(html).html, html);
    }

    #[test]
    fn style_text_in_other_raw_elements_is_not_a_sheet() {
        let found = scan(
            "<title><style>x</style></title><textarea><style>y</style></textarea><style>z</style>",
        );
        assert_eq!(found.styles.trim(), "z");
    }

    #[test]
    fn an_oversized_sheet_is_dropped_not_inlined() {
        let sheet = format!(
            "<style>{}</style><p class=a>x</p>",
            ".a{color:red}".repeat(30_000)
        );
        let result = inline(&sheet);
        assert!(!result.html.contains("style="), "nothing inlined");
        assert_eq!(result.media, None);
    }

    #[test]
    fn deep_nesting_is_not_inlined() {
        let html = format!(
            "<style>a {{ color: red }}</style>{}<a>x</a>{}",
            "<div>".repeat(MAX_NESTING + 10),
            "</div>".repeat(MAX_NESTING + 10)
        );
        let result = inline(&html);
        assert_eq!(result.html, html);
        // Siblings are not nesting.
        let wide = format!(
            "<style>p {{ color: red }}</style>{}",
            "<p>x</p>".repeat(500)
        );
        assert!(inline(&wide).html.contains("color: red"));
    }

    #[test]
    fn work_past_the_cap_is_not_inlined() {
        let properties: String = (0..200).map(|i| format!("a{i}: x;")).collect();
        let html = format!(
            "<style>* {{ {properties} }}</style>{}",
            "<b></b>".repeat(6_000)
        );
        let result = inline(&html);
        assert_eq!(result.html, html);
    }

    #[test]
    fn the_inliner_reads_only_filtered_rules() {
        let result = inline(
            r"<style>p { x\3b background-image\3a url\28 https\3a \2f \2f t\2e example\2f p\29 \3b y: red; color: blue } div { background: url(https://t.example/d) }</style><p>x</p><div>y</div>",
        );
        assert!(!result.html.contains("t.example"), "{}", result.html);
        assert!(!result.html.contains("url("), "{}", result.html);
        assert!(
            result.html.contains(r#"<p style="color: blue;">"#),
            "{}",
            result.html
        );
    }
}
