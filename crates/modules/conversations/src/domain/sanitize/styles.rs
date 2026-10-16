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
//! nodes nested deeper than [`MAX_NESTING`] (the inliner's serialiser
//! recurses once per level, so a deeper tree would overflow the stack),
//! more than [`MAX_ELEMENTS`] elements, more than [`MAX_WORK`] elements
//! times distinct properties (the size of what it could write), more than
//! [`MAX_MATCHING`] selectors times elements (the matching), or more than
//! [`MAX_APPLYING`] declarations times that work (applying them).
//!
//! Depth and element count come from the tree html5ever's tree builder
//! makes ([`shape`]), the same parse the inliner runs, never from counting
//! tags: stray end tags and foreign content (`<svg><xmp>`) make the tags
//! and the tree disagree.

use std::{
    borrow::Cow,
    cell::{Ref, RefCell},
    sync::Arc,
};

use super::css;
use css_inline::{CSSInliner, InlineError, StylesheetResolver};
use html5ever::{
    Attribute, ParseOpts, QualName, local_name, ns, parse_document,
    tendril::{StrTendril, TendrilSink},
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        states::RawKind,
    },
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
};

/// Most bytes of `<style>` text inlined.
pub const MAX_STYLE_BYTES: usize = 256 * 1024;
/// Deepest element nesting inlined.
pub const MAX_NESTING: usize = 256;
/// Most elements inlined.
pub const MAX_ELEMENTS: usize = 20_000;
/// Most elements times distinct properties inlined.
pub const MAX_WORK: usize = 1_000_000;
/// Most selectors times elements inlined.
pub const MAX_MATCHING: usize = 50_000_000;
/// Most declarations (per matched element) times [`MAX_WORK`]'s measure
/// inlined: applying a declaration looks through the element's properties.
pub const MAX_APPLYING: usize = 500_000_000;

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
    if found.styles.trim().is_empty() || found.styles.len() > MAX_STYLE_BYTES {
        return unchanged(None);
    }
    let tree = shape(html);
    if tree.deepest > MAX_NESTING || tree.elements > MAX_ELEMENTS {
        return unchanged(None);
    }
    let media = css::media_rules(&found.styles);
    let rules = css::plain_rules(&found.styles);
    let work = rules.properties.saturating_mul(tree.elements);
    if rules.css.is_empty()
        || work > MAX_WORK
        || rules.selectors.saturating_mul(tree.elements) > MAX_MATCHING
        || rules.declarations.saturating_mul(work) > MAX_APPLYING
    {
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

/// The shape of the tree html5ever builds for a document.
#[derive(Debug, Default, PartialEq, Eq)]
struct Shape {
    /// Elements created.
    elements: usize,
    /// The deepest node, the document's children at depth 1.
    deepest: usize,
}

/// Parse `html` as the inliner does (a whole document) and measure the tree
/// it makes, walking it with an explicit stack: nothing here recurses.
fn shape(html: &str) -> Shape {
    parse_document(Measure::default(), ParseOpts::default()).one(html)
}

/// A node of the measured tree: its name (empty unless it is an element)
/// and its links. Text is never kept: it is a leaf and adds no depth.
struct Node {
    name: QualName,
    parent: Option<usize>,
    previous: Option<usize>,
    next: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
}

/// A tree sink that keeps only the tree's links (node 0 is the document).
struct Measure {
    nodes: RefCell<Vec<Node>>,
    elements: RefCell<usize>,
}

impl Default for Measure {
    fn default() -> Self {
        Self {
            nodes: RefCell::new(vec![Node::new(unnamed())]),
            elements: RefCell::new(0),
        }
    }
}

/// The name of a node that is not an element.
fn unnamed() -> QualName {
    QualName::new(None, ns!(), local_name!(""))
}

impl Node {
    fn new(name: QualName) -> Self {
        Self {
            name,
            parent: None,
            previous: None,
            next: None,
            first: None,
            last: None,
        }
    }
}

impl Measure {
    fn push(&self, name: QualName) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(Node::new(name));
        nodes.len() - 1
    }

    fn detach(nodes: &mut [Node], node: usize) {
        let (parent, previous, next) = {
            let n = &nodes[node];
            (n.parent, n.previous, n.next)
        };
        let Some(parent) = parent else { return };
        match previous {
            Some(previous) => nodes[previous].next = next,
            None => nodes[parent].first = next,
        }
        match next {
            Some(next) => nodes[next].previous = previous,
            None => nodes[parent].last = previous,
        }
        let n = &mut nodes[node];
        n.parent = None;
        n.previous = None;
        n.next = None;
    }

    fn append_node(nodes: &mut [Node], parent: usize, child: usize) {
        Self::detach(nodes, child);
        let last = nodes[parent].last;
        {
            let c = &mut nodes[child];
            c.parent = Some(parent);
            c.previous = last;
        }
        match last {
            Some(last) => nodes[last].next = Some(child),
            None => nodes[parent].first = Some(child),
        }
        nodes[parent].last = Some(child);
    }

    fn insert_before(nodes: &mut [Node], sibling: usize, child: usize) {
        Self::detach(nodes, child);
        let Some(parent) = nodes[sibling].parent else {
            return;
        };
        let previous = nodes[sibling].previous;
        {
            let c = &mut nodes[child];
            c.parent = Some(parent);
            c.previous = previous;
            c.next = Some(sibling);
        }
        match previous {
            Some(previous) => nodes[previous].next = Some(child),
            None => nodes[parent].first = Some(child),
        }
        nodes[sibling].previous = Some(child);
    }
}

