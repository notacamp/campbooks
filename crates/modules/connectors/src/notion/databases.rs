//! A Notion database (a data source, API version 2026-03-11) in the neutral
//! form the pages module reads and writes
//! (`contracts/notion-data-source-snapshot.v1.schema.json` and
//! `contracts/notion-table-changes.v1.schema.json`).
//!
//! The mapping is structural, property kind to property kind, and never
//! reads what a cell says: a select is a select, a formula is a formula
//! (read only, its last value as text), a kind Pages lacks keeps its Notion
//! kind and a short text. Every property and every cell carries a hash of
//! its neutral form, which is what "Notion changed it" means: Notion
//! re-serialising a value does not fake an edit.
//!
//! Reading a database is `read_tables.rs`; making changes is
//! `write_tables.rs`. This file is pure: Notion's JSON in, neutral values
//! out, and back.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use campbooks_kernel::blobs::BlobHash;

/// The media type of a [`Snapshot`] blob.
pub const SNAPSHOT_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-data-source-snapshot+json; version=1";

/// The media type of a [`ChangeDocument`] blob.
pub const CHANGES_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-table-changes+json; version=1";

/// The most properties one database is read with.
pub const MAX_PROPERTIES: usize = 200;

/// The most options one property is read with.
pub const MAX_OPTIONS: usize = 100;

/// The longest text one Notion rich-text item may hold, in characters.
pub const RICH_TEXT_LIMIT: usize = 2000;

/// The longest a display text of a read-only value is kept.
const MAX_DISPLAY: usize = 500;

/// The longest a link may be.
const MAX_LINK: usize = 2048;

/// The kinds whose values and columns go both ways.
pub const TWO_WAY: [&str; 11] = [
    "title",
    "rich_text",
    "number",
    "select",
    "multi_select",
    "status",
    "date",
    "checkbox",
    "url",
    "email",
    "phone_number",
];

/// The kinds a column can be made as at Notion by the API (a status column
/// cannot be created or have its groups changed there).
pub const CREATABLE: [&str; 9] = [
    "rich_text",
    "number",
    "select",
    "multi_select",
    "date",
    "checkbox",
    "url",
    "email",
    "phone_number",
];

/// Whether `kind` goes both ways.
pub fn two_way(kind: &str) -> bool {
    TWO_WAY.contains(&kind)
}

/// A hash of a neutral value, in hexadecimal: the bytes are the value's
/// canonical JSON (object keys sorted).
pub fn hash_of(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    BlobHash::of(&bytes).to_string()
}

/// An option of a select, multi-select or status property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionRef {
    /// Notion's id of the option.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colour (Notion's ten).
    pub color: String,
    /// A status option's group: `todo`, `doing` or `done`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A property of the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    /// Notion's id of the property.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its Notion kind (`title`, `rich_text`, `select`, `formula`, …;
    /// `other` for a kind this module does not know).
    pub kind: String,
    /// The options of a select, multi-select or status property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionRef>,
    /// A number's Notion format (`number`, `percent`, `dollar`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// A hash of the property as described here (name, kind, options and
    /// format).
    pub hash: String,
}

/// The database's properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    /// A hash of every property's id and hash.
    pub hash: String,
    /// The properties, the title first.
    pub properties: Vec<Property>,
}

/// One cell of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// Notion's id of the property.
    pub property: String,
    /// The value in the neutral form of the property's kind; `null` for
    /// none.
    pub value: Value,
    /// A hash of the value.
    pub hash: String,
}

/// An entry (a row).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The entry's page at Notion.
    pub id: Uuid,
    /// When Notion last edited it.
    pub edited: DateTime<Utc>,
    /// Its cells.
    pub cells: Vec<Cell>,
}

/// A database as one run read it
/// (`contracts/notion-data-source-snapshot.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Always 1.
    pub version: u32,
    /// The data source at Notion.
    pub data_source: Uuid,
    /// The database's title.
    pub title: String,
    /// When it was read.
    pub read_at: DateTime<Utc>,
    /// The properties, when they changed since the last read (or on a first
    /// read).
    #[serde(default)]
    pub schema: Option<Schema>,
    /// The entries edited since the last read.
    pub entries: Vec<Entry>,
    /// Whether `present` lists every entry the database has.
    #[serde(default)]
    pub sweep: bool,
    /// With `sweep`: the id of every live entry.
    #[serde(default)]
    pub present: Option<Vec<Uuid>>,
}

