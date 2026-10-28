//! A Notion database in the neutral form the connectors module hands over
//! (`contracts/notion-data-source-snapshot.v1.schema.json`), and the
//! document this module writes for it to make
//! (`contracts/notion-table-changes.v1.schema.json`). Modules never import
//! each other: these are this module's own readings of the contracts.

use campbooks_kernel::blobs::BlobHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// The media type of a snapshot blob.
pub const SNAPSHOT_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-data-source-snapshot+json; version=1";

/// The media type of a change document blob.
pub const CHANGES_CONTENT_TYPE: &str =
    "application/vnd.campbooks.notion-table-changes+json; version=1";

/// The largest snapshot read, in bytes.
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;

/// An option of a select, multi-select or status property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NOption {
    /// Notion's id of the option.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colour.
    #[serde(default)]
    pub color: String,
    /// A status option's group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A property of the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NProperty {
    /// Notion's id of the property.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its Notion kind.
    pub kind: String,
    /// The options of a select, multi-select or status property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<NOption>,
    /// A number's Notion format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// The connector's hash of the property, in hexadecimal.
    pub hash: String,
}

/// The database's properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NSchema {
    /// A hash of every property.
    pub hash: String,
    /// The properties, the title first.
    pub properties: Vec<NProperty>,
}

/// One cell of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NCell {
    /// Notion's id of the property.
    pub property: String,
    /// The value in the neutral form of the property's kind.
    pub value: Value,
    /// The connector's hash of the value, in hexadecimal.
    pub hash: String,
}

/// An entry (a row).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NEntry {
    /// The entry's page at Notion.
    pub id: Uuid,
    /// When Notion last edited it.
    pub edited: DateTime<Utc>,
    /// Its cells.
    pub cells: Vec<NCell>,
}

/// A database as one run read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Always 1.
    pub version: u32,
    /// The data source at Notion.
    pub data_source: Uuid,
    /// The database's title.
    #[serde(default)]
    pub title: String,
    /// When it was read.
    pub read_at: DateTime<Utc>,
    /// The properties, when they changed since the last read.
    #[serde(default)]
    pub schema: Option<NSchema>,
    /// The entries edited since the last read.
    pub entries: Vec<NEntry>,
    /// Whether `present` lists every entry the database has.
    #[serde(default)]
    pub sweep: bool,
    /// With `sweep`: the id of every live entry.
    #[serde(default)]
    pub present: Option<Vec<Uuid>>,
}

/// The hash of a value in the connector's form: SHA-256 of its canonical
/// JSON (object keys sorted).
pub fn remote_hash(value: &Value) -> [u8; 32] {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    *BlobHash::of(&bytes).as_bytes()
}

/// The hash of a value in this module's form: the same encoding, over the
/// canonical value a cell holds. Empty and absent are one: `null`.
pub fn ours_hash(value: &Value) -> [u8; 32] {
    remote_hash(value)
}

/// A hash as hexadecimal.
pub fn hex(hash: &[u8; 32]) -> String {
    super::super::model::hash_hex(hash)
}

/// The hash `text` spells.
pub fn unhex(text: &str) -> Option<[u8; 32]> {
    super::super::model::hash_of_hex(text)
}

/// A hash a database column holds, if it is one.
pub fn bytes32(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}

/// The `notion-table-changes` document (what the connector writes).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ChangeDocument {
    /// Always 1.
    pub version: u32,
    /// What each item writes.
    pub items: Vec<ChangeItem>,
}

/// One item's content.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ChangeItem {
    /// The item's place in the request.
    pub seq: u32,
    /// A column's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A column's Notion kind, or a cell's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A number's Notion format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// The options to add.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<NewOption>,
    /// A cell's neutral value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// The cells of a new row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<NewCell>,
}

/// An option to add.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewOption {
    /// Its name.
    pub name: String,
    /// Its colour.
    pub color: String,
    /// A status option's group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// One cell of a new row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewCell {
    /// Notion's id of the property.
    pub property: String,
    /// Its Notion kind.
    pub kind: String,
    /// The neutral value.
    pub value: Value,
}
