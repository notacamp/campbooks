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
//! makes ([`measure`]), the same parse the inliner runs, never from counting
//! tags: stray end tags and foreign content (`<svg><xmp>`) make the tags
//! and the tree disagree.
//!
//! [`measure`] is also the gate for everything that parses the HTML after
//! it (the inliner, ammonia): the tree builder's time grows with the depth
//! of its stack of open elements times the tags it reads, so HTML built for
//! it takes minutes. The measure counts that work in its tree sink, as the
//! later parses will do it on the same tokens and in the same modes (a
//! whole document for the inliner, a fragment in a `div` for ammonia), and
//! gives up past
//! [`MAX_PARSE_WORK`] units or [`MAX_PARSE_DEPTH`] levels. A parse cannot be
//! cancelled, so the HTML is fed a chunk at a time and the count is read
//! between chunks: a message built to be slow costs about one chunk past the
//! budget.

use std::{
    borrow::Cow,
    cell::{Cell, Ref, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use super::css;
use css_inline::{CSSInliner, InlineError, StylesheetResolver};
use html5ever::{
    Attribute, ParseOpts, Parser, QualName, local_name, ns, parse_document, parse_fragment,
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
/// Most tree-builder work the measure allows (see [`measure`]): about seven
/// times what a dense 2 MiB newsletter costs (the reader reads no more), and
/// a few tens of milliseconds of parsing.
pub const MAX_PARSE_WORK: usize = 10_000_000;
/// Deepest tree the measure allows.
pub const MAX_PARSE_DEPTH: usize = 4_096;
/// The most time measuring may take, for both parses together: about three
/// times what the densest ordinary 2 MiB mail needs on the production CPU.
/// It covers work the tree builder and the tokenizer do without calling the
/// sink (the active formatting list's duplicate check, a start tag's
/// attributes), which the work count cannot see.
pub const MAX_PARSE_TIME: Duration = Duration::from_millis(500);
/// Bytes of HTML fed to the measure between two reads of its work count and
/// the clock: small, so what one huge tag costs between two checks stays
/// small.
const CHUNK: usize = 1024;

/// What a measuring parse unwinds with when it is over time.
struct OverBudget;

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

/// Inline the style sheets of `html`, whose measured tree is `tree`.
pub fn inline(html: &str, tree: &Shape) -> Inlined {
    let found = scan(html);
    let unchanged = |media: Option<String>| Inlined {
        html: html.to_owned(),
        body: found.body.clone(),
        media,
    };
    if found.styles.trim().is_empty() || found.styles.len() > MAX_STYLE_BYTES {
        return unchanged(None);
    }
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

/// The shape of the tree html5ever builds for a document, and the work it
/// took to build it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// Elements created.
    pub elements: usize,
    /// The deepest node, the document's children at depth 1.
    pub deepest: usize,
    /// Tree-builder work, in the units of [`MAX_PARSE_WORK`].
    pub work: usize,
}

/// Parse `html` as the inliner and ammonia do, counting the tree builder's
/// work, and measure the tree, walking it with an explicit stack: nothing
/// here recurses. `None` when the work passes [`MAX_PARSE_WORK`] or the tree
/// is deeper than [`MAX_PARSE_DEPTH`].
///
/// The later parses run the same algorithm on the same tokens, so what this
/// counts is what they would pay, as long as it parses the way each of them
/// does: the inliner parses a whole document, ammonia a fragment in a `div`
/// (its `make_parser`), and the two can differ wholesale (a leading
/// `<frameset>` makes a document drop everything after it, while a fragment
/// reads it all). So both are measured, and both must stay within the
/// budget; the shape is the document's, which is what the inliner sees.
pub fn measure(html: &str) -> Option<Shape> {
    measure_within(html, MAX_PARSE_TIME)
}

/// [`measure`] with its own time allowance (tests on slow machines).
fn measure_within(html: &str, allowance: Duration) -> Option<Shape> {
    let deadline = Instant::now() + allowance;
    let fragment = measure_with(html, deadline, |sink| {
        parse_fragment(
            sink,
            ParseOpts::default(),
            QualName::new(None, ns!(html), local_name!("div")),
            vec![],
            false,
        )
    })?;
    let document = measure_with(html, deadline, |sink| {
        parse_document(sink, ParseOpts::default())
    })?;
    Some(Shape {
        work: document.work.max(fragment.work),
        ..document
    })
}

/// One measured parse of `html` by the parser `start` makes, within
/// `deadline`. The HTML is fed in chunks of at most 1 KiB and the count and
/// the clock are checked after each, because a running parse cannot be
/// stopped from outside: a message built to be slow costs about one chunk
/// past the budget instead of minutes. Inside a chunk the sink checks the
/// clock too, and unwinds when it is over: that is the only way out of a
/// parse that is running, and the parser is dropped with the closure.
fn measure_with(
    html: &str,
    deadline: Instant,
    start: impl FnOnce(Measure) -> Parser<Measure>,
) -> Option<Shape> {
    let work = Rc::new(Cell::new(0));
    let measured = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut parser = start(Measure::new(Rc::clone(&work), deadline));
        let mut rest = html;
        while !rest.is_empty() {
            let mut end = rest.len().min(CHUNK);
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            let (chunk, tail) = rest.split_at(end);
            parser.process(StrTendril::from_slice(chunk));
            if work.get() > MAX_PARSE_WORK || Instant::now() > deadline {
                return None;
            }
            rest = tail;
        }
        Some(parser.finish())
    }));
    match measured {
        Ok(shape) => shape.filter(|s| s.work <= MAX_PARSE_WORK && s.deepest <= MAX_PARSE_DEPTH),
        Err(payload) if payload.is::<OverBudget>() => None,
        Err(payload) => std::panic::resume_unwind(payload),
    }
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
    /// How many children it has, kept exact.
    children: usize,
    /// How many attributes it has.
    attrs: usize,
}