impl TreeSink for Measure {
    type Handle = usize;
    type Output = Shape;
    type ElemName<'a>
        = Ref<'a, QualName>
    where
        Self: 'a;

    fn finish(self) -> Shape {
        let nodes = self.nodes.into_inner();
        let mut deepest = 0;
        let mut stack = vec![(0_usize, 0_usize)];
        while let Some((node, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            let mut child = nodes[node].first;
            while let Some(c) = child {
                stack.push((c, depth + 1));
                child = nodes[c].next;
            }
        }
        Shape {
            elements: self.elements.into_inner(),
            deepest,
        }
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> usize {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> Self::ElemName<'a> {
        Ref::map(self.nodes.borrow(), |nodes| &nodes[*target].name)
    }

    fn create_element(
        &self,
        name: QualName,
        _attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> usize {
        *self.elements.borrow_mut() += 1;
        self.push(name)
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.push(unnamed())
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.push(unnamed())
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        if let NodeOrText::AppendNode(child) = child {
            Self::append_node(&mut self.nodes.borrow_mut(), *parent, child);
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        if self.nodes.borrow()[*element].parent.is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    // The inliner keeps a template's contents as its children, so do we.
    fn get_template_contents(&self, target: &usize) -> usize {
        *target
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &usize, new_node: NodeOrText<usize>) {
        if let NodeOrText::AppendNode(child) = new_node {
            Self::insert_before(&mut self.nodes.borrow_mut(), *sibling, child);
        }
    }

    fn add_attrs_if_missing(&self, _target: &usize, _attrs: Vec<Attribute>) {}

    fn remove_from_parent(&self, target: &usize) {
        Self::detach(&mut self.nodes.borrow_mut(), *target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let mut nodes = self.nodes.borrow_mut();
        while let Some(child) = nodes[*node].first {
            Self::append_node(&mut nodes, *new_parent, child);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

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
    fn depth_is_the_tree_s_not_the_tags() {
        // Stray end tags the tree builder ignores: the tags balance out,
        // the tree is thousands deep.
        let stray = format!(
            "<style>p {{ color: red }}</style>{}<p>x</p>",
            "<div></span>".repeat(50_000)
        );
        assert!(shape(&stray).deepest > MAX_NESTING);
        assert_eq!(inline(&stray).html, stray);
        // Foreign content: in `<svg>`, `<xmp>` is an element, not raw text.
        let foreign = format!(
            "<style>p {{ color: red }}</style><svg><xmp>{}</svg><p>x</p>",
            "<g>".repeat(MAX_NESTING + 10)
        );
        assert!(shape(&foreign).deepest > MAX_NESTING);
        assert_eq!(inline(&foreign).html, foreign);
        // A plain document: html, body, p.
        assert_eq!(
            shape("<p>x</p>"),
            Shape {
                elements: 4,
                deepest: 3
            }
        );
    }

    #[test]
    fn matching_past_the_cap_is_not_inlined() {
        let html = format!(
            "<style>{}</style>{}",
            "p{a:b}".repeat(20_000),
            "<b></b>".repeat(3_000)
        );
        assert_eq!(inline(&html).html, html);
    }

    #[test]
    fn applying_past_the_cap_is_not_inlined() {
        let properties = (0..256).fold(String::new(), |mut all, i| {
            let _ = write!(all, "a{i}:x;");
            all
        });
        let html = format!(
            "<style>{}</style>{}",
            format!("*{{{properties}}}").repeat(100),
            "<b></b>".repeat(3_000)
        );
        assert_eq!(inline(&html).html, html);
    }

    #[test]
    fn work_past_the_cap_is_not_inlined() {
        let properties = (0..200).fold(String::new(), |mut all, i| {
            let _ = write!(all, "a{i}: x;");
            all
        });
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
