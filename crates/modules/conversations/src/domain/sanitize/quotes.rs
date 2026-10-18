//! Quoted history: the block a reply carries of the mail before it, found
//! from its markup alone, never from its words.
//!
//! Mail clients wrap what they quote in markup of their own: Gmail's
//! `gmail_quote` (its `gmail_attr` attribution inside), Apple Mail's and
//! Thunderbird's `<blockquote type="cite">` (Thunderbird's attribution in a
//! `moz-cite-prefix` just before it), Yahoo's `yahoo_quoted`, Zoho's
//! `zmail_extra` (after a `zmail_extra_hr`), and Outlook's `appendonsend` /
//! `divRplyFwdMsg`, after which the rest of their parent is the quoted
//! message. [`mark`] wraps the first such block in one
//! `<div data-cb-quoted="1">` for the reader to fold, when:
//!
//! - something visible comes before it (a message that is only a quote, a
//!   bare forward, is shown whole);
//! - nothing visible comes after it: it is trailing;
//! - it holds no visible text of its own between or after its quotes (an
//!   inline reply, answered between the quotes, is shown whole);
//! - a `div` may stand where it is (not inside a paragraph, a list or a
//!   table's rows).
//!
//! A quote inside the folded block is folded with it; only the outermost
//! block is marked.
//!
//! It reads the sanitiser's last output: html5ever's serialisation, where
//! every element but a void one has its end tag, attribute values are always
//! double-quoted and never hold a raw `"`, text never holds a raw `<`, and no
//! comment or raw-text element survives. The version in use also escapes `<`
//! and `>` inside attribute values, but the reading here does not rely on
//! it: a tag ends at the first `>` outside a quoted value, so a `<` always
//! starts a tag and the nesting read is the tree's. Anything that does not
//! fit that shape (a tag that never ends, a name that is not a plain tag
//! name, an end tag that does not close the innermost open element, an
//! element left open) leaves the HTML unchanged: [`mark`] never guesses.
//! The mark itself is never allowed through ammonia, so a sender cannot write
//! one: the only `data-cb-quoted` in the output is the one added here.

use std::fmt::Write as _;

/// The attribute the reader folds. Never allowed through ammonia.
pub const MARK: &str = "data-cb-quoted";

/// Outlook's ids before its quoted message, kept by the sanitiser on a
/// `div` (only these values).
pub const OUTLOOK_IDS: [&str; 2] = ["appendonsend", "divRplyFwdMsg"];

/// Elements that end at their start tag.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Parents a `div` (and the reader's `details`) may stand in without the
/// parser moving it: flow content.
const FLOW_PARENTS: [&str; 18] = [
    "div",
    "td",
    "th",
    "li",
    "dd",
    "blockquote",
    "center",
    "section",
    "article",
    "aside",
    "header",
    "footer",
    "nav",
    "figure",
    "details",
    "font",
    "span",
    "body",
];

/// Characters that take no room: zero-width spaces and joiners, the
/// byte-order mark, the soft hyphen, the combining grapheme joiner
/// (preheader padding).
const INVISIBLE: [char; 7] = [
    '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}', '\u{ad}', '\u{34f}',
];

/// One element of the output.
#[derive(Debug)]
struct Element {
    name: String,
    class: String,
    id: String,
    kind: String,
    parent: Option<usize>,
    /// The element just before it under the same parent, with nothing
    /// visible between them.
    previous: Option<usize>,
    /// Where its start tag begins.
    start: usize,
    /// Where its end tag begins (its start tag's end for a void element).
    inner_end: usize,
    /// Blockquotes around it.
    depth: usize,
}

/// Something visible: text or an image.
#[derive(Debug, Clone, Copy)]
struct Visible {
    at: usize,
    /// Blockquotes around it.
    depth: usize,
}

/// The output read as a tree.
#[derive(Debug, Default)]
struct Tree {
    elements: Vec<Element>,
    visible: Vec<Visible>,
}

impl Element {
    fn has_class(&self, name: &str) -> bool {
        self.class
            .split_ascii_whitespace()
            .any(|class| class == name)
    }

    fn is(&self, name: &str) -> bool {
        self.name == name
    }
}

