//! A page's body, read as the neutral blocks Notion's are mapped to
//! (campbooks-cloud#376).
//!
//! Structural, element to block: a `paragraph` is a paragraph, a `listItem`
//! of a `bulletList` is a bulleted item, a `toggle` is a toggle whose first
//! paragraph is its own text and whose other blocks are the blocks under
//! it. A `conflict` is not a block: it is kept apart, with Notion's version
//! it holds. Text is read as runs (bold, italic, strike, code, links); a
//! mention is read in the form Notion would have it: another synced page as
//! a page mention, anything else of Campbooks as the words "Campbooks link"
//! with the app's address of it, **never** its subject, name or text.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use uuid::Uuid;
use yrs::{
    Any, Out, ReadTxn, Text, Xml, XmlElementRef, XmlFragment, XmlOut, XmlTextRef,
    types::text::YChange,
};

use super::model::{Block, FileRef, Kind, Run, hash_of_hex, normalize};

/// The words a Campbooks record is named by at Notion.
pub const CAMPBOOKS_LINK: &str = "Campbooks link";

/// What reading a body needs to know of the rest of the workspace.
#[derive(Debug, Clone, Default)]
pub struct Resolver {
    /// The pages linked to Notion pages: the page, and its Notion page.
    pub notion_of: HashMap<Uuid, Uuid>,
    /// The same, the other way round.
    pub page_of: HashMap<Uuid, Uuid>,
    /// The address of the workspace in the app, without a closing `/`
    /// (`https://…/w/<workspace>`); empty when the app has none.
    pub app: String,
    /// The page being read.
    pub this: Uuid,
}

impl Resolver {
    /// The address of a record in the app, when it has one a link can name.
    fn link(&self, ref_type: &str, ref_id: &str) -> Option<String> {
        if self.app.is_empty() {
            return None;
        }
        let path = match ref_type {
            "conversations.conversation" => format!("search/{ref_id}"),
            "tasks.task" => format!("tasks/{ref_id}"),
            "schedule.event" => "schedule".to_owned(),
            "pages.page" => format!("pages/{ref_id}"),
            // A person, a member: never named by their address; the page
            // that mentions them is the link.
            _ => format!("pages/{}", self.this),
        };
        let url = format!("{}/{path}", self.app);
        let lower = url.to_ascii_lowercase();
        (lower.starts_with("https://") || lower.starts_with("http://")).then_some(url)
    }

    /// The address of the page being read in the app, when the app has one.
    pub fn page_link(&self) -> Option<String> {
        self.link("pages.page", &self.this.to_string())
    }

    /// A mention, in the form Notion has it.
    fn mention(&self, ref_type: &str, ref_id: &str) -> Run {
        if ref_type == "pages.page"
            && let Some(notion) = Uuid::parse_str(ref_id)
                .ok()
                .and_then(|page| self.notion_of.get(&page))
        {
            return Run {
                page: Some(*notion),
                ..Run::default()
            };
        }
        Run {
            text: CAMPBOOKS_LINK.to_owned(),
            link: self.link(ref_type, ref_id),
            ..Run::default()
        }
    }
}

/// A block of the body, with the blocks under it.
#[derive(Debug, Clone)]
pub struct Node {
    /// The block's stable id in the body, when it has one.
    pub bid: Option<Uuid>,
    /// The block's content (its `children` are left empty: they are below).
    pub block: Block,
    /// The blocks under it.
    pub children: Vec<Node>,
}

impl Node {
    /// This block and every block under it, in document order.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Node>) {
        out.push(self);
        for child in &self.children {
            child.walk(out);
        }
    }
}

/// A conflict the body holds.
#[derive(Debug, Clone)]
pub struct ConflictNode {
    /// The block it is about.
    pub of: Uuid,
    /// The hash of Notion's version.
    pub hash: Option<[u8; 32]>,
    /// When Notion last edited its version.
    pub edited: Option<DateTime<Utc>>,
    /// Notion removed the block.
    pub removed: bool,
    /// Notion's version.
    pub theirs: Option<Block>,
}

/// A body, read.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// The blocks, in order.
    pub nodes: Vec<Node>,
    /// The conflicts.
    pub conflicts: Vec<ConflictNode>,
}

impl Tree {
    /// Every block, in document order.
    pub fn all(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        for node in &self.nodes {
            node.walk(&mut out);
        }
        out
    }

    /// The block with stable id `bid`.
    pub fn find(&self, bid: Uuid) -> Option<&Node> {
        self.all().into_iter().find(|node| node.bid == Some(bid))
    }

    /// The conflict about block `bid`.
    pub fn conflict_of(&self, bid: Uuid) -> Option<&ConflictNode> {
        self.conflicts.iter().find(|conflict| conflict.of == bid)
    }
}

/// An element's attributes that hold a value.
pub fn attributes<T: ReadTxn>(txn: &T, element: &XmlElementRef) -> BTreeMap<String, Any> {
    element
        .attributes(txn)
        .filter_map(|(key, value)| match value {
            Out::Any(any) => Some((key.to_owned(), any)),
            _ => None,
        })
        .collect()
}