/// A tree sink that keeps only the tree's links (node 0 is the document).
struct Measure {
    nodes: RefCell<Vec<Node>>,
    elements: RefCell<usize>,
    /// Work counted so far, shared with [`measure`], which reads it between
    /// chunks while the parser owns the sink.
    work: Rc<Cell<usize>>,
    /// When measuring must stop, checked every 1,024 sink calls.
    deadline: Instant,
    calls: Cell<u32>,
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
            children: 0,
            attrs: 0,
        }
    }
}

impl Measure {
    fn new(work: Rc<Cell<usize>>, deadline: Instant) -> Self {
        Self {
            nodes: RefCell::new(vec![Node::new(unnamed())]),
            elements: RefCell::new(0),
            work,
            deadline,
            calls: Cell::new(0),
        }
    }

    /// Count `units` of work. Every sink method calls this first, before it
    /// borrows anything, so the unwind below leaves no borrow behind.
    fn charge(&self, units: usize) {
        self.work.set(self.work.get().saturating_add(units));
        let calls = self.calls.get().wrapping_add(1);
        self.calls.set(calls);
        if calls % 1024 == 0 && Instant::now() > self.deadline {
            std::panic::resume_unwind(Box::new(OverBudget));
        }
    }

    /// The children of the parent of `node` (none for a root).
    fn siblings(&self, node: usize) -> usize {
        let nodes = self.nodes.borrow();
        nodes[node]
            .parent
            .map_or(0, |parent| nodes[parent].children)
    }

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
        nodes[parent].children -= 1;
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
        nodes[parent].children += 1;
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
        nodes[parent].children += 1;
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
            work: self.work.get(),
        }
    }

    // Every call is a unit. The tree builder's quadratic paths all go
    // through the sink: `elem_name` is what each scope check calls for every
    // open element it passes, and foster parenting and the adoption agency
    // move nodes (`append_before_sibling`, `remove_from_parent`,
    // `reparent_children`), weighted here as ammonia's RcDom pays for them
    // (it searches the parent's child list).

    fn parse_error(&self, _msg: Cow<'static, str>) {
        self.charge(1);
    }

    fn get_document(&self) -> usize {
        self.charge(1);
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> Self::ElemName<'a> {
        self.charge(1);
        Ref::map(self.nodes.borrow(), |nodes| &nodes[*target].name)
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _flags: ElementFlags) -> usize {
        self.charge(1);
        self.charge(attrs.len());
        *self.elements.borrow_mut() += 1;
        let node = self.push(name);
        self.nodes.borrow_mut()[node].attrs = attrs.len();
        node
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.charge(1);
        self.push(unnamed())
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.charge(1);
        self.push(unnamed())
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        self.charge(1);
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
        self.charge(1);
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
        self.charge(1);
    }

    // The inliner keeps a template's contents as its children, so do we.
    fn get_template_contents(&self, target: &usize) -> usize {
        self.charge(1);
        *target
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        self.charge(1);
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {
        self.charge(1);
    }

    fn append_before_sibling(&self, sibling: &usize, new_node: NodeOrText<usize>) {
        self.charge(1);
        self.charge(self.siblings(*sibling));
        if let NodeOrText::AppendNode(child) = new_node {
            Self::insert_before(&mut self.nodes.borrow_mut(), *sibling, child);
        }
    }

    // RcDom rebuilds a set of the element's existing attribute names on
    // every call, so a repeated `<html a1>` costs the attributes so far.
    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        self.charge(1);
        let existing = self.nodes.borrow()[*target].attrs;
        self.charge(existing.saturating_add(attrs.len()));
        self.nodes.borrow_mut()[*target].attrs = existing.saturating_add(attrs.len());
    }

    fn remove_from_parent(&self, target: &usize) {
        self.charge(1);
        self.charge(self.siblings(*target));
        Self::detach(&mut self.nodes.borrow_mut(), *target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        self.charge(1);
        let moved = self.nodes.borrow()[*node].children;
        self.charge(moved);
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

    /// `super::inline` on the HTML's own measured shape.
    fn inline(html: &str) -> Inlined {
        super::inline(html, &accepted(html).expect("within the parse budget"))
    }

    /// `measure` with minutes of time, for slow test machines: the work
    /// budget and depth cap still apply.
    fn accepted(html: &str) -> Option<Shape> {
        measure_within(html, Duration::from_secs(60))
    }

    const FIXTURES: [&str; 7] = [
        include_str!("../../../fixtures/email-html/newsletter.html"),
        include_str!("../../../fixtures/email-html/receipt.html"),
        include_str!("../../../fixtures/email-html/outlook.html"),
        include_str!("../../../fixtures/email-html/gmail.html"),
        include_str!("../../../fixtures/email-html/marketing-600.html"),
        include_str!("../../../fixtures/email-html/marketing-fluid.html"),
        include_str!("../../../fixtures/email-html/hostile.html"),
    ];

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
        // the tree is past the cap. Just past it: the tree builder's work
        // grows with depth squared, so thousands deep takes minutes here.
        let stray = format!(
            "<style>p {{ color: red }}</style>{}<p>x</p>",
            "<div></span>".repeat(MAX_NESTING + 10)
        );
        assert!(accepted(&stray).expect("cheap").deepest > MAX_NESTING);
        assert_eq!(inline(&stray).html, stray);
        // Foreign content: in `<svg>`, `<xmp>` is an element, not raw text.
        let foreign = format!(
            "<style>p {{ color: red }}</style><svg><xmp>{}</svg><p>x</p>",
            "<g>".repeat(MAX_NESTING + 10)
        );
        assert!(accepted(&foreign).expect("cheap").deepest > MAX_NESTING);
        assert_eq!(inline(&foreign).html, foreign);
        // A plain document: html, body, p.
        let plain = accepted("<p>x</p>").expect("cheap");
        assert_eq!((plain.elements, plain.deepest), (4, 3));
    }

    #[test]
    fn measuring_a_deep_tree_does_not_recurse() {
        // The walk keeps its own stack: a tree 100,000 deep is measured on a
        // 64 KiB thread. (Built by hand: parsing one that deep takes hours.)
        let deepest = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let count = 100_000;
                let nodes = (0..count)
                    .map(|i| {
                        let mut node = Node::new(unnamed());
                        node.first = (i + 1 < count).then_some(i + 1);
                        node
                    })
                    .collect();
                let tree = Measure {
                    nodes: RefCell::new(nodes),
                    elements: RefCell::new(0),
                    work: Rc::new(Cell::new(0)),
                };
                tree.finish().deepest
            })
            .expect("spawn")
            .join()
            .expect("measured without overflowing the stack");
        assert_eq!(deepest, 99_999);
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

    #[test]
    fn html_built_to_be_slow_is_refused() {
        // Minutes of tree building in full; the budget stops each within a
        // chunk or so.
        assert!(measure(&"<div></span>".repeat(200_000)).is_none());
        assert!(measure(&"<div>".repeat(200_000)).is_none());
        assert!(measure(&"<a><table><a>x".repeat(50_000)).is_none());
    }

    #[test]
    fn a_leading_frameset_does_not_hide_the_work() {
        // A document drops what follows a leading `<frameset>`; ammonia's
        // fragment parse reads it all, so the fragment is measured too.
        assert!(measure(&format!("<frameset>{}", "<div></span>".repeat(200_000))).is_none());
        assert!(measure(&format!("<frameset>{}", "<div>".repeat(200_000))).is_none());
        assert!(accepted("<frameset><p>x</p>").is_some());
    }

    #[test]
    fn ordinary_mail_is_far_under_the_budget() {
        for html in FIXTURES {
            let shape = accepted(html).expect("within the budget");
            assert!(shape.work < MAX_PARSE_WORK / 10, "{}", shape.work);
        }
    }

    #[test]
    fn a_large_newsletter_has_room() {
        let block = r#"<table width="600" cellpadding="0"><tr><td class="a"><a href="https://t.example/x"><img src="https://t.example/i.png" width="100"></a><h2>Title</h2><p>Some <b>bold</b> words and more words.</p><ul><li>One</li><li>Two</li></ul></td></tr></table>"#;
        let html = block.repeat(3_000);
        assert!(html.len() > 700 * 1024);
        let html = format!("{html}{html}");
        let shape = accepted(&html).expect("within the budget");
        assert!(shape.work < MAX_PARSE_WORK / 4, "{}", shape.work);
    }

    #[test]
    fn depth_past_the_cap_is_refused() {
        assert!(measure(&"<font color=red>x".repeat(5_000)).is_none());
        assert!(accepted(&"<font color=red>x".repeat(3_000)).is_some());
    }

    #[test]
    fn work_the_sink_cannot_see_is_refused() {
        // Attributes added to the same element again and again.
        let repeated = (0..20_000)
            .map(|i| format!("<html a{i}>"))
            .collect::<String>();
        assert!(measure(&repeated).is_none());
        // The active formatting list's duplicate check.
        let formatting = (0..20_000)
            .map(|i| format!("<b id={i}>"))
            .collect::<String>();
        assert!(measure(&formatting).is_none());
        // Many open elements, then end tags that scan them.
        let strays = "<span>".repeat(100_000) + &"</x>".repeat(2_048);
        assert!(measure(&strays).is_none());
        // One start tag with 60,000 attributes.
        let many = (0..60_000).map(|i| format!("a{i} ")).collect::<String>();
        assert!(measure(&format!("<p {many}>x</p>")).is_none());
        // Deep spans, then templates left open.
        let templates = "<span>".repeat(20_000) + &"<template>".repeat(10_000);
        assert!(measure(&templates).is_none());
    }
}
