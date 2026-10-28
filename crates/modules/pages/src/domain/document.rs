//! A page's body as a Yjs document (`yrs`), and the checks every update
//! passes before anything is stored.
//!
//! The document is one XML fragment named `default` (what `TipTap`'s
//! collaboration extension writes): blocks, with text and inline nodes
//! inside. The server never trusts what a client sends. It applies an update
//! to the page's document in memory, then reads the **whole result** against
//! an allowlist:
//!
//! - only the block, inline and mark names of the schema, each only where
//!   the schema lets it stand, with only its own attributes, each of the
//!   right kind;
//! - links only to `https:`, `http:` or `mailto:`; mentions and embeds only
//!   of the record types of the schema, with a well-formed id;
//! - files and images naming a blob by its hash (the caller checks the blob
//!   is one they may use);
//! - no root type but `default`, no nested document, nothing hidden;
//! - an update of at most 256 KiB, a document of at most 4 MiB and 20,000
//!   blocks, nested at most 32 deep;
//! - an update that leaves a dependency missing is refused, never kept half
//!   applied.
//!
//! Anything else is a [`Rejection`], and nothing is stored.

use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};

use campbooks_kernel::Error;
use yrs::{
    Any, Doc, Out, ReadTxn, StateVector, Text, Transact, TransactionMut, Update, Xml,
    XmlElementRef, XmlFragment, XmlOut, XmlTextRef,
    types::text::YChange,
    updates::{decoder::Decode, encoder::Encode},
};

use super::LinkKind;

/// The name of the document's one root type.
pub const ROOT: &str = "default";

/// The largest update accepted, in bytes.
pub const MAX_UPDATE_BYTES: usize = 256 * 1024;

/// The largest document, as a full update, in bytes.
pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// The most blocks (elements and text runs) a document holds.
pub const MAX_BLOCKS: usize = 20_000;

/// How deep blocks nest at most.
const MAX_NESTING: usize = 32;

/// The longest a file name, link or other attribute text may be.
const MAX_ATTRIBUTE: usize = 2048;

/// The largest file a block may name, in bytes.
const MAX_FILE_BYTES: i64 = 1 << 30;

/// The record types a mention or an embed may point at.
pub const REF_TYPES: &[&str] = &[
    "workspaces.member",
    "conversations.conversation",
    "tasks.task",
    "schedule.event",
    "pages.page",
    "person",
];

/// The code languages a code block may name.
pub const LANGUAGES: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "csharp",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "kotlin",
    "markdown",
    "php",
    "python",
    "ruby",
    "rust",
    "sql",
    "swift",
    "typescript",
    "xml",
    "yaml",
    "plaintext",
];

/// Why an update, or the document it would make, was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Over 256 KiB.
    UpdateTooLarge,
    /// Not a Yjs update, or one that cannot be applied.
    Malformed,
    /// It needs changes the document does not have.
    OutOfOrder,
    /// The document would be over 4 MiB.
    DocumentTooLarge,
    /// The document would have over 20,000 blocks.
    TooManyBlocks,
    /// A root type besides the page's body.
    UnknownRoot,
    /// A block, inline node or mark the schema does not have.
    UnknownBlock,
    /// An attribute the block does not have, or of the wrong kind.
    BadAttribute,
    /// A link that is not https, http or mailto.
    BadLink,
    /// A mention or embed of a record type the schema does not have, or with
    /// an id that is not one.
    BadReference,
    /// A block where the schema does not let it stand.
    BadStructure,
    /// Blocks nested too deep.
    TooDeep,
}

impl Rejection {
    /// The code of the 400 it is answered with.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UpdateTooLarge => "update_too_large",
            Self::Malformed | Self::OutOfOrder | Self::UnknownRoot | Self::BadStructure => {
                "invalid_update"
            }
            Self::DocumentTooLarge | Self::TooManyBlocks | Self::TooDeep => "document_too_large",
            Self::UnknownBlock | Self::BadAttribute | Self::BadReference => "unknown_block",
            Self::BadLink => "link_scheme",
        }
    }
}

impl From<Rejection> for Error {
    fn from(rejection: Rejection) -> Self {
        Self::BadRequest(crate::MESSAGES.reason(rejection.code()))
    }
}