/// The `plain_text` of a Notion rich-text array.
pub fn plain(items: Option<&Value>) -> String {
    items
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("plain_text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn safe_link(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    href.len() <= MAX_LINK
        && !href.chars().any(|c| c.is_whitespace() || c.is_control())
        && (lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:"))
}

/// A Notion rich-text array as neutral runs.
pub fn runs(items: Option<&Value>) -> Value {
    let Some(items) = items.and_then(Value::as_array) else {
        return Value::Null;
    };
    let mut out = Vec::new();
    for item in items {
        let text = item
            .get("plain_text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        let mut run = Map::new();
        run.insert("text".into(), Value::String(text.to_owned()));
        for (mark, key) in [
            ("bold", "bold"),
            ("italic", "italic"),
            ("strike", "strikethrough"),
            ("code", "code"),
        ] {
            if item
                .pointer(&format!("/annotations/{key}"))
                .and_then(Value::as_bool)
                == Some(true)
            {
                run.insert(mark.into(), Value::Bool(true));
            }
        }
        if let Some(href) = item
            .get("href")
            .and_then(Value::as_str)
            .filter(|href| safe_link(href))
        {
            run.insert("href".into(), Value::String(href.to_owned()));
        }
        out.push(Value::Object(run));
    }
    if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    }
}

fn clean_label(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn status_group(raw: &str) -> &'static str {
    match raw {
        "to_do" => "todo",
        "in_progress" => "doing",
        _ => "done",
    }
}

/// The options Notion lists under `config`, for a status property with the
/// groups it puts them in.
fn options_of(kind: &str, config: &Value) -> Vec<OptionRef> {
    let groups: Vec<(String, String)> = config
        .get("groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .flat_map(|group| {
                    let name = group
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let which = match name.as_str() {
                        "to-do" | "to do" | "not started" => "to_do",
                        "in progress" => "in_progress",
                        _ => "complete",
                    };
                    group
                        .get("option_ids")
                        .and_then(Value::as_array)
                        .map(|ids| {
                            ids.iter()
                                .filter_map(Value::as_str)
                                .map(|id| (id.to_owned(), which.to_owned()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    config
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|option| {
                    let id = option.get("id").and_then(Value::as_str)?;
                    let name = clean_label(option.get("name").and_then(Value::as_str)?, 100);
                    if name.is_empty() || id.len() > 200 {
                        return None;
                    }
                    let color = option
                        .get("color")
                        .and_then(Value::as_str)
                        .unwrap_or("default")
                        .to_owned();
                    let group = (kind == "status").then(|| {
                        let raw = groups
                            .iter()
                            .find(|(option_id, _)| option_id == id)
                            .map_or("to_do", |(_, group)| group.as_str());
                        status_group(raw).to_owned()
                    });
                    Some(OptionRef {
                        id: id.to_owned(),
                        name,
                        color,
                        group,
                    })
                })
                .take(MAX_OPTIONS)
                .collect()
        })
        .unwrap_or_default()
}

/// A property of a data source (or of a page: the same shape without the
/// settings), as much of it as is used.
pub fn property_of(name: &str, raw: &Value) -> Option<Property> {
    let id = raw.get("id").and_then(Value::as_str)?;
    if id.is_empty() || id.len() > 200 {
        return None;
    }
    let notion_kind = raw.get("type").and_then(Value::as_str)?;
    let kind = match notion_kind {
        "title" | "rich_text" | "number" | "select" | "multi_select" | "status" | "date"
        | "checkbox" | "url" | "email" | "phone_number" | "people" | "formula" | "rollup"
        | "files" | "relation" | "created_time" | "created_by" | "last_edited_time"
        | "last_edited_by" | "unique_id" | "verification" | "button" | "place" => notion_kind,
        _ => "other",
    };
    let config = raw.get(notion_kind).cloned().unwrap_or(Value::Null);
    let options = if matches!(kind, "select" | "multi_select" | "status") {
        options_of(kind, &config)
    } else {
        Vec::new()
    };
    let format = (kind == "number").then(|| {
        config
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("number")
            .to_owned()
    });
    let name = clean_label(name, 100);
    let name = if name.is_empty() {
        "Untitled".to_owned()
    } else {
        name
    };
    let described = json!({
        "name": name, "kind": kind, "format": format,
        "options": options.iter().map(|o| json!({
            "id": o.id, "name": o.name, "color": o.color, "group": o.group,
        })).collect::<Vec<_>>(),
    });
    Some(Property {
        id: id.to_owned(),
        name,
        kind: kind.to_owned(),
        options,
        format,
        hash: hash_of(&described),
    })
}

/// The schema of a data source object.
pub fn schema_of(raw: &Value) -> Schema {
    let mut properties: Vec<Property> = raw
        .get("properties")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, property)| property_of(name, property))
                .take(MAX_PROPERTIES)
                .collect()
        })
        .unwrap_or_default();
    // The title first, the rest by name (Notion gives no order).
    properties.sort_by(|a, b| {
        (a.kind != "title", &a.name, &a.id).cmp(&(b.kind != "title", &b.name, &b.id))
    });
    let listed: Vec<Value> = properties.iter().map(|p| json!([p.id, p.hash])).collect();
    Schema {
        hash: hash_of(&Value::Array(listed)),
        properties,
    }
}

