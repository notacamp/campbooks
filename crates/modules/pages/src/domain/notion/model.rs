//! A Notion page's blocks in the neutral form the connectors module hands
//! over (`contracts/notion-page-snapshot.v1.schema.json`) and takes
//! (`contracts/notion-block-changes.v1.schema.json`), and the one hash of a
//! block's content that tells an edit from a re-serialisation.
//!
//! The mapping to and from the page's body is structural, block kind to
//! block kind (`tree.rs`, `edit.rs`): nothing here reads what a block says
//! to decide anything. The types are this module's own copy of the contract
//! (modules never import each other); `tests::contract` keeps the two
//! honest against the schema files.

use campbooks_kernel::blobs::BlobHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The media type of a snapshot blob.
pub const SNAPSHOT_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-page-snapshot+json; version=1";

/// The media type of a block-changes blob.
pub const CHANGES_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-block-changes+json; version=1";

/// What a block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A paragraph.
    Paragraph,
    /// A heading of `level` 1 to 3.
    Heading,
    /// A bulleted list item.
    BulletedItem,
    /// A numbered list item.
    NumberedItem,
    /// A to-do item.
    Todo,
    /// A toggle.
    Toggle,
    /// A quote.
    Quote,
    /// A callout.
    Callout,
    /// A divider.
    Divider,
    /// A code block.
    Code,
    /// An image.
    Image,
    /// A file.
    File,
    /// A link to a sub-page.
    PageLink,
    /// A block Pages lacks: kept in Notion, never edited from here.
    External,
}

impl Kind {
    /// The name kept in `pages.external_blocks.kind`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading => "heading",
            Self::BulletedItem => "bulleted_item",
            Self::NumberedItem => "numbered_item",
            Self::Todo => "todo",
            Self::Toggle => "toggle",
            Self::Quote => "quote",
            Self::Callout => "callout",
            Self::Divider => "divider",
            Self::Code => "code",
            Self::Image => "image",
            Self::File => "file",
            Self::PageLink => "page_link",
            Self::External => "external",
        }
    }

    /// The kind named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "paragraph" => Self::Paragraph,
            "heading" => Self::Heading,
            "bulleted_item" => Self::BulletedItem,
            "numbered_item" => Self::NumberedItem,
            "todo" => Self::Todo,
            "toggle" => Self::Toggle,
            "quote" => Self::Quote,
            "callout" => Self::Callout,
            "divider" => Self::Divider,
            "code" => Self::Code,
            "image" => Self::Image,
            "file" => Self::File,
            "page_link" => Self::PageLink,
            "external" => Self::External,
            _ => return None,
        })
    }

    /// Whether the block's own text is the first paragraph of its element,
    /// with the blocks under it after (a list item, a to-do, a toggle, a
    /// quote, a callout).
    pub const fn leads_with_text(self) -> bool {
        matches!(
            self,
            Self::BulletedItem
                | Self::NumberedItem
                | Self::Todo
                | Self::Toggle
                | Self::Quote
                | Self::Callout
        )
    }

    /// Whether the block is an item of a list element.
    pub const fn is_item(self) -> bool {
        matches!(self, Self::BulletedItem | Self::NumberedItem | Self::Todo)
    }

    /// Whether the member can change a block of this kind from here, and it
    /// is written to Notion.
    pub const fn editable(self) -> bool {
        matches!(
            self,
            Self::Paragraph
                | Self::Heading
                | Self::BulletedItem
                | Self::NumberedItem
                | Self::Todo
                | Self::Toggle
                | Self::Quote
                | Self::Callout
                | Self::Code
        )
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A stretch of text with the same marks.
#[allow(
    clippy::struct_excessive_bools,
    reason = "the four marks Pages can show, each independent"
)]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Run {
    /// The text. A page mention keeps the words Notion showed for it.
    pub text: String,
    /// Bold.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    /// Italic.
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Struck through.
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    /// Inline code.
    #[serde(default, skip_serializing_if = "is_false")]
    pub code: bool,
    /// Where it links: `https`, `http` or `mailto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The Notion page it mentions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Uuid>,
}

impl Run {
    /// Plain text.
    pub fn plain(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            ..Self::default()
        }
    }

    fn same_marks(&self, other: &Self) -> bool {
        self.bold == other.bold
            && self.italic == other.italic
            && self.strike == other.strike
            && self.code == other.code
            && self.link == other.link
    }
}

/// Runs without empty ones, and neighbours with the same marks joined: the
/// one shape a text has, whoever wrote it.
pub fn normalize(runs: Vec<Run>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        if run.text.is_empty() && run.page.is_none() {
            continue;
        }
        if let Some(last) = out.last_mut()
            && last.page.is_none()
            && run.page.is_none()
            && last.same_marks(&run)
        {
            last.text.push_str(&run.text);
            continue;
        }
        out.push(run);
    }
    out
}