/// A record a body points at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Link {
    /// The record type.
    pub ref_type: String,
    /// The record's id (a person's cleaned address).
    pub ref_id: String,
    /// How the body points at it.
    pub kind: LinkKind,
}

/// What a document holds, read while it was checked.
#[derive(Debug, Clone, Default)]
pub struct Inspection {
    /// How many blocks (elements and text runs).
    pub blocks: usize,
    /// The hashes of the blobs its files and images name.
    pub blobs: BTreeSet<String>,
    /// What it mentions, embeds and links to.
    pub links: BTreeSet<Link>,
    /// The tables it shows, as (table, view) ids.
    pub table_views: BTreeSet<(String, String)>,
    /// Its plain text, one line per text block; mentions add nothing.
    pub text: String,
    /// Its size as a full update, in bytes.
    pub size: usize,
}

/// Run `work`, answering [`Rejection::Malformed`] if it panics: a crafted
/// update never takes a request down.
fn guarded<T>(work: impl FnOnce() -> Result<T, Rejection>) -> Result<T, Rejection> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(result) => result,
        Err(_) => Err(Rejection::Malformed),
    }
}

fn apply_to(txn: &mut TransactionMut<'_>, bytes: &[u8]) -> Result<(), Rejection> {
    let update = Update::decode_v1(bytes).map_err(|_| Rejection::Malformed)?;
    txn.apply_update(update).map_err(|_| Rejection::Malformed)
}

/// The document made of a stored `snapshot` and the updates after it. These
/// were checked when they were stored.
pub fn open(snapshot: Option<&[u8]>, tail: &[Vec<u8>]) -> Result<Doc, Rejection> {
    guarded(|| {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            if let Some(state) = snapshot {
                apply_to(&mut txn, state)?;
            }
            for update in tail {
                apply_to(&mut txn, update)?;
            }
        }
        Ok(doc)
    })
}

/// Apply a client's `update` to `doc`. Refused when it is too large, is not
/// a Yjs update, or needs changes `doc` does not have. Run [`inspect`] on
/// the result before keeping it.
pub fn apply(doc: &Doc, update: &[u8]) -> Result<(), Rejection> {
    if update.len() > MAX_UPDATE_BYTES {
        return Err(Rejection::UpdateTooLarge);
    }
    guarded(|| {
        let decoded = Update::decode_v1(update).map_err(|_| Rejection::Malformed)?;
        let mut txn = doc.transact_mut();
        txn.apply_update(decoded)
            .map_err(|_| Rejection::Malformed)?;
        let store = ReadTxn::store(&txn);
        if store.pending_update().is_some() || store.pending_ds().is_some() {
            return Err(Rejection::OutOfOrder);
        }
        Ok(())
    })
}

/// The document's state vector (v1).
pub fn state_vector(doc: &Doc) -> Vec<u8> {
    doc.transact().state_vector().encode_v1()
}

/// The whole document as one update (v1).
pub fn full_state(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

/// What a client holding `state_vector` (v1) is missing, as an update (v1).
pub fn diff_since(doc: &Doc, state_vector: &[u8]) -> Result<Vec<u8>, Rejection> {
    let vector = StateVector::decode_v1(state_vector).map_err(|_| Rejection::Malformed)?;
    guarded(|| Ok(doc.transact().encode_state_as_update_v1(&vector)))
}

/// Read the whole document against the schema.
pub fn inspect(doc: &Doc) -> Result<Inspection, Rejection> {
    guarded(|| {
        let root = doc.get_or_insert_xml_fragment(ROOT);
        let txn = doc.transact();
        if txn.root_refs().any(|(name, _)| name != ROOT) {
            return Err(Rejection::UnknownRoot);
        }
        let size = txn.encode_state_as_update_v1(&StateVector::default()).len();
        if size > MAX_DOCUMENT_BYTES {
            return Err(Rejection::DocumentTooLarge);
        }
        let mut walk = Walk {
            txn: &txn,
            blocks: 0,
            blobs: BTreeSet::new(),
            links: BTreeSet::new(),
            table_views: BTreeSet::new(),
            text: String::new(),
        };
        walk.children(&root, Context::Blocks, 1)?;
        Ok(Inspection {
            blocks: walk.blocks,
            blobs: walk.blobs,
            links: walk.links,
            table_views: walk.table_views,
            text: walk.text,
            size,
        })
    })
}

/// What a block may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    /// Blocks (paragraphs, lists, callouts, ...).
    Blocks,
    /// Only these items.
    Items(&'static [&'static str]),
    /// Text runs and inline nodes.
    Inline,
    /// One or more plain text runs.
    CodeText,
    /// Nothing.
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Block,
    Item,
    Inline,
}