/// A date value, as `{start, end?, time_zone?}`.
fn date_of(raw: &Value) -> Value {
    let Some(start) = raw.get("start").and_then(Value::as_str) else {
        return Value::Null;
    };
    let mut out = Map::new();
    out.insert("start".into(), Value::String(start.to_owned()));
    if let Some(end) = raw.get("end").and_then(Value::as_str) {
        out.insert("end".into(), Value::String(end.to_owned()));
    }
    if let Some(zone) = raw.get("time_zone").and_then(Value::as_str) {
        out.insert("time_zone".into(), Value::String(zone.to_owned()));
    }
    Value::Object(out)
}

fn option_value(raw: &Value) -> Value {
    match (
        raw.get("id").and_then(Value::as_str),
        raw.get("name").and_then(Value::as_str),
    ) {
        (Some(id), Some(name)) => json!({"id": id, "name": clean_label(name, 100)}),
        _ => Value::Null,
    }
}

fn display(text: &str) -> Value {
    let text = clean_label(text, MAX_DISPLAY);
    if text.is_empty() {
        Value::Null
    } else {
        Value::String(text)
    }
}

fn user_names(raw: &Value) -> Vec<String> {
    let one = |user: &Value| {
        let name = clean_label(
            user.get("name").and_then(Value::as_str).unwrap_or_default(),
            100,
        );
        (!name.is_empty()).then_some(name)
    };
    match raw {
        Value::Array(users) => users.iter().filter_map(one).collect(),
        other => one(other).into_iter().collect(),
    }
}

fn formula_text(raw: &Value) -> Value {
    match raw.get("type").and_then(Value::as_str) {
        Some("string") => display(
            raw.get("string")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ),
        Some("number") => raw
            .get("number")
            .filter(|n| n.is_number())
            .map_or(Value::Null, |n| Value::String(n.to_string())),
        Some("boolean") => raw
            .get("boolean")
            .and_then(Value::as_bool)
            .map_or(Value::Null, |b| Value::String(b.to_string())),
        Some("date") => raw
            .pointer("/date/start")
            .and_then(Value::as_str)
            .map_or(Value::Null, |s| Value::String(s.to_owned())),
        _ => Value::Null,
    }
}

fn rollup_text(raw: &Value) -> Value {
    match raw.get("type").and_then(Value::as_str) {
        Some("number") => raw
            .get("number")
            .filter(|n| n.is_number())
            .map_or(Value::Null, |n| Value::String(n.to_string())),
        Some("date") => raw
            .pointer("/date/start")
            .and_then(Value::as_str)
            .map_or(Value::Null, |s| Value::String(s.to_owned())),
        Some("array") => {
            let n = raw
                .get("array")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if n == 0 {
                Value::Null
            } else {
                Value::String(format!("{n}"))
            }
        }
        _ => Value::Null,
    }
}