/// `html` (the sanitiser's output) with its trailing quoted history wrapped
/// in `<div data-cb-quoted="1">`, or unchanged when it has none.
pub fn mark(html: &str) -> String {
    let Some(tree) = read(html) else {
        return html.to_owned();
    };
    match quoted(&tree, html.len()) {
        // Only ever between two tags, never inside one.
        Some((start, end))
            if html[start..].starts_with('<')
                && (end == html.len() || html[..end].ends_with('>')) =>
        {
            let mut marked = String::with_capacity(html.len() + 40);
            marked.push_str(&html[..start]);
            let _ = write!(marked, r#"<div {MARK}="1">"#);
            marked.push_str(&html[start..end]);
            marked.push_str("</div>");
            marked.push_str(&html[end..]);
            marked
        }
        _ => html.to_owned(),
    }
}

/// Where the trailing quoted block starts and ends, if there is one.
fn quoted(tree: &Tree, len: usize) -> Option<(usize, usize)> {
    let (index, element) = tree
        .elements
        .iter()
        .enumerate()
        .find(|(_, element)| is_quote(element))?;
    let previous = element.previous.map(|i| &tree.elements[i]);
    let start = match previous {
        Some(prefix)
            if (element.is("blockquote") && prefix.has_class("moz-cite-prefix"))
                || (element.has_class("zmail_extra") && prefix.has_class("zmail_extra_hr")) =>
        {
            prefix.start
        }
        _ => element.start,
    };
    let parent = element.parent.map(|i| &tree.elements[i]);
    let end = if is_outlook(element) {
        // The rest of the parent is the quoted message.
        parent.map_or(len, |parent| parent.inner_end)
    } else {
        tree.end_of(index)
    };
    if parent.is_some_and(|parent| !FLOW_PARENTS.contains(&parent.name.as_str())) {
        return None;
    }
    let before = tree.visible.iter().any(|seen| seen.at < start);
    let after = tree.visible.iter().any(|seen| seen.at >= end);
    if !before || after || tree.answered_inside(index, element, end) {
        return None;
    }
    Some((start, end))
}

fn is_outlook(element: &Element) -> bool {
    element.is("div")
        && OUTLOOK_IDS
            .iter()
            .any(|id| element.id.eq_ignore_ascii_case(id))
}

fn is_quote(element: &Element) -> bool {
    element.has_class("gmail_quote")
        || element.has_class("yahoo_quoted")
        || element.has_class("zmail_extra")
        || (element.is("blockquote") && element.kind.eq_ignore_ascii_case("cite"))
        || is_outlook(element)
}

impl Tree {
    /// Where element `index` ends: after its end tag.
    fn end_of(&self, index: usize) -> usize {
        let element = &self.elements[index];
        if VOID.contains(&element.name.as_str()) {
            return element.inner_end;
        }
        // `</name>`
        element.inner_end + element.name.len() + 3
    }

    /// Whether a wrapper (Gmail's, Yahoo's, Zoho's) holds visible text
    /// outside its quotes once the first of them has started: the member
    /// answered between the quotes.
    fn answered_inside(&self, index: usize, element: &Element, end: usize) -> bool {
        if element.is("blockquote") || is_outlook(element) {
            return false;
        }
        let Some(first) = self.elements[index + 1..]
            .iter()
            .take_while(|inner| inner.start < end)
            .find(|inner| inner.is("blockquote"))
        else {
            return false;
        };
        self.visible
            .iter()
            .any(|seen| seen.at > first.start && seen.at < end && seen.depth <= element.depth)
    }

    /// A run of text at `at`: something visible unless it is only spaces.
    fn text(&mut self, text: &str, at: usize, depth: usize, last_child: &mut [Option<usize>]) {
        if !visible(text) {
            return;
        }
        self.visible.push(Visible { at, depth });
        if let Some(slot) = last_child.last_mut() {
            *slot = None;
        }
    }
}

/// Read `html` (html5ever's serialisation) as a tree, or `None` when it is
/// not shaped as that serialisation always is.
fn read(html: &str) -> Option<Tree> {
    let mut tree = Tree::default();
    // Open elements, innermost last.
    let mut open: Vec<usize> = Vec::new();
    // The last element closed under each open element (and the root), with
    // nothing visible after it yet.
    let mut last_child: Vec<Option<usize>> = vec![None];
    let mut depth = 0;
    let mut at = 0;
    while at < html.len() {
        let Some(offset) = html[at..].find('<') else {
            tree.text(&html[at..], at, depth, &mut last_child);
            break;
        };
        if offset > 0 {
            tree.text(&html[at..at + offset], at, depth, &mut last_child);
        }
        let start = at + offset;
        let close = tag_end(html, start)?;
        let tag = &html[start + 1..close];
        at = close + 1;
        if let Some(name) = tag.strip_prefix('/') {
            // The serialiser closes the innermost open element, by its name.
            let index = open.pop()?;
            if !name.eq_ignore_ascii_case(&tree.elements[index].name) {
                return None;
            }
            last_child.pop();
            tree.elements[index].inner_end = start;
            if tree.elements[index].name == "blockquote" {
                depth = depth.saturating_sub(1);
            }
            if let Some(slot) = last_child.last_mut() {
                *slot = Some(index);
            }
            continue;
        }
        let end_of_name = tag
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let (name, attributes) = tag.split_at(end_of_name);
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        let name = name.to_ascii_lowercase();
        let index = tree.elements.len();
        let previous = last_child.last().copied().flatten();
        let element = Element {
            class: attribute(attributes, "class").to_owned(),
            id: attribute(attributes, "id").to_owned(),
            kind: attribute(attributes, "type").to_owned(),
            parent: open.last().copied(),
            previous,
            start,
            inner_end: at,
            depth,
            name,
        };
        if VOID.contains(&element.name.as_str()) {
            if element.is("img") {
                tree.visible.push(Visible { at: start, depth });
            }
            tree.elements.push(element);
            if let Some(slot) = last_child.last_mut() {
                *slot = Some(index);
            }
        } else {
            if element.is("blockquote") {
                depth += 1;
            }
            tree.elements.push(element);
            open.push(index);
            last_child.push(None);
        }
    }
    open.is_empty().then_some(tree)
}

/// Where the tag starting at `start` ends: the first `>` outside a quoted
/// attribute value, or `None` when there is none.
fn tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quoted = false;
    for (offset, byte) in html.as_bytes()[start..].iter().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b'>' if !quoted => return Some(start + offset),
            _ => {}
        }
    }
    None
}

