//! A Notion page's blocks in the neutral form the pages module reads and
//! writes (`contracts/notion-page-snapshot.v1.schema.json` and
//! `contracts/notion-block-changes.v1.schema.json`).
//!
//! The mapping is structural, block type to block type, and never reads what
//! a page says: a paragraph is a paragraph, a toggle is a toggle, a kind
//! Pages lacks is `external` (its Notion type and nothing of its content).
//! Rich text keeps what Pages can show (bold, italic, strike, code, links
//! and mentions of other Notion pages); underline and colour are not kept.
//!
//! Reading a page's blocks is `read.rs`; making changes is `write.rs`. This
//! file is pure: Notion's JSON in, neutral blocks out, and back.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

/// The media type of a [`Snapshot`] blob.
pub const SNAPSHOT_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-page-snapshot+json; version=1";

/// The media type of a [`ChangeDocument`] blob.
pub const CHANGES_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-block-changes+json; version=1";

/// The longest text one Notion rich-text item may hold, in characters.
pub const RICH_TEXT_LIMIT: usize = 2000;

/// The longest a link may be.
const MAX_LINK: usize = 2048;

/// The longest label of an `external` block.
const MAX_LABEL: usize = 32;

/// The code languages Pages knows (its block schema's allowlist).
const LANGUAGES: [&str; 22] = [
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

/// What a block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// An image Notion hosts, downloaded.
    Image,
    /// A file Notion hosts, downloaded.
    File,
    /// A sub-page.
    PageLink,
    /// A block Pages lacks: kept in Notion, never edited from here.
    External,
}

impl Kind {
    /// Whether a block of this kind can be written to Notion.
    pub const fn writable(self) -> bool {
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
                | Self::Divider
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
    fn same_marks(&self, other: &Self) -> bool {
        self.bold == other.bold
            && self.italic == other.italic
            && self.strike == other.strike
            && self.code == other.code
            && self.link == other.link
    }
}

/// A file Notion hosts, downloaded into the workspace's blobs.
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
    /// The block's id at Notion (in a change document: the block's id in
    /// Pages).
    pub id: Uuid,
    /// What it is.
    pub kind: Kind,
    /// A heading's level, 1 to 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// A to-do's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// A code block's language, one of Pages' languages.
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
}

/// A page's blocks as the connector read them
/// (`contracts/notion-page-snapshot.v1.schema.json`).
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

/// What a request of block changes says to write: the block of each item,
/// by the item's place in the request.
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

/// A file hosted by Notion that a block wants downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWant {
    /// Where Notion serves it (a signed address that expires within the
    /// hour).
    pub url: String,
    /// What to call it.
    pub name: String,
}

/// What reading one Notion block gave.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The block, without its children.
    pub block: Block,
    /// Whether the blocks under it are part of the page.
    pub recurse: bool,
    /// A file to download for it.
    pub file: Option<FileWant>,
}