/// The neutral value of a page's property `raw` of Notion kind `kind`.
pub fn value_of(kind: &str, raw: &Value) -> Value {
    let inner = raw.get(raw.get("type").and_then(Value::as_str).unwrap_or(kind));
    let inner = inner.unwrap_or(&Value::Null);
    match kind {
        "title" | "rich_text" => runs(Some(inner)),
        "number" => {
            if inner.is_number() {
                inner.clone()
            } else {
                Value::Null
            }
        }
        "select" | "status" => {
            if inner.is_null() {
                Value::Null
            } else {
                option_value(inner)
            }
        }
        "multi_select" => {
            let items: Vec<Value> = inner
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(option_value)
                        .filter(|o| !o.is_null())
                        .collect()
                })
                .unwrap_or_default();
            if items.is_empty() {
                Value::Null
            } else {
                Value::Array(items)
            }
        }
        "date" => date_of(inner),
        "checkbox" => Value::Bool(inner.as_bool().unwrap_or(false)),
        "url" | "email" | "phone_number" => inner
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map_or(Value::Null, |text| Value::String(text.trim().to_owned())),
        "people" | "created_by" | "last_edited_by" => {
            let names = user_names(inner);
            if names.is_empty() {
                Value::Null
            } else {
                json!(names)
            }
        }
        "formula" => formula_text(inner),
        "rollup" => rollup_text(inner),
        "created_time" | "last_edited_time" => inner
            .as_str()
            .map_or(Value::Null, |text| Value::String(text.to_owned())),
        "unique_id" => {
            let number = inner.get("number").and_then(Value::as_i64);
            match (inner.get("prefix").and_then(Value::as_str), number) {
                (Some(prefix), Some(n)) if !prefix.is_empty() => {
                    Value::String(format!("{prefix}-{n}"))
                }
                (_, Some(n)) => Value::String(n.to_string()),
                _ => Value::Null,
            }
        }
        "files" => {
            let names: Vec<String> = inner
                .as_array()
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|file| file.get("name").and_then(Value::as_str))
                        .map(|name| clean_label(name, 100))
                        .collect()
                })
                .unwrap_or_default();
            display(&names.join(", "))
        }
        // Relations, buttons, places, verification and kinds not known are
        // kept at Notion: nothing of them is read.
        _ => Value::Null,
    }
}

/// An entry's cells, those of `schema`'s properties the page has.
pub fn entry_of(raw: &Value, schema: &Schema) -> Option<Entry> {
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())?;
    let edited = raw
        .get("last_edited_time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())?
        .with_timezone(&Utc);
    let properties = raw.get("properties").and_then(Value::as_object)?;
    let by_id: std::collections::HashMap<&str, &Value> = properties
        .values()
        .filter_map(|value| Some((value.get("id")?.as_str()?, value)))
        .collect();
    let cells = schema
        .properties
        .iter()
        .filter_map(|property| {
            let raw = by_id.get(property.id.as_str())?;
            let value = value_of(&property.kind, raw);
            Some(Cell {
                property: property.id.clone(),
                hash: hash_of(&value),
                value,
            })
        })
        .collect();
    Some(Entry { id, edited, cells })
}

/// Whether a page property value may be incomplete: Notion gives at most
/// 25 items of a list in a page object. (A relation or a rollup is not read
/// at all, so it is never refetched.)
pub fn may_be_truncated(kind: &str, raw: &Value) -> bool {
    let inner_len = |key: &str| raw.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    match kind {
        "title" | "rich_text" | "people" => inner_len(kind) >= 25,
        _ => false,
    }
}

// --- What is written ---------------------------------------------------------

/// The change document the pages module wrote
/// (`contracts/notion-table-changes.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChangeDocument {
    /// Always 1.
    pub version: u32,
    /// What each item writes, by its place in the request.
    pub items: Vec<ChangeItem>,
}

/// One item's content.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChangeItem {
    /// The item's place in the request.
    pub seq: u32,
    /// A column's name (add, update).
    #[serde(default)]
    pub name: Option<String>,
    /// A column's Notion kind (add, update) or a cell's.
    #[serde(default)]
    pub kind: Option<String>,
    /// A number's format (add, update).
    #[serde(default)]
    pub format: Option<String>,
    /// The options to add (add, update): names and colours.
    #[serde(default)]
    pub options: Vec<NewOption>,
    /// A cell's neutral value (cell update).
    #[serde(default)]
    pub value: Option<Value>,
    /// The cells of a new row.
    #[serde(default)]
    pub cells: Vec<NewCell>,
}

/// An option to add.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewOption {
    /// Its name.
    pub name: String,
    /// Its colour.
    #[serde(default)]
    pub color: Option<String>,
    /// A status option's group.
    #[serde(default)]
    pub group: Option<String>,
}

/// One cell of a new row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewCell {
    /// Notion's id of the property.
    pub property: String,
    /// Its Notion kind.
    pub kind: String,
    /// The neutral value.
    #[serde(default)]
    pub value: Value,
}

/// The colours Notion has.
const COLORS: [&str; 10] = [
    "default", "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

fn colour(color: Option<&str>) -> &'static str {
    COLORS
        .iter()
        .find(|known| Some(**known) == color)
        .copied()
        .unwrap_or("default")
}