/// Whether escaped `text` shows anything: a character other than a space
/// (`&nbsp;` included) or one that takes no room.
fn visible(text: &str) -> bool {
    text.replace("&nbsp;", " ")
        .chars()
        .any(|c| !c.is_whitespace() && !INVISIBLE.contains(&c))
}

/// The value of attribute `name` in `attributes` as html5ever writes them
/// (`name="value"`, lower-case names), or "".
fn attribute<'a>(attributes: &'a str, name: &str) -> &'a str {
    let mut rest = attributes;
    while let Some(eq) = rest.find("=\"") {
        let key = rest[..eq].trim();
        let value_start = eq + 2;
        let Some(length) = rest[value_start..].find('"') else {
            return "";
        };
        let value = &rest[value_start..value_start + length];
        if key == name {
            return value;
        }
        rest = &rest[value_start + length + 1..];
    }
    ""
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use html5ever::{
        tendril::StrTendril,
        tokenizer::{
            BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        },
    };

    use super::*;
    use crate::domain::sanitize::{InlineImages, sanitize};

    /// A start tag as a browser reads it: its name and its attributes.
    type Tag = (String, Vec<(String, String)>);

    #[derive(Default)]
    struct Tags(RefCell<Vec<Tag>>);

    impl TokenSink for Tags {
        type Handle = ();

        fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
            if let Token::TagToken(tag) = token
                && tag.kind == TagKind::StartTag
            {
                let mut attributes: Vec<(String, String)> = tag
                    .attrs
                    .iter()
                    .map(|a| (a.name.local.to_string(), a.value.to_string()))
                    .collect();
                attributes.sort();
                self.0.borrow_mut().push((tag.name.to_string(), attributes));
            }
            TokenSinkResult::Continue
        }
    }

    /// Every start tag of `html` as html5ever's tokenizer reads it, sorted.
    fn tags(html: &str) -> Vec<Tag> {
        let tokenizer = Tokenizer::new(Tags::default(), TokenizerOpts::default());
        let queue = BufferQueue::default();
        queue.push_back(StrTendril::from_slice(html));
        let _ = tokenizer.feed(&queue);
        tokenizer.end();
        let mut found = tokenizer.sink.0.take();
        found.sort();
        found
    }

    /// Marking `html` adds exactly one `div[data-cb-quoted]` or nothing, and
    /// changes no other element or attribute; the mark is never inside a value.
    fn assert_mark_adds_only_the_wrapper(html: &str) -> String {
        let marked = mark(html);
        let before = tags(html);
        let mut after = tags(&marked);
        let wrapper = (String::from("div"), vec![(MARK.to_owned(), "1".to_owned())]);
        if marked != html {
            let position = after.iter().position(|tag| *tag == wrapper);
            assert!(position.is_some(), "no wrapper in {marked}");
            if let Some(position) = position {
                after.remove(position);
            }
        }
        assert_eq!(after, before, "{marked}");
        for (_, attributes) in &after {
            for (name, value) in attributes {
                assert!(
                    name != MARK && !value.contains(MARK),
                    "the mark leaked into {name}={value:?}: {marked}"
                );
            }
        }
        marked
    }

    fn clean(html: &str) -> String {
        sanitize(html, false, &InlineImages::new()).html
    }

    const OPEN: &str = r#"<div data-cb-quoted="1">"#;

    /// `html` sanitised, from just inside the mark to the end, if marked.
    fn folded(html: &str) -> Option<String> {
        let html = clean(html);
        let start = html.find(OPEN)?;
        assert_eq!(html.matches(MARK).count(), 1, "{html}");
        Some(html[start + OPEN.len()..].to_owned())
    }

    #[test]
    fn a_gmail_reply_folds_its_quote_with_the_attribution() {
        let quote = folded(
            r#"<div dir="ltr">Sounds good, see you Friday.</div><br><div class="gmail_quote gmail_quote_container"><div dir="ltr" class="gmail_attr">On Thu, Marta Silva wrote:<br></div><blockquote class="gmail_quote" style="margin:0px 0px 0px 0.8ex">Could you send the revised quote?</blockquote></div>"#,
        )
        .expect("folded");
        assert!(
            quote.starts_with(r#"<div class="gmail_quote gmail_quote_container">"#),
            "{quote}"
        );
        assert!(quote.contains("gmail_attr"), "{quote}");
        assert!(
            quote.contains("Could you send the revised quote?"),
            "{quote}"
        );
        assert!(!quote.contains("Sounds good"), "{quote}");
    }

    #[test]
    fn the_gmail_fixture_folds_its_quote() {
        let html = clean(include_str!("../../../fixtures/email-html/gmail.html"));
        let mark = html.find(MARK).expect("marked");
        assert!(html[..mark].contains("Thanks,"), "{html}");
        assert!(html[mark..].contains("Here are the figures"), "{html}");
    }

    #[test]
    fn apple_mail_and_thunderbird_fold_their_cite_blockquote() {
        let apple = folded(
            r#"<div>Yes, oak.</div><div><br><blockquote type="cite"><div>Do you still want oak?</div></blockquote></div>"#,
        )
        .expect("apple");
        assert!(apple.starts_with(r#"<blockquote type="cite">"#), "{apple}");

        let thunderbird = folded(
            r#"<p>Yes, oak.</p><div class="moz-cite-prefix">On 14/10/2026 Sam Rivera wrote:<br></div><blockquote type="cite" cite="mid:1@example.com"><p>Do you still want oak?</p></blockquote>"#,
        )
        .expect("thunderbird");
        assert!(
            thunderbird.starts_with(r#"<div class="moz-cite-prefix">"#),
            "{thunderbird}"
        );
        assert!(
            thunderbird.contains("Do you still want oak?"),
            "{thunderbird}"
        );
    }

    #[test]
    fn yahoo_and_zoho_fold_their_wrappers() {
        let yahoo = folded(
            r#"<div>Approved.</div><div id="yahoo_quoted_1" class="yahoo_quoted"><div>On Thursday, Sam Rivera wrote:</div><div>Revised quote attached.</div></div>"#,
        )
        .expect("yahoo");
        assert!(
            yahoo.starts_with(r#"<div class="yahoo_quoted">"#),
            "{yahoo}"
        );

        let zoho = folded(
            r#"<div>Approved.</div><div class="zmail_extra_hr" style="border-top: 1px solid #e1e1e1"><br></div><div class="zmail_extra"><div>---- On Thu, Sam Rivera wrote ----</div><blockquote>Revised quote attached.</blockquote></div>"#,
        )
        .expect("zoho");
        assert!(zoho.starts_with(r#"<div class="zmail_extra_hr""#), "{zoho}");
        assert!(zoho.contains("Revised quote attached."), "{zoho}");
    }

    #[test]
    fn outlook_folds_from_its_reply_header_to_the_end_of_its_parent() {
        let quote = folded(
            r#"<div><div>Approved, thanks.</div><div id="appendonsend"></div><hr style="display:inline-block;width:98%"><div id="divRplyFwdMsg" dir="ltr"><b>From:</b> Sam Rivera</div><div>Revised quote attached.</div></div>"#,
        )
        .expect("outlook");
        assert!(
            quote.starts_with(r#"<div id="appendonsend"></div><hr"#),
            "{quote}"
        );
        assert!(
            quote.ends_with("<div>Revised quote attached.</div></div></div>"),
            "{quote}"
        );
        let html = clean(
            r#"<div>Approved.</div><div id="divRplyFwdMsg"><b>From:</b> Sam</div><div>Old.</div>"#,
        );
        assert!(
            html.starts_with("<div>Approved.</div><div data-cb-quoted"),
            "{html}"
        );
    }

    #[test]
    fn an_inline_reply_is_not_folded() {
        // Answers between the quotes.
        assert!(folded(
                r#"<blockquote type="cite">Is Thursday fine?</blockquote><div>Yes.</div><blockquote type="cite">And oak?</blockquote><div>Oak, please.</div>"#
            ).is_none());
        // The last quote is not the first: still an inline reply.
        assert!(folded(
                r#"<div>Hi,</div><blockquote type="cite">Is Thursday fine?</blockquote><div>Yes.</div><blockquote type="cite">And oak?</blockquote>"#
            ).is_none());
        // Answered inside Gmail's quote, between its blockquotes.
        assert!(folded(
                r#"<div>See below.</div><div class="gmail_quote"><div class="gmail_attr">On Thu, Marta wrote:</div><blockquote class="gmail_quote">Is Thursday fine?</blockquote><div>Yes.</div><blockquote class="gmail_quote">And oak?</blockquote></div>"#
            ).is_none());
    }

    #[test]
    fn only_a_trailing_block_with_words_before_it_is_folded() {
        // Something after it.
        assert!(
            folded(r#"<div>Hi</div><blockquote type="cite">old</blockquote><p>Sam</p>"#).is_none()
        );
        // An image after it counts.
        assert!(folded(
                r#"<div>Hi</div><blockquote type="cite">old</blockquote><img src="https://cdn.example/sig.png" width="80" height="20">"#
            ).is_none());
        // Nothing before it: a bare forward is shown whole.
        assert!(
            folded(
                r#"<div class="gmail_quote">---------- Forwarded message ---------<br>Hi</div>"#
            )
            .is_none()
        );
        // Spaces after it do not count.
        assert!(
            folded("<div>Hi</div><blockquote type=\"cite\">old</blockquote>\n&nbsp;<br>\u{200b}")
                .is_some()
        );
    }

    #[test]
    fn nested_quotes_fold_once_from_the_outermost() {
        let quote = folded(
            r#"<div>Perfect, approved.</div><blockquote type="cite"><div>Revised quote attached.</div><blockquote type="cite"><div>Yes, oak.</div><blockquote type="cite">Do you still want oak?</blockquote></blockquote></blockquote>"#,
        )
        .expect("folded");
        assert!(
            quote.starts_with(r#"<blockquote type="cite"><div>Revised"#),
            "{quote}"
        );
        assert!(quote.contains("Do you still want oak?"), "{quote}");
    }

    #[test]
    fn a_block_where_a_div_cannot_stand_is_not_folded() {
        assert!(folded(r#"<p>Hi <span class="gmail_quote">old</span></p>"#).is_none());
        assert!(folded(r#"<ul><li>Hi</li><li class="gmail_quote">old</li></ul>"#).is_none());
    }

    #[test]
    fn a_sender_cannot_write_the_mark_or_spoof_a_quote() {
        let html = clean(
            r#"<div data-cb-quoted="1">Pay this invoice today.</div><div class="data-cb-quoted">x</div><blockquote type="note">not a cite</blockquote><div id="divReply">not Outlook</div>"#,
        );
        // The words may stay as a class name; the reader reads only the attribute.
        assert!(!html.contains(&format!("{MARK}=")), "{html}");
        assert!(!html.contains(r#"type="note""#), "{html}");
        assert!(!html.contains("divReply"), "{html}");
        assert!(html.contains("Pay this invoice today."), "{html}");
        // Only the structural attributes the fold reads survive, and only
        // with the values it reads.
        let kept = clean(
            r#"<div>a</div><blockquote type="CITE">b</blockquote><div id="appendonsend"></div><div id="other">c</div>"#,
        );
        assert!(kept.contains(r#"<blockquote type="CITE">"#), "{kept}");
        assert!(kept.contains(r#"<div id="appendonsend">"#), "{kept}");
        assert!(!kept.contains(r#"id="other""#), "{kept}");
    }

    #[test]
    fn words_never_decide() {
        // An attribution line without the markup is just words.
        assert!(
            folded("<div>Approved.</div><div>On Thu, Sam Rivera wrote:</div><div>&gt; old</div>")
                .is_none()
        );
    }

    /// A `>` or `<` inside a value never ends a tag or starts one, even if a
    /// serialiser wrote them raw (the one in use escapes them).
    #[test]
    fn a_tag_ends_only_outside_its_quoted_values() {
        let payload = r#"<p>Hi</p><span title="x><img/src=https://t.example/p.gif" class="gmail_quote">Q</span>"#;
        assert_eq!(
            assert_mark_adds_only_the_wrapper(payload),
            r#"<p>Hi</p><div data-cb-quoted="1"><span title="x><img/src=https://t.example/p.gif" class="gmail_quote">Q</span></div>"#
        );
        // A quote written inside a value is not one.
        let lookalike = r#"<p>Hi</p><p title="a>b<blockquote type=&quot;cite&quot;>">x</p>"#;
        assert_eq!(assert_mark_adds_only_the_wrapper(lookalike), lookalike);
        // An end tag inside a value does not close anything.
        assert_eq!(
            assert_mark_adds_only_the_wrapper(
                r#"<div>Hi</div><div class="gmail_quote"><div title="x</div>y">old</div></div>"#
            ),
            r#"<div>Hi</div><div data-cb-quoted="1"><div class="gmail_quote"><div title="x</div>y">old</div></div></div>"#
        );
    }

    #[test]
    fn html_not_shaped_as_the_serialisation_is_left_alone() {
        for odd in [
            // A tag that never ends, a value that never closes.
            r#"<div>Hi</div><div class="gmail_quote""#,
            r#"<div>Hi</div><div class="gmail_quote>old</div>"#,
            // An end tag that closes the wrong element, or none.
            r#"<div>Hi</div><blockquote type="cite">old</div>"#,
            r#"<div>Hi</div></span><blockquote type="cite">old</blockquote>"#,
            // A name that is not a plain tag name.
            r#"<div>Hi</div><img/src="x" class="gmail_quote"><blockquote type="cite">old</blockquote><di!v>"#,
            // An element left open.
            r#"<div>Hi</div><blockquote type="cite">old"#,
        ] {
            assert_eq!(mark(odd), odd, "{odd}");
        }
    }

    /// Whatever a sender writes, the sanitised output gains at most the one
    /// wrapper, with every other element and attribute as it was.
    #[test]
    fn hostile_bodies_gain_only_the_wrapper() {
        for body in [
            r#"<p>Hi</p><span title='x><img/src=https://t.example/p.gif' class='gmail_quote'>Q</span>"#,
            r#"<p>Hi</p><p title='a>b<blockquote type="cite">'>x</p><blockquote type="cite">old</blockquote>"#,
            r#"<div>Hi</div><div class="gmail_quote"><div title="x</div>y">old</div></div>"#,
            r#"<div>Hi</div><div class="gmail_quote" data-cb-quoted="1">old</div>"#,
            r#"<p>Hi</p><a href="https://example.com/?a=<b>&c=>" class="gmail_quote">x</a>"#,
            "<p>Hi<blockquote type=cite>unclosed",
            r#"<table><tr><td>Hi</td></tr></table><div class="yahoo_quoted"><img src="https://t.example/a.png" alt=">"></div>"#,
        ] {
            for show in [false, true] {
                let html = sanitize(body, show, &InlineImages::new()).html;
                let wrappers = tags(&html)
                    .iter()
                    .filter(|(_, attributes)| attributes.iter().any(|(name, _)| name == MARK))
                    .count();
                assert!(wrappers <= 1, "{html}");
                for (_, attributes) in tags(&html) {
                    for (name, value) in attributes {
                        assert!(!value.contains(MARK), "{name}={value:?} in {html}");
                    }
                }
                // Marking again finds the same block or none, and adds nothing else.
                let _ = assert_mark_adds_only_the_wrapper(&html.replace(OPEN, "<div>"));
            }
        }
    }

    #[test]
    fn attributes_are_read_as_the_serialiser_writes_them() {
        assert_eq!(
            attribute(r#"dir="ltr" class="gmail_quote x" style="a: b""#, "class"),
            "gmail_quote x"
        );
        assert_eq!(attribute(r#"title="a=&quot;b&quot;" id="x""#, "id"), "x");
        assert_eq!(attribute("", "class"), "");
    }
}