#[derive(Debug, Clone, Copy)]
struct Spec {
    class: Class,
    content: Context,
}

/// The schema: what each element name is, and what it holds.
fn spec(name: &str) -> Option<Spec> {
    let (class, content) = match name {
        "paragraph" | "heading" => (Class::Block, Context::Inline),
        "codeBlock" => (Class::Block, Context::CodeText),
        "bulletList" | "orderedList" => (Class::Block, Context::Items(&["listItem"])),
        "taskList" => (Class::Block, Context::Items(&["taskItem"])),
        "listItem" | "taskItem" => (Class::Item, Context::Blocks),
        // A conflict is Notion's version of a block we disagree about,
        // beneath ours.
        "toggle" | "blockquote" | "callout" | "conflict" => (Class::Block, Context::Blocks),
        "horizontalRule" | "image" | "file" | "pageLink" | "embed" | "external" | "tableView" => {
            (Class::Block, Context::Empty)
        }
        "mention" | "hardBreak" => (Class::Inline, Context::Empty),
        _ => return None,
    };
    Some(Spec { class, content })
}

/// Whether a `child` element may stand in `context`.
fn admits(context: Context, name: &str, child: Spec) -> bool {
    match context {
        Context::Blocks => child.class == Class::Block,
        Context::Items(names) => names.contains(&name),
        Context::Inline => child.class == Class::Inline,
        Context::CodeText | Context::Empty => false,
    }
}

