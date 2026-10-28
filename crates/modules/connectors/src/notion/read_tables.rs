//! Reading a Notion database into a [`Snapshot`] (campbooks-cloud#377).
//!
//! A run asks Notion for the data source itself (one call: its properties),
//! and reads its entries edited since the cursor, oldest first, a hundred a
//! call, each call taking the connection's budget. A first read, or one
//! after the properties changed, reads every entry. Now and then (and when
//! the webhook woke the stream) the ids of every entry are listed too, so an
//! entry that went away is told from one that did not change. The values of
//! a long list (more than 25 titles, texts or people) are read in full.
//!
//! Nothing here is stored: the caller keeps the snapshot as a blob and
//! announces it.

#![allow(
    clippy::similar_names,
    reason = "a Notion page and the budget's pace are different things"
)]

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{
    Notion,
    data_source::DataSourceInfo,
    databases::{self, Entry, Schema, Snapshot},
    pace::{Pace, Stop},
};

/// Most entries one run reads in full; more come in the next run.
pub const RUN_ENTRIES: usize = 1000;

/// Most pages of one long value read.
const MAX_VALUE_PAGES: usize = 4;

/// How often the ids of every entry are listed.
pub const SWEEP_EVERY: TimeDelta = TimeDelta::hours(1);

/// What a data source's stream remembers between runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cursor {
    /// The newest edit time of an entry read.
    #[serde(default)]
    pub edited: Option<DateTime<Utc>>,
    /// The entries read that were edited at exactly that time (Notion's
    /// times are to the minute, and a query is "at or after": these are not
    /// read again).
    #[serde(default)]
    pub seen_at: Vec<Uuid>,
    /// The hash of the properties as last read.
    #[serde(default)]
    pub schema: Option<String>,
    /// A webhook asked for a read, whatever the edit times say.
    #[serde(default)]
    pub force: bool,
    /// When the ids of every entry were last listed.
    #[serde(default)]
    pub swept: Option<DateTime<Utc>>,
}

/// What a read gave.
#[derive(Debug)]
pub struct Read {
    /// The snapshot.
    pub snapshot: Snapshot,
    /// The cursor to keep once the snapshot is applied.
    pub cursor: Cursor,
    /// More entries wait: run again at once.
    pub more: bool,
    /// How many entries the snapshot holds.
    pub entries: usize,
}

/// Read the data source `info` is about: `info` was just retrieved. `first`
/// is the stream's first run.
pub async fn read(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    info: &DataSourceInfo,
    cursor: &Cursor,
    first: bool,
    now: DateTime<Utc>,
) -> Result<Read, Stop> {
    let schema_changed = cursor.schema.as_deref() != Some(info.schema.hash.as_str());
    let full = first || schema_changed;
    let since = if full { None } else { cursor.edited };
    let mut entries: Vec<Entry> = Vec::new();
    let mut next: Option<String> = None;
    let mut newest = cursor.edited;
    let mut more = false;
    let mut listed_all = false;
    loop {
        pace.take().await?;
        let page = notion
            .query_entries(token, info.id, since, None, next.as_deref())
            .await?;
        for raw in &page.entries {
            let known = !full
                && raw
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .is_some_and(|id| cursor.seen_at.contains(&id))
                && raw
                    .get("last_edited_time")
                    .and_then(Value::as_str)
                    .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
                    .map(|time| time.with_timezone(&Utc))
                    == cursor.edited;
            if known {
                continue;
            }
            let raw = complete(notion, token, pace, raw, &info.schema).await?;
            if let Some(entry) = databases::entry_of(&raw, &info.schema) {
                newest = newest.max(Some(entry.edited));
                entries.push(entry);
            }
        }
        next = page.next;
        if next.is_none() {
            listed_all = true;
            break;
        }
        if entries.len() >= RUN_ENTRIES {
            more = true;
            break;
        }
    }
    let due_sweep = cursor.swept.is_none_or(|at| now - at >= SWEEP_EVERY) || cursor.force;
    let (sweep, present, swept) = if full && listed_all && since.is_none() {
        // Every entry was read: they are the ones there are.
        (
            true,
            Some(entries.iter().map(|e| e.id).collect()),
            Some(now),
        )
    } else if due_sweep && !more {
        (
            true,
            Some(list_ids(notion, token, pace, info).await?),
            Some(now),
        )
    } else {
        (false, None, cursor.swept)
    };
    let count = entries.len();
    let entries_seen: Vec<(Uuid, DateTime<Utc>)> =
        entries.iter().map(|e| (e.id, e.edited)).collect();
    Ok(Read {
        snapshot: Snapshot {
            version: 1,
            data_source: info.id,
            title: info.title.clone(),
            read_at: now,
            schema: (schema_changed || first).then(|| info.schema.clone()),
            entries,
            sweep,
            present,
        },
        cursor: Cursor {
            seen_at: seen_at(&entries_seen, newest, cursor),
            edited: newest,
            schema: Some(info.schema.hash.clone()),
            force: false,
            swept,
        },
        more,
        entries: count,
    })
}

