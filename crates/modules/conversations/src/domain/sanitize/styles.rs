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
//! Nothing here fetches or reads anything: remote style sheets are never
//! loaded (css-inline without its `http` and `file` features, and a
//! resolver that refuses every location), and every declaration is
//! filtered again afterwards ([`super::css`]).

use std::{cell::RefCell, sync::Arc};

use css_inline::{CSSInliner, InlineError, StylesheetResolver};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        states::RawKind,
    },
};

/// Most bytes of `<style>` text inlined; past that, the sheets are dropped
/// as before (selector matching grows with rules times elements).
pub const MAX_STYLE_BYTES: usize = 256 * 1024;

/// The HTML with its style sheets inlined, and what inlining left over.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inlined {
    /// The HTML to sanitise: rules inlined, `<style>` and `<link>` gone
    /// (or the input unchanged when it had no sheet, or inlining failed).
    pub html: String,
    /// The `<body>` element's `style` (with the rules that matched it) and
    /// `bgcolor`, unfiltered.
    pub body: Vec<(String, String)>,
    /// The sheets' text that inlining kept back (their at-rules), unfiltered.
    pub leftover: String,
}

/// Inline the style sheets of `html`.
pub fn inline(html: &str) -> Inlined {
    let original = scan(html);
    let fallback = || Inlined {
        html: html.to_owned(),
        body: original.body.clone(),
        leftover: String::new(),
    };
    if original.styles.is_empty() || original.styles.len() > MAX_STYLE_BYTES {
        return fallback();
    }
    let inliner = CSSInliner::options()
        .inline_style_tags(true)
        .keep_style_tags(false)
        .keep_link_tags(false)
        .keep_at_rules(true)
        .load_remote_stylesheets(false)
        .resolver(Arc::new(Refuse))
        .build();
    match inliner.inline(html) {
        Ok(output) => {
            let found = scan(&output);
            Inlined {
                html: output,
                body: found.body,
                leftover: found.styles,
            }
        }
        Err(_) => fallback(),
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
                    return TokenSinkResult::Continue;
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
        assert!(result.leftover.contains("@media"), "{}", result.leftover);
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
        assert!(result.leftover.is_empty());
    }
}
