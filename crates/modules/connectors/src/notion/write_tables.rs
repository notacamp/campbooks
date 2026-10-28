//! Making one table change at Notion (campbooks-cloud#377).
//!
//! Only changes the pages module asked for and confirmed, bound to where
//! they are made (`table_changes.rs`), reach here. Each one is checked
//! against Notion first, and none is written over what Notion changed:
//!
//! - **Add a column**: refused when Notion has a column of that name.
//! - **Change a column**: the column must be there, and the schema (or the
//!   column) must be as the last sync saw it; options are only ever added
//!   (the API renames and reorders none).
//! - **Remove a column**: only when the change carries the member's
//!   confirmation (`confirmed`), never the title, and only a column as the
//!   last sync saw it.
//! - **Add a row**: to this database only.
//! - **Set a cell**: the entry must be a row of this database, and the cell
//!   as the last sync saw it.
//! - **Trash a row**: the only trashing here, of an entry of this database
//!   that has not been edited since the last sync.
//!
//! **A database is never trashed or deleted**, and no call here can: the
//! client has none. Nothing in an error or a log carries a name or a value.

#![allow(
    clippy::similar_names,
    reason = "a Notion page and the budget's pace are different things"
)]

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{
    Notion,
    data_source::DataSourceInfo,
    databases::{self, ChangeItem, Property},
    pace::{Pace, Stop},
    table_changes::{Recorded, Written},
};
use crate::{events::NotionReason, provider::ProviderError};

/// How making a change ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Made {
    /// Written.
    Applied(Written),
    /// Not written, for this reason.
    Skipped(NotionReason),
    /// Refused by Notion.
    Failed(NotionReason),
}

/// What a run learned of Notion, so a change does not ask twice.
#[derive(Debug, Default)]
pub struct Run {
    schema: Option<DataSourceInfo>,
    entries: HashMap<Uuid, Value>,
}

impl Run {
    /// The data source as Notion has it now.
    async fn schema(
        &mut self,
        notion: &Notion,
        token: &str,
        pace: &mut Pace,
        data_source: Uuid,
    ) -> Result<&DataSourceInfo, Stop> {
        if self.schema.is_none() {
            pace.take().await?;
            self.schema = Some(notion.data_source(token, data_source).await?);
        }
        self.schema
            .as_ref()
            .ok_or_else(|| Stop::Provider(ProviderError::Unexpected("no schema".into())))
    }

    /// The data source is not as it was read.
    fn forget_schema(&mut self) {
        self.schema = None;
    }
}

/// What a failed call means for the change: a thing that is not there is
/// skipped, a request Notion refuses fails, and the rest is trouble for the
/// run to deal with.
fn classify(err: ProviderError) -> Result<Made, Stop> {
    match err {
        ProviderError::NotFound => Ok(Made::Skipped(NotionReason::NotFound)),
        ProviderError::Unexpected(_) => Ok(Made::Failed(NotionReason::Rejected)),
        other => Err(Stop::Provider(other)),
    }
}