fn text_of(value: &Any) -> Option<&str> {
    match value {
        Any::String(text) => Some(&**text),
        _ => None,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "the value is finite, whole and within 2^53 just above"
)]
fn whole_of(value: &Any) -> Option<i64> {
    match value {
        Any::Number(number) if number.is_finite() && number.fract() == 0.0 => {
            (number.abs() < 9.0e15).then_some(*number as i64)
        }
        Any::BigInt(number) => Some(*number),
        _ => None,
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_uuid(text: &str) -> bool {
    sqlx::types::Uuid::parse_str(text).is_ok_and(|id| id.to_string() == text)
}

/// A correspondent's address as it is kept: trimmed, lower case, one `@`,
/// no spaces. A structural check of an address, nothing about content.
pub fn clean_address(raw: &str) -> Option<String> {
    let address = raw.trim().to_lowercase();
    let well_formed = address.len() <= 320
        && address.matches('@').count() == 1
        && !address.starts_with('@')
        && !address.ends_with('@')
        && address
            .chars()
            .all(|c| !c.is_whitespace() && !c.is_control());
    well_formed.then_some(address)
}

pub fn is_reference(ref_type: &str, ref_id: &str) -> bool {
    match ref_type {
        "person" => clean_address(ref_id).as_deref() == Some(ref_id),
        other => REF_TYPES.contains(&other) && is_uuid(ref_id),
    }
}

fn safe_href(href: &str) -> Result<(), Rejection> {
    if href.len() > MAX_ATTRIBUTE
        || href.chars().any(|c| {
            c.is_control() || c.is_whitespace() || campbooks_kernel::text::is_bidi_control(c)
        })
    {
        return Err(Rejection::BadLink);
    }
    let lower = href.to_ascii_lowercase();
    if ["https:", "http:", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        Ok(())
    } else {
        Err(Rejection::BadLink)
    }
}

fn check_mark(key: &str, value: &Any) -> Result<(), Rejection> {
    // y-prosemirror keeps two marks of one type apart as `name--hash`.
    let name = key.split("--").next().unwrap_or(key);
    match name {
        "bold" | "italic" | "strike" | "code" => match value {
            Any::Null | Any::Undefined | Any::Bool(_) => Ok(()),
            Any::Map(map) if map.is_empty() => Ok(()),
            _ => Err(Rejection::BadAttribute),
        },
        "link" => match value {
            Any::Null | Any::Undefined => Ok(()),
            Any::Map(map) => {
                let mut href: Option<&str> = None;
                for (attribute, held) in map.iter() {
                    match (attribute.as_str(), held) {
                        ("href", Any::String(text)) => href = Some(&**text),
                        ("target" | "rel" | "class", Any::String(text))
                            if text.len() <= 256 && text.chars().all(|c| !c.is_control()) => {}
                        ("href" | "target" | "rel" | "class", Any::Null | Any::Undefined) => {}
                        _ => return Err(Rejection::BadAttribute),
                    }
                }
                safe_href(href.ok_or(Rejection::BadLink)?)
            }
            _ => Err(Rejection::BadAttribute),
        },
        _ => Err(Rejection::UnknownBlock),
    }
}

/// One element's attributes, the non-empty ones.
type Attributes = BTreeMap<String, Any>;

/// Check that `attributes` are the ones element `name` has, of the right
/// kind.
fn check_attributes(name: &str, attributes: &Attributes) -> Result<(), Rejection> {
    for (key, value) in attributes {
        let fits = match (name, key.as_str()) {
            // Every block has a stable id, the same block at Notion; a
            // conflict names the block it is about.
            (_, "bid") | ("conflict", "of") | ("pageLink", "page") => {
                text_of(value).is_some_and(is_uuid)
            }
            // A table shown in the page: the table and one of its views, by id only.
            ("tableView", "table" | "view") => text_of(value).is_some_and(is_uuid),
            ("conflict", "hash") | ("image" | "file", "blob") => {
                text_of(value).is_some_and(is_hash)
            }
            ("conflict", "edited") => text_of(value)
                .is_some_and(|t| t.len() <= 40 && t.chars().all(|c| c.is_ascii_graphic())),
            ("conflict", "removed") | ("taskItem", "checked") | ("toggle", "open") => {
                matches!(value, Any::Bool(_))
            }
            // A block kept in Notion: its Notion type, nothing of its content.
            ("external", "kind") => text_of(value).is_some_and(|t| {
                !t.is_empty()
                    && t.len() <= 32
                    && t.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            }),
            ("heading", "level") => whole_of(value).is_some_and(|n| (1..=3).contains(&n)),
            ("orderedList", "start") => {
                whole_of(value).is_some_and(|n| (1..=1_000_000).contains(&n))
            }
            ("orderedList", "type") => text_of(value)
                .is_some_and(|t| t.len() <= 8 && t.chars().all(|c| c.is_ascii_alphanumeric())),
            ("callout", "tone") => {
                text_of(value).is_some_and(|t| ["neutral", "info", "warning"].contains(&t))
            }
            ("codeBlock", "language") => text_of(value).is_some_and(|t| LANGUAGES.contains(&t)),
            ("image" | "file", "name") => text_of(value).is_some_and(|t| {
                t.chars().count() <= 255 && t.chars().all(campbooks_kernel::text::is_displayable)
            }),
            ("image" | "file", "size") => {
                whole_of(value).is_some_and(|n| (0..=MAX_FILE_BYTES).contains(&n))
            }
            ("mention" | "embed", "ref_type") => {
                text_of(value).is_some_and(|t| REF_TYPES.contains(&t))
            }
            ("mention" | "embed", "ref_id") => text_of(value).is_some_and(|t| t.len() <= 320),
            _ => return Err(Rejection::BadAttribute),
        };
        if !fits {
            return Err(Rejection::BadAttribute);
        }
    }
    Ok(())
}

struct Walk<'a, T: ReadTxn> {
    txn: &'a T,
    blocks: usize,
    blobs: BTreeSet<String>,
    links: BTreeSet<Link>,
    table_views: BTreeSet<(String, String)>,
    text: String,
}

impl<T: ReadTxn> Walk<'_, T> {
    fn count(&mut self) -> Result<(), Rejection> {
        self.blocks += 1;
        if self.blocks > MAX_BLOCKS {
            return Err(Rejection::TooManyBlocks);
        }
        Ok(())
    }

    fn children(
        &mut self,
        parent: &impl XmlFragment,
        context: Context,
        depth: usize,
    ) -> Result<(), Rejection> {
        let txn = self.txn;
        for child in parent.children(txn) {
            match child {
                XmlOut::Element(element) => self.element(&element, context, depth)?,
                XmlOut::Text(text) => self.text_run(&text, context)?,
                XmlOut::Fragment(_) => return Err(Rejection::UnknownBlock),
            }
        }
        Ok(())
    }

    fn element(
        &mut self,
        element: &XmlElementRef,
        parent: Context,
        depth: usize,
    ) -> Result<(), Rejection> {
        if depth > MAX_NESTING {
            return Err(Rejection::TooDeep);
        }
        let tag: &str = element.tag();
        let spec = spec(tag).ok_or(Rejection::UnknownBlock)?;
        if !admits(parent, tag, spec) {
            return Err(Rejection::BadStructure);
        }
        self.count()?;
        let txn = self.txn;
        let mut attributes = Attributes::new();
        for (key, value) in element.attributes(txn) {
            match value {
                Out::Any(Any::Null | Any::Undefined) => {}
                Out::Any(any) => {
                    attributes.insert(key.to_owned(), any);
                }
                _ => return Err(Rejection::BadAttribute),
            }
        }
        check_attributes(tag, &attributes)?;
        self.record(tag, &attributes)?;
        match spec.content {
            Context::Empty => {
                if element.children(txn).next().is_some() {
                    return Err(Rejection::BadStructure);
                }
            }
            content => self.children(element, content, depth + 1)?,
        }
        if matches!(spec.content, Context::Inline | Context::CodeText) {
            self.text.push('\n');
        }
        Ok(())
    }

    /// Note what a node points at, once its attributes fit.
    fn record(&mut self, tag: &str, attributes: &Attributes) -> Result<(), Rejection> {
        let held = |key: &str| attributes.get(key).and_then(text_of);
        match tag {
            "image" | "file" => {
                let blob = held("blob").ok_or(Rejection::BadAttribute)?;
                self.blobs.insert(blob.to_owned());
            }
            "pageLink" => {
                let page = held("page").ok_or(Rejection::BadAttribute)?;
                self.links.insert(Link {
                    ref_type: "pages.page".to_owned(),
                    ref_id: page.to_owned(),
                    kind: LinkKind::Child,
                });
            }
            "tableView" => {
                let table = held("table").ok_or(Rejection::BadReference)?;
                let view = held("view").ok_or(Rejection::BadReference)?;
                self.table_views.insert((table.to_owned(), view.to_owned()));
                self.links.insert(Link {
                    ref_type: "pages.page".to_owned(),
                    ref_id: table.to_owned(),
                    kind: LinkKind::Embed,
                });
            }
            "mention" | "embed" => {
                let ref_type = held("ref_type").ok_or(Rejection::BadReference)?;
                let ref_id = held("ref_id").ok_or(Rejection::BadReference)?;
                if !is_reference(ref_type, ref_id) {
                    return Err(Rejection::BadReference);
                }
                self.links.insert(Link {
                    ref_type: ref_type.to_owned(),
                    ref_id: ref_id.to_owned(),
                    kind: if tag == "mention" {
                        LinkKind::Mention
                    } else {
                        LinkKind::Embed
                    },
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn text_run(&mut self, run: &XmlTextRef, context: Context) -> Result<(), Rejection> {
        if !matches!(context, Context::Inline | Context::CodeText) {
            return Err(Rejection::BadStructure);
        }
        self.count()?;
        let txn = self.txn;
        if run.attributes(txn).next().is_some() {
            return Err(Rejection::BadAttribute);
        }
        for chunk in run.diff(txn, YChange::identity) {
            let Out::Any(Any::String(text)) = &chunk.insert else {
                return Err(Rejection::BadStructure);
            };
            if let Some(marks) = &chunk.attributes {
                for (key, value) in marks.iter() {
                    if context == Context::CodeText && !matches!(value, Any::Null | Any::Undefined)
                    {
                        return Err(Rejection::BadAttribute);
                    }
                    check_mark(key, value)?;
                }
            }
            self.text.push_str(text);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