/// The text an attribute holds.
pub fn text_attribute(attributes: &BTreeMap<String, Any>, key: &str) -> Option<String> {
    match attributes.get(key) {
        Some(Any::String(text)) => Some(text.to_string()),
        _ => None,
    }
}

/// The identifier an attribute holds.
pub fn uuid_attribute(attributes: &BTreeMap<String, Any>, key: &str) -> Option<Uuid> {
    text_attribute(attributes, key).and_then(|text| Uuid::parse_str(&text).ok())
}

fn bool_attribute(attributes: &BTreeMap<String, Any>, key: &str) -> Option<bool> {
    match attributes.get(key) {
        Some(Any::Bool(value)) => Some(*value),
        _ => None,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a small whole number the schema already bounded"
)]
fn number_attribute(attributes: &BTreeMap<String, Any>, key: &str) -> Option<u64> {
    match attributes.get(key) {
        Some(Any::Number(number)) if number.is_finite() && *number >= 0.0 => Some(*number as u64),
        Some(Any::BigInt(number)) => u64::try_from(*number).ok(),
        _ => None,
    }
}

/// Read the blocks under `parent` (the document's body, or an element).
pub fn read<T: ReadTxn>(txn: &T, parent: &impl XmlFragment, resolver: &Resolver) -> Tree {
    let mut tree = Tree::default();
    let children: Vec<XmlOut> = parent.children(txn).collect();
    tree.nodes = blocks(txn, &children, resolver, &mut tree.conflicts);
    tree
}

fn blocks<T: ReadTxn>(
    txn: &T,
    children: &[XmlOut],
    resolver: &Resolver,
    conflicts: &mut Vec<ConflictNode>,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    for child in children {
        if let XmlOut::Element(element) = child {
            nodes.extend(element_nodes(txn, element, resolver, conflicts));
        }
    }
    nodes
}