/// The entries edited at exactly `newest`: those just read, and those the
/// cursor already knew when `newest` did not move.
fn seen_at(
    read: &[(Uuid, DateTime<Utc>)],
    newest: Option<DateTime<Utc>>,
    cursor: &Cursor,
) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = if newest == cursor.edited {
        cursor.seen_at.clone()
    } else {
        Vec::new()
    };
    for (id, edited) in read {
        if Some(*edited) == newest && !ids.contains(id) {
            ids.push(*id);
        }
    }
    ids.truncate(RUN_ENTRIES);
    ids
}

/// The id of every live entry, read as titles only.
async fn list_ids(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    info: &DataSourceInfo,
) -> Result<Vec<Uuid>, Stop> {
    let title = info
        .schema
        .properties
        .iter()
        .find(|p| p.kind == "title")
        .map(|p| p.id.clone());
    let mut ids = Vec::new();
    let mut next: Option<String> = None;
    loop {
        pace.take().await?;
        let page = notion
            .query_entries(token, info.id, None, title.as_deref(), next.as_deref())
            .await?;
        ids.extend(page.entries.iter().filter_map(|raw| {
            raw.get("id")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
        }));
        next = page.next;
        if next.is_none() || ids.len() >= 20_000 {
            return Ok(ids);
        }
    }
}

/// `raw` with the long lists it holds read in full.
async fn complete(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    raw: &Value,
    schema: &Schema,
) -> Result<Value, Stop> {
    let mut raw = raw.clone();
    let Some(page) = raw
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return Ok(raw);
    };
    let mut long: Vec<(String, String, String)> = Vec::new();
    if let Some(properties) = raw.get("properties").and_then(Value::as_object) {
        for (name, value) in properties {
            let Some(id) = value.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(property) = schema.properties.iter().find(|p| p.id == id) else {
                continue;
            };
            if databases::may_be_truncated(&property.kind, value) {
                long.push((name.clone(), id.to_owned(), property.kind.clone()));
            }
        }
    }
    for (name, id, kind) in long {
        let mut items: Vec<Value> = Vec::new();
        let mut next: Option<String> = None;
        for _ in 0..MAX_VALUE_PAGES {
            pace.take().await?;
            let (answer, cursor) = notion
                .property_items(token, page, &id, next.as_deref())
                .await?;
            for item in answer
                .get("results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                // `{"type": "title", "title": {...}}`, one item per run.
                if let Some(inner) = item.get(kind.as_str()) {
                    items.push(inner.clone());
                }
            }
            next = cursor;
            if next.is_none() {
                break;
            }
        }
        if let Some(slot) = raw
            .get_mut("properties")
            .and_then(|p| p.get_mut(&name))
            .and_then(|p| p.get_mut(kind.as_str()))
        {
            *slot = Value::Array(items);
        }
    }
    Ok(raw)
}