fn edited(raw: &Value) -> Option<DateTime<Utc>> {
    raw.get("last_edited_time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.with_timezone(&Utc))
}

fn trashed(raw: &Value) -> bool {
    raw.get("in_trash")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether entry `raw` is a row of `data_source`.
fn belongs_to(raw: &Value, data_source: Uuid) -> bool {
    raw.pointer("/parent/data_source_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        == Some(data_source)
}

/// Make `row`. `item` is its content in the document the pages module
/// stored; `data_source` is the database the stream reads.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
pub async fn make(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    row: &Recorded,
    item: Option<&ChangeItem>,
    data_source: Uuid,
) -> Result<Made, Stop> {
    match row.kind.as_str() {
        "schema_add" => add_column(notion, token, pace, run, item, data_source).await,
        "schema_update" => update_column(notion, token, pace, run, row, item, data_source).await,
        "schema_remove" => remove_column(notion, token, pace, run, row, data_source).await,
        "row_create" => create_row(notion, token, pace, run, item, data_source).await,
        "cell_update" => update_cell(notion, token, pace, run, row, item, data_source).await,
        "row_trash" => trash_row(notion, token, pace, row, data_source).await,
        _ => Ok(Made::Skipped(NotionReason::Unsupported)),
    }
}

/// The property of `answer` (a data source object) named `name`.
fn named(answer: &Value, name: &str) -> Option<Property> {
    answer
        .get("properties")
        .and_then(Value::as_object)?
        .iter()
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name.trim()))
        .and_then(|(key, raw)| databases::property_of(key, raw))
}

fn written_column(property: &Property, answer: &Value) -> Written {
    Written {
        external: Some(property.id.clone()),
        edited_at: edited(answer),
        hash: Some(property.hash.clone()),
    }
}

async fn add_column(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    item: Option<&ChangeItem>,
    data_source: Uuid,
) -> Result<Made, Stop> {
    let Some(item) = item else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let (Some(name), Some(kind)) = (item.name.as_deref(), item.kind.as_deref()) else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    if !databases::CREATABLE.contains(&kind) {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    }
    let Some(settings) =
        databases::column_settings(kind, item.format.as_deref(), &item.options, &[])
    else {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    };
    let current = run.schema(notion, token, pace, data_source).await?;
    if current
        .schema
        .properties
        .iter()
        .any(|p| p.name.trim().eq_ignore_ascii_case(name.trim()))
    {
        // Notion has a column of that name: what it holds is Notion's.
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    let mut properties = Map::new();
    properties.insert(name.trim().to_owned(), settings);
    pace.take().await?;
    run.forget_schema();
    let answer = match notion
        .update_data_source_properties(token, data_source, &Value::Object(properties))
        .await
    {
        Ok(answer) => answer,
        Err(err) => return classify(err),
    };
    match named(&answer, name) {
        Some(property) => Ok(Made::Applied(written_column(&property, &answer))),
        None => Ok(Made::Failed(NotionReason::Rejected)),
    }
}

async fn update_column(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    row: &Recorded,
    item: Option<&ChangeItem>,
    data_source: Uuid,
) -> Result<Made, Stop> {
    let (Some(item), Some(external)) = (item, row.external_property.as_deref()) else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let current = run.schema(notion, token, pace, data_source).await?;
    let Some(property) = current
        .schema
        .properties
        .iter()
        .find(|p| p.id == external)
        .cloned()
    else {
        return Ok(Made::Skipped(NotionReason::NotFound));
    };
    // The whole schema as the last sync saw it, or at least this column.
    let moved = row.schema_hash.as_deref() != Some(current.schema.hash.as_str())
        && row.base_hash.as_deref() != Some(property.hash.as_str());
    if moved {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    let mut change = Map::new();
    if let Some(name) = item.name.as_deref().filter(|n| !n.trim().is_empty()) {
        change.insert("name".into(), Value::String(name.trim().to_owned()));
    }
    let kind = item.kind.as_deref().unwrap_or(&property.kind);
    if kind != property.kind {
        // A change of kind: only between kinds the API can make.
        if !databases::CREATABLE.contains(&kind)
            || !databases::CREATABLE.contains(&property.kind.as_str())
        {
            return Ok(Made::Skipped(NotionReason::Unsupported));
        }
        if let Some(Value::Object(settings)) =
            databases::column_settings(kind, item.format.as_deref(), &item.options, &[])
        {
            change.extend(settings);
        }
    } else if databases::options_addable(kind) && !item.options.is_empty() {
        if let Some(Value::Object(settings)) = databases::column_settings(
            kind,
            item.format.as_deref(),
            &item.options,
            &property.options,
        ) {
            change.extend(settings);
        }
    } else if kind == "number"
        && let Some(format) = item.format.as_deref()
        && property.format.as_deref() != Some(format)
        && let Some(Value::Object(settings)) =
            databases::column_settings(kind, Some(format), &[], &[])
    {
        change.extend(settings);
    }
    if change.is_empty() {
        // Nothing the API can change: the base moves on what Notion holds.
        return Ok(Made::Applied(Written {
            external: Some(property.id.clone()),
            edited_at: None,
            hash: Some(property.hash.clone()),
        }));
    }
    let mut properties = Map::new();
    properties.insert(external.to_owned(), Value::Object(change));
    pace.take().await?;
    run.forget_schema();
    let answer = match notion
        .update_data_source_properties(token, data_source, &Value::Object(properties))
        .await
    {
        Ok(answer) => answer,
        Err(err) => return classify(err),
    };
    let after = answer
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|map| {
            map.iter()
                .filter_map(|(key, raw)| databases::property_of(key, raw))
                .find(|p| p.id == external)
        });
    match after {
        Some(property) => Ok(Made::Applied(written_column(&property, &answer))),
        None => Ok(Made::Failed(NotionReason::Rejected)),
    }
}

async fn remove_column(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    row: &Recorded,
    data_source: Uuid,
) -> Result<Made, Stop> {
    // The member's confirmation, or nothing is sent.
    if !row.confirmed {
        return Ok(Made::Skipped(NotionReason::ConfirmationRequired));
    }
    let Some(external) = row.external_property.as_deref() else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let current = run.schema(notion, token, pace, data_source).await?;
    let Some(property) = current
        .schema
        .properties
        .iter()
        .find(|p| p.id == external)
        .cloned()
    else {
        return Ok(Made::Skipped(NotionReason::NotFound));
    };
    if property.kind == "title" {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    }
    // A column Notion changed since the last sync is kept.
    if row.base_hash.as_deref() != Some(property.hash.as_str()) {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    let mut properties = Map::new();
    properties.insert(external.to_owned(), Value::Null);
    pace.take().await?;
    run.forget_schema();
    match notion
        .update_data_source_properties(token, data_source, &Value::Object(properties))
        .await
    {
        Ok(answer) => Ok(Made::Applied(Written {
            external: Some(external.to_owned()),
            edited_at: edited(&answer),
            hash: None,
        })),
        Err(err) => classify(err),
    }
}

async fn create_row(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    item: Option<&ChangeItem>,
    data_source: Uuid,
) -> Result<Made, Stop> {
    let Some(item) = item else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let current = run.schema(notion, token, pace, data_source).await?;
    let mut properties = Map::new();
    for cell in &item.cells {
        let Some(property) = current
            .schema
            .properties
            .iter()
            .find(|p| p.id == cell.property)
        else {
            continue;
        };
        if property.kind != cell.kind || !databases::two_way(&cell.kind) {
            continue;
        }
        if let Some(payload) = databases::value_payload(&cell.kind, &cell.value, &property.options)
        {
            properties.insert(cell.property.clone(), payload);
        }
    }
    pace.take().await?;
    let answer = match notion
        .create_entry(token, data_source, &Value::Object(properties))
        .await
    {
        Ok(answer) => answer,
        Err(err) => return classify(err),
    };
    let Some(page) = answer
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    Ok(Made::Applied(Written {
        external: Some(page.to_string()),
        edited_at: edited(&answer),
        hash: None,
    }))
}

async fn update_cell(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    row: &Recorded,
    item: Option<&ChangeItem>,
    data_source: Uuid,
) -> Result<Made, Stop> {
    let (Some(item), Some(external), Some(page)) =
        (item, row.external_property.as_deref(), row.external_page)
    else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let Some(kind) = item.kind.as_deref().filter(|k| databases::two_way(k)) else {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    };
    let current = run.schema(notion, token, pace, data_source).await?;
    let schema_hash = current.schema.hash.clone();
    let Some(property) = current
        .schema
        .properties
        .iter()
        .find(|p| p.id == external)
        .cloned()
    else {
        return Ok(Made::Skipped(NotionReason::NotFound));
    };
    if property.kind != kind {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    let entry = match entry(notion, token, pace, run, page).await {
        Ok(entry) => entry,
        Err(stop) => return stop_or_skip(stop),
    };
    if trashed(&entry) || !belongs_to(&entry, data_source) {
        return Ok(Made::Skipped(NotionReason::NotFound));
    }
    // The cell as the last sync saw it.
    let schema = databases::Schema {
        hash: schema_hash,
        properties: vec![property.clone()],
    };
    let now_hash = databases::entry_of(&entry, &schema)
        .and_then(|e| e.cells.into_iter().find(|c| c.property == external))
        .map(|c| c.hash);
    if now_hash.as_deref() != row.base_hash.as_deref() {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    let value = item.value.clone().unwrap_or(Value::Null);
    let Some(payload) = databases::value_payload(kind, &value, &property.options) else {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    };
    let mut properties = Map::new();
    properties.insert(external.to_owned(), payload);
    pace.take().await?;
    let answer = match notion
        .update_entry(token, page, &Value::Object(properties))
        .await
    {
        Ok(answer) => answer,
        Err(err) => return classify(err),
    };
    run.entries.insert(page, answer.clone());
    let hash = databases::entry_of(&answer, &schema)
        .and_then(|e| e.cells.into_iter().find(|c| c.property == external))
        .map(|c| c.hash);
    Ok(Made::Applied(Written {
        external: Some(external.to_owned()),
        edited_at: edited(&answer),
        hash,
    }))
}

/// A page Notion can no longer find is a change skipped, not a run lost.
fn stop_or_skip(stop: Stop) -> Result<Made, Stop> {
    match stop {
        Stop::Provider(err) => classify(err),
        other => Err(other),
    }
}

async fn entry(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    run: &mut Run,
    page: Uuid,
) -> Result<Value, Stop> {
    if let Some(known) = run.entries.get(&page) {
        return Ok(known.clone());
    }
    pace.take().await?;
    let fetched = notion.entry(token, page).await?;
    run.entries.insert(page, fetched.clone());
    Ok(fetched)
}

async fn trash_row(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    row: &Recorded,
    data_source: Uuid,
) -> Result<Made, Stop> {
    let Some(page) = row.external_page else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    pace.take().await?;
    let current = match notion.entry(token, page).await {
        Ok(current) => current,
        Err(err) => return classify(err),
    };
    if trashed(&current) {
        return Ok(Made::Skipped(NotionReason::NotFound));
    }
    // Only a row of this database is ever trashed from here: never a page
    // that is somewhere else, whatever the request says.
    if !belongs_to(&current, data_source) {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    }
    if edited(&current) != row.base_edited_at || row.base_edited_at.is_none() {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    pace.take().await?;
    match notion.trash_entry(token, page).await {
        Ok(answer) => Ok(Made::Applied(Written {
            external: Some(page.to_string()),
            edited_at: edited(&answer),
            hash: None,
        })),
        Err(err) => classify(err),
    }
}

/// `{"seq": …}`'s item of a document, if any.
pub fn item_of(document: &databases::ChangeDocument, seq: u32) -> Option<&ChangeItem> {
    document.items.iter().find(|item| item.seq == seq)
}