#[allow(
    clippy::too_many_lines,
    reason = "one match over every element of the schema"
)]
fn element_nodes<T: ReadTxn>(
    txn: &T,
    element: &XmlElementRef,
    resolver: &Resolver,
    conflicts: &mut Vec<ConflictNode>,
) -> Vec<Node> {
    let tag: &str = element.tag();
    let attrs = attributes(txn, element);
    let bid = uuid_attribute(&attrs, "bid");
    let leaf = |block: Block| {
        vec![Node {
            bid,
            block,
            children: Vec::new(),
        }]
    };
    let blank = || Block::new(bid.unwrap_or_default(), Kind::External);
    match tag {
        "paragraph" => {
            let mut block = blank();
            block.kind = Kind::Paragraph;
            block.runs = inline(txn, element, resolver);
            leaf(block)
        }
        "heading" => {
            let mut block = blank();
            block.kind = Kind::Heading;
            block.level = number_attribute(&attrs, "level")
                .and_then(|level| u8::try_from(level).ok())
                .filter(|level| (1..=3).contains(level))
                .or(Some(1));
            block.runs = inline(txn, element, resolver);
            leaf(block)
        }
        "codeBlock" => {
            let mut block = blank();
            block.kind = Kind::Code;
            block.language =
                Some(text_attribute(&attrs, "language").unwrap_or_else(|| "plaintext".to_owned()));
            let text: String = inline(txn, element, resolver)
                .iter()
                .map(|run| run.text.as_str())
                .collect();
            block.runs = normalize(vec![Run::plain(&text)]);
            leaf(block)
        }
        "bulletList" | "orderedList" | "taskList" => {
            let kind = match tag {
                "bulletList" => Kind::BulletedItem,
                "orderedList" => Kind::NumberedItem,
                _ => Kind::Todo,
            };
            let items: Vec<XmlOut> = element.children(txn).collect();
            let mut nodes = Vec::new();
            for item in &items {
                if let XmlOut::Element(item) = item {
                    nodes.push(leading(txn, item, kind, resolver, conflicts));
                }
            }
            nodes
        }
        "toggle" | "blockquote" | "callout" => {
            let kind = match tag {
                "toggle" => Kind::Toggle,
                "blockquote" => Kind::Quote,
                _ => Kind::Callout,
            };
            vec![leading(txn, element, kind, resolver, conflicts)]
        }
        "horizontalRule" => {
            let mut block = blank();
            block.kind = Kind::Divider;
            leaf(block)
        }
        "image" | "file" => {
            let mut block = blank();
            block.kind = if tag == "image" {
                Kind::Image
            } else {
                Kind::File
            };
            block.file = text_attribute(&attrs, "blob").map(|blob| FileRef {
                blob,
                size: number_attribute(&attrs, "size").unwrap_or(0),
                content_type: String::new(),
                name: text_attribute(&attrs, "name").unwrap_or_default(),
            });
            leaf(block)
        }
        "pageLink" => {
            let mut block = blank();
            block.kind = Kind::PageLink;
            block.page = uuid_attribute(&attrs, "page")
                .and_then(|page| resolver.notion_of.get(&page).copied());
            leaf(block)
        }
        "external" => {
            let mut block = blank();
            block.label =
                Some(text_attribute(&attrs, "kind").unwrap_or_else(|| "unsupported".to_owned()));
            leaf(block)
        }
        // A record of Campbooks shown in the page: nothing Notion has.
        "embed" => {
            let mut block = blank();
            block.label = Some("campbooks_embed".to_owned());
            leaf(block)
        }
        "conflict" => {
            if let Some(of) = uuid_attribute(&attrs, "of") {
                let children: Vec<XmlOut> = element.children(txn).collect();
                let mut scratch = Vec::new();
                let mut inner = blocks(txn, &children, resolver, &mut scratch);
                let theirs = if inner.is_empty() {
                    None
                } else {
                    Some(inner.remove(0).block)
                };
                conflicts.push(ConflictNode {
                    of,
                    hash: text_attribute(&attrs, "hash").and_then(|hex| hash_of_hex(&hex)),
                    edited: text_attribute(&attrs, "edited")
                        .and_then(|time| DateTime::parse_from_rfc3339(&time).ok())
                        .map(|time| time.with_timezone(&Utc)),
                    removed: bool_attribute(&attrs, "removed").unwrap_or(false),
                    theirs,
                });
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// An element whose first paragraph is the block's own text and whose other
/// blocks are the blocks under it.
fn leading<T: ReadTxn>(
    txn: &T,
    element: &XmlElementRef,
    kind: Kind,
    resolver: &Resolver,
    conflicts: &mut Vec<ConflictNode>,
) -> Node {
    let attrs = attributes(txn, element);
    let bid = uuid_attribute(&attrs, "bid");
    let mut block = Block::new(bid.unwrap_or_default(), kind);
    if kind == Kind::Todo {
        block.checked = Some(bool_attribute(&attrs, "checked").unwrap_or(false));
    }
    let children: Vec<XmlOut> = element.children(txn).collect();
    let mut rest: &[XmlOut] = &children;
    if let Some(XmlOut::Element(first)) = children.first()
        && &**first.tag() == "paragraph"
    {
        block.runs = inline(txn, first, resolver);
        rest = children.get(1..).unwrap_or_default();
    }
    Node {
        bid,
        block,
        children: blocks(txn, rest, resolver, conflicts),
    }
}

/// The runs of an element's inline content: text with its marks, mentions,
/// hard breaks.
pub fn inline<T: ReadTxn>(txn: &T, element: &XmlElementRef, resolver: &Resolver) -> Vec<Run> {
    let mut runs = Vec::new();
    for child in element.children(txn) {
        match child {
            XmlOut::Text(text) => runs.extend(text_runs(txn, &text)),
            XmlOut::Element(inner) => match &**inner.tag() {
                "mention" => {
                    let attrs = attributes(txn, &inner);
                    if let (Some(ref_type), Some(ref_id)) = (
                        text_attribute(&attrs, "ref_type"),
                        text_attribute(&attrs, "ref_id"),
                    ) {
                        runs.push(resolver.mention(&ref_type, &ref_id));
                    }
                }
                "hardBreak" => runs.push(Run::plain("\n")),
                _ => {}
            },
            XmlOut::Fragment(_) => {}
        }
    }
    normalize(runs)
}

fn text_runs<T: ReadTxn>(txn: &T, text: &XmlTextRef) -> Vec<Run> {
    let mut runs = Vec::new();
    for chunk in text.diff(txn, YChange::identity) {
        let Out::Any(Any::String(content)) = &chunk.insert else {
            continue;
        };
        let mut run = Run::plain(content);
        if let Some(marks) = &chunk.attributes {
            for (key, value) in marks.iter() {
                if matches!(value, Any::Null | Any::Undefined | Any::Bool(false)) {
                    continue;
                }
                // Two marks of one type are kept apart as `name--hash`.
                match key.split("--").next().unwrap_or_default() {
                    "bold" => run.bold = true,
                    "italic" => run.italic = true,
                    "strike" => run.strike = true,
                    "code" => run.code = true,
                    "link" => {
                        if let Any::Map(map) = value
                            && let Some(Any::String(href)) = map.get("href")
                        {
                            run.link = Some(href.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        // A link to Notion's address of a page is that page's mention (it is
        // how a mention of a page that is not synced is written).
        if let Some(page) = run.link.as_deref().and_then(notion_page_of_link) {
            run.page = Some(page);
            run.link = None;
        }
        runs.push(run);
    }
    runs
}

/// The Notion page `https://www.notion.so/<32 hexadecimal digits>` names.
fn notion_page_of_link(link: &str) -> Option<Uuid> {
    let id = link.strip_prefix("https://www.notion.so/")?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Uuid::parse_str(id).ok()
}

#[cfg(test)]
mod tests;