fn rich_text_items(runs: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for run in runs.as_array().into_iter().flatten() {
        let text = run.get("text").and_then(Value::as_str).unwrap_or_default();
        let flag = |key: &str| run.get(key).and_then(Value::as_bool).unwrap_or(false);
        let link = run
            .get("href")
            .and_then(Value::as_str)
            .filter(|href| safe_link(href))
            .map(|href| json!({"url": href}));
        let chars: Vec<char> = text.chars().collect();
        for piece in chars.chunks(RICH_TEXT_LIMIT) {
            let content: String = piece.iter().collect();
            if content.is_empty() {
                continue;
            }
            out.push(json!({
                "type": "text",
                "text": {"content": content, "link": link},
                "annotations": {
                    "bold": flag("bold"), "italic": flag("italic"),
                    "strikethrough": flag("strike"), "code": flag("code"),
                    "underline": false, "color": "default",
                },
            }));
        }
    }
    out
}

/// `{key: value}`.
fn keyed(key: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert(key.to_owned(), value);
    Value::Object(map)
}

/// The payload that sets a page property of Notion kind `kind` to the
/// neutral `value`; `None` for a kind that is never written or a value of
/// the wrong shape.
pub fn value_payload(kind: &str, value: &Value, options: &[OptionRef]) -> Option<Value> {
    let by_name = |name: &str| {
        let name = name.trim().to_lowercase();
        options
            .iter()
            .find(|o| o.name.trim().to_lowercase() == name)
    };
    match kind {
        "title" => Some(json!({"title": rich_text_items(value)})),
        "rich_text" => Some(json!({"rich_text": rich_text_items(value)})),
        "number" => match value {
            Value::Null => Some(json!({"number": null})),
            Value::Number(_) => Some(json!({"number": value})),
            _ => None,
        },
        "select" | "status" => match value {
            Value::Null if kind == "select" => Some(json!({"select": null})),
            Value::Null => None,
            other => {
                let name = other.get("name").and_then(Value::as_str)?;
                // A status can only be one of its options.
                if kind == "status" && by_name(name).is_none() {
                    return None;
                }
                Some(keyed(kind, json!({"name": name})))
            }
        },
        "multi_select" => match value {
            Value::Null => Some(json!({"multi_select": []})),
            Value::Array(items) => {
                let names: Vec<Value> = items
                    .iter()
                    .filter_map(|item| item.get("name").and_then(Value::as_str))
                    .map(|name| json!({"name": name}))
                    .collect();
                Some(json!({"multi_select": names}))
            }
            _ => None,
        },
        "date" => match value {
            Value::Null => Some(json!({"date": null})),
            Value::Object(_) => Some(json!({"date": value})),
            _ => None,
        },
        "checkbox" => Some(json!({"checkbox": value.as_bool().unwrap_or(false)})),
        "url" | "email" | "phone_number" => match value {
            Value::Null => Some(keyed(kind, Value::Null)),
            Value::String(text) => Some(keyed(kind, json!(text))),
            _ => None,
        },
        _ => None,
    }
}

/// The settings of a column of Notion kind `kind` as the API takes them
/// when it is added, or its options added to.
pub fn column_settings(
    kind: &str,
    format: Option<&str>,
    options: &[NewOption],
    keep: &[OptionRef],
) -> Option<Value> {
    match kind {
        "rich_text" | "date" | "checkbox" | "url" | "email" | "phone_number" => {
            Some(keyed(kind, json!({})))
        }
        "number" => Some(json!({"number": {"format": format.unwrap_or("number")}})),
        "select" | "multi_select" => {
            // The API cannot rename an option and removes what is omitted:
            // the options Notion has are kept by id, the new ones added.
            let mut listed: Vec<Value> = keep.iter().map(|o| json!({"id": o.id})).collect();
            for option in options {
                if keep
                    .iter()
                    .any(|o| o.name.trim().eq_ignore_ascii_case(option.name.trim()))
                {
                    continue;
                }
                listed.push(json!({
                    "name": option.name, "color": colour(option.color.as_deref()),
                }));
            }
            Some(keyed(kind, json!({"options": listed})))
        }
        _ => None,
    }
}

/// Whether the API can add options to a column of `kind`.
pub fn options_addable(kind: &str) -> bool {
    matches!(kind, "select" | "multi_select")
}

#[cfg(test)]
mod tests;