/// A block of Notion's, as the neutral form. `None` for a block that is in
/// the trash or that has no id or edit time.
#[allow(
    clippy::too_many_lines,
    reason = "one match over every Notion block type"
)]
pub fn parse(raw: &Value) -> Option<Parsed> {
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())?;
    let flag = |key: &str| raw.get(key).and_then(Value::as_bool).unwrap_or(false);
    if flag("in_trash") {
        return None;
    }
    let edited = raw
        .get("last_edited_time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())?
        .with_timezone(&Utc);
    let type_name = raw.get("type").and_then(Value::as_str)?;
    let body = raw.get(type_name);
    let has_children = flag("has_children");
    let runs = || runs_of(body.and_then(|body| body.get("rich_text")));
    let mut block = Block::new(id, Kind::External);
    block.edited = Some(edited);
    let mut recurse = false;
    let mut file = None;
    match type_name {
        "paragraph" | "code" if has_children => block.label = Some(label_of(type_name)),
        "paragraph" => {
            block.kind = Kind::Paragraph;
            block.runs = runs();
        }
        "heading_1" | "heading_2" | "heading_3" => {
            let toggleable = body
                .and_then(|body| body.get("is_toggleable"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if toggleable || has_children {
                block.label = Some(label_of(type_name));
            } else {
                block.kind = Kind::Heading;
                block.level = Some(match type_name {
                    "heading_1" => 1,
                    "heading_2" => 2,
                    _ => 3,
                });
                block.runs = runs();
            }
        }
        "bulleted_list_item" | "numbered_list_item" | "toggle" | "quote" | "callout" => {
            block.kind = match type_name {
                "bulleted_list_item" => Kind::BulletedItem,
                "numbered_list_item" => Kind::NumberedItem,
                "toggle" => Kind::Toggle,
                "quote" => Kind::Quote,
                _ => Kind::Callout,
            };
            block.runs = runs();
            recurse = has_children;
        }
        "to_do" => {
            block.kind = Kind::Todo;
            block.checked = Some(
                body.and_then(|body| body.get("checked"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
            block.runs = runs();
            recurse = has_children;
        }
        "divider" => block.kind = Kind::Divider,
        "code" => {
            block.kind = Kind::Code;
            block.language = Some(local_language(
                body.and_then(|body| body.get("language"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ));
            // Code is plain text: marks inside it mean nothing here, and
            // Campbooks stores none, so the hashes of both sides agree.
            let text: String = runs().iter().map(|run| run.text.as_str()).collect();
            block.runs = vec![Run {
                text,
                ..Run::default()
            }];
        }
        "image" | "file" | "pdf" => {
            let hosted = body
                .filter(|body| body.get("type").and_then(Value::as_str) == Some("file"))
                .and_then(|body| {
                    let url = body.pointer("/file/url").and_then(Value::as_str)?;
                    let name = body
                        .get("name")
                        .and_then(Value::as_str)
                        .map(clean_name)
                        .filter(|name| !name.is_empty())
                        .or_else(|| name_from_url(url))
                        .unwrap_or_else(|| type_name.to_owned());
                    Some(FileWant {
                        url: url.to_owned(),
                        name,
                    })
                });
            match hosted {
                Some(want) => {
                    block.kind = if type_name == "image" {
                        Kind::Image
                    } else {
                        Kind::File
                    };
                    file = Some(want);
                }
                None => block.label = Some(label_of(type_name)),
            }
        }
        "child_page" => {
            block.kind = Kind::PageLink;
            block.page = Some(id);
            let title = body
                .and_then(|body| body.get("title"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            block.runs = normalize(vec![Run {
                text: title.to_owned(),
                ..Run::default()
            }]);
        }
        other => block.label = Some(label_of(other)),
    }
    Some(Parsed {
        block,
        recurse,
        file,
    })
}

/// A Notion type as the label of an `external` block: lower-case letters,
/// digits and `_`, at most 32.
fn label_of(type_name: &str) -> String {
    let label: String = type_name
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
        .take(MAX_LABEL)
        .collect();
    if label.is_empty() {
        "unsupported".to_owned()
    } else {
        label
    }
}

/// A file name Pages can show: displayable characters, at most 255.
fn clean_name(raw: &str) -> String {
    campbooks_kernel::text::displayable(raw)
        .trim()
        .chars()
        .take(255)
        .collect()
}

/// The last segment of a file's address, decoded, as its name.
fn name_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let last = parsed.path_segments()?.next_back()?;
    let name = clean_name(&percent_decode(last));
    (!name.is_empty()).then_some(name)
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let decoded = (byte == b'%')
            .then(|| bytes.get(at + 1..at + 3))
            .flatten()
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match decoded {
            Some(value) => {
                out.push(value);
                at += 3;
            }
            None => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A link Pages may keep: `https`, `http` or `mailto`, no spaces or control
/// characters. A path Notion gave (`/<page id>`) is Notion's.
pub fn clean_link(href: &str) -> Option<String> {
    let href = href.trim();
    let absolute = if href.starts_with('/') && !href.starts_with("//") {
        format!("https://www.notion.so{href}")
    } else {
        href.to_owned()
    };
    let lower = absolute.to_ascii_lowercase();
    let scheme_ok = ["https:", "http:", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme));
    let clean = absolute.len() <= MAX_LINK
        && absolute
            .chars()
            .all(|c| !c.is_control() && !c.is_whitespace());
    (scheme_ok && clean).then_some(absolute)
}

/// Notion's rich text as runs.
pub fn runs_of(value: Option<&Value>) -> Vec<Run> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut runs = Vec::with_capacity(items.len());
    for item in items {
        let text = item
            .get("plain_text")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/text/content").and_then(Value::as_str))
            .unwrap_or_default();
        let mark = |name: &str| {
            item.pointer(&format!("/annotations/{name}"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        let mut run = Run {
            text: text.to_owned(),
            bold: mark("bold"),
            italic: mark("italic"),
            strike: mark("strikethrough"),
            code: mark("code"),
            link: None,
            page: None,
        };
        let mentions_page = item.get("type").and_then(Value::as_str) == Some("mention")
            && item.pointer("/mention/type").and_then(Value::as_str) == Some("page");
        if mentions_page {
            run.page = item
                .pointer("/mention/page/id")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok());
        }
        if run.page.is_none() {
            run.link = item
                .pointer("/text/link/url")
                .and_then(Value::as_str)
                .or_else(|| item.get("href").and_then(Value::as_str))
                .and_then(clean_link);
        }
        runs.push(run);
    }
    normalize(runs)
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

/// Pages' name of a Notion code language; unknown ones are plain text.
fn local_language(notion: &str) -> String {
    let lower = notion.trim().to_ascii_lowercase();
    let local = match lower.as_str() {
        "plain text" | "" => "plaintext",
        "c++" => "cpp",
        "c#" => "csharp",
        "shell" | "sh" | "zsh" => "bash",
        other => other,
    };
    if LANGUAGES.contains(&local) {
        local.to_owned()
    } else {
        "plaintext".to_owned()
    }
}

/// Notion's name of one of Pages' code languages.
fn notion_language(local: &str) -> &'static str {
    match local {
        "cpp" => "c++",
        "csharp" => "c#",
        "bash" => "bash",
        "c" => "c",
        "css" => "css",
        "go" => "go",
        "html" => "html",
        "java" => "java",
        "javascript" => "javascript",
        "json" => "json",
        "kotlin" => "kotlin",
        "markdown" => "markdown",
        "php" => "php",
        "python" => "python",
        "ruby" => "ruby",
        "rust" => "rust",
        "sql" => "sql",
        "swift" => "swift",
        "typescript" => "typescript",
        "xml" => "xml",
        "yaml" => "yaml",
        _ => "plain text",
    }
}

/// Runs as Notion's rich text: every item at most [`RICH_TEXT_LIMIT`]
/// characters. Underline and colour are not written (Pages has none).
pub fn rich_text(runs: &[Run]) -> Vec<Value> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        let annotations = json!({
            "bold": run.bold,
            "italic": run.italic,
            "strikethrough": run.strike,
            "underline": false,
            "code": run.code,
            "color": "default",
        });
        if let Some(page) = run.page {
            out.push(json!({
                "type": "mention",
                "mention": {"type": "page", "page": {"id": page.to_string()}},
                "annotations": annotations,
            }));
            continue;
        }
        let mut rest = run.text.as_str();
        while !rest.is_empty() {
            let cut = rest
                .char_indices()
                .nth(RICH_TEXT_LIMIT)
                .map_or(rest.len(), |(at, _)| at);
            let (head, tail) = rest.split_at(cut);
            out.push(json!({
                "type": "text",
                "text": {
                    "content": head,
                    "link": run.link.as_ref().map(|url| json!({"url": url})),
                },
                "annotations": annotations,
            }));
            rest = tail;
        }
    }
    out
}

/// A block's Notion type and the body that goes under that name; `None` for
/// a kind that is not written.
fn body_of(block: &Block) -> Option<(String, Value)> {
    let text = || rich_text(&block.runs);
    let named = |name: &str, body: Value| Some((name.to_owned(), body));
    match block.kind {
        Kind::Paragraph => named("paragraph", json!({"rich_text": text()})),
        Kind::Heading => {
            let level = block.level.unwrap_or(1).clamp(1, 3);
            named(&format!("heading_{level}"), json!({"rich_text": text()}))
        }
        Kind::BulletedItem => named("bulleted_list_item", json!({"rich_text": text()})),
        Kind::NumberedItem => named("numbered_list_item", json!({"rich_text": text()})),
        Kind::Todo => named(
            "to_do",
            json!({"rich_text": text(), "checked": block.checked.unwrap_or(false)}),
        ),
        Kind::Toggle => named("toggle", json!({"rich_text": text()})),
        Kind::Quote => named("quote", json!({"rich_text": text()})),
        Kind::Callout => named("callout", json!({"rich_text": text()})),
        Kind::Divider => named("divider", json!({})),
        Kind::Code => named(
            "code",
            json!({
                "rich_text": text(),
                "language": notion_language(block.language.as_deref().unwrap_or("plaintext")),
            }),
        ),
        Kind::Image | Kind::File | Kind::PageLink | Kind::External => None,
    }
}

/// The body of a request that adds `block` as a new block; `None` for a
/// kind that is not written.
pub fn create_json(block: &Block) -> Option<Value> {
    let (name, body) = body_of(block)?;
    let mut map = Map::new();
    map.insert("object".to_owned(), Value::String("block".to_owned()));
    map.insert("type".to_owned(), Value::String(name.clone()));
    map.insert(name, body);
    Some(Value::Object(map))
}

/// The body of a request that rewrites a block as `block`; `None` for a kind
/// that has nothing to rewrite (a divider) or is not written.
pub fn update_json(block: &Block) -> Option<Value> {
    if block.kind == Kind::Divider {
        return None;
    }
    let (name, body) = body_of(block)?;
    let mut map = Map::new();
    map.insert(name, body);
    Some(Value::Object(map))
}

#[cfg(test)]
mod tests;