/// A file Notion hosted, downloaded into the workspace's blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRef {
    /// The blob's hash.
    pub blob: String,
    /// Its size in bytes.
    pub size: u64,
    /// Its media type.
    pub content_type: String,
    /// Its name.
    pub name: String,
}

/// One block, with the blocks under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// The block's id: at Notion in a snapshot; in the page, in a change
    /// document.
    pub id: Uuid,
    /// What it is.
    pub kind: Kind,
    /// A heading's level, 1 to 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// A to-do's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// A code block's language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Its text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<Run>,
    /// An image's or file's blob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileRef>,
    /// A sub-page's Notion page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Uuid>,
    /// An `external` block's Notion type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// When Notion last edited it (a snapshot's blocks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<DateTime<Utc>>,
    /// The blocks under it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
}

impl Block {
    /// A block of `kind` with nothing else yet.
    pub fn new(id: Uuid, kind: Kind) -> Self {
        Self {
            id,
            kind,
            level: None,
            checked: None,
            language: None,
            runs: Vec::new(),
            file: None,
            page: None,
            label: None,
            edited: None,
            children: Vec::new(),
        }
    }

    /// A paragraph of plain `text`.
    pub fn paragraph(id: Uuid, text: &str) -> Self {
        let mut block = Self::new(id, Kind::Paragraph);
        block.runs = normalize(vec![Run::plain(text)]);
        block
    }

    /// The block's plain text.
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

/// A page's blocks as the connector read them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The format version, 1.
    pub version: u16,
    /// The Notion page.
    pub notion_page: Uuid,
    /// Its title.
    pub title: String,
    /// When Notion last edited the page.
    pub edited: DateTime<Utc>,
    /// When it was read: later snapshots of one page have later times.
    pub read_at: DateTime<Utc>,
    /// Every block was read. A page over the limits is read in part: blocks
    /// not seen are not blocks that went away.
    pub complete: bool,
    /// The blocks, in order.
    pub blocks: Vec<Block>,
}

/// What a request of block changes says to write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeDocument {
    /// The format version, 1.
    pub version: u16,
    /// The items.
    pub items: Vec<ChangeItem>,
}

/// One item of a [`ChangeDocument`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeItem {
    /// The item's place in the request.
    pub seq: u32,
    /// What to write, for an update or an append.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<Block>,
}

/// The canonical content of a run: what its hash is made of. A page
/// mention is its page and nothing of the words Notion showed for it.
#[derive(Serialize)]
struct CanonRun<'a> {
    t: &'a str,
    #[serde(skip_serializing_if = "is_false")]
    b: bool,
    #[serde(skip_serializing_if = "is_false")]
    i: bool,
    #[serde(skip_serializing_if = "is_false")]
    s: bool,
    #[serde(skip_serializing_if = "is_false")]
    c: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    l: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p: Option<Uuid>,
}

/// The canonical content of a block, without the blocks under it: its kind,
/// what the kind has of attributes, and its text. Never an id or a time.
#[derive(Serialize)]
struct Canon<'a> {
    k: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    runs: Vec<CanonRun<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blob: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
}

/// The SHA-256 of a block's content: the same for the same text in the same
/// kind, however either side serialised it. A sub-page's link is its page
/// (its title is the page's), and an `external` block is its Notion type.
pub fn hash(block: &Block) -> [u8; 32] {
    let text_matters = !matches!(block.kind, Kind::PageLink | Kind::External);
    let runs = if text_matters {
        block
            .runs
            .iter()
            .map(|run| CanonRun {
                t: if run.page.is_some() { "" } else { &run.text },
                b: run.bold,
                i: run.italic,
                s: run.strike,
                c: run.code,
                l: run.link.as_deref(),
                p: run.page,
            })
            .collect()
    } else {
        Vec::new()
    };
    let canon = Canon {
        k: block.kind,
        level: block.level,
        checked: block.checked,
        language: block.language.as_deref(),
        runs,
        blob: block.file.as_ref().map(|file| file.blob.as_str()),
        page: block.page,
        label: block.label.as_deref(),
    };
    // A struct of strings, numbers and booleans always encodes.
    let bytes = serde_json::to_vec(&canon).unwrap_or_default();
    *BlobHash::of(&bytes).as_bytes()
}

/// A hash as the 64 hexadecimal digits a conflict carries.
pub fn hash_hex(hash: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }
    out
}

/// The hash the 64 hexadecimal digits `hex` spell.
pub fn hash_of_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let mut out = [0_u8; 32];
    for (at, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(at * 2..at * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests;
