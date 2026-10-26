//! Making one block change at Notion (campbooks-cloud#376).
//!
//! Only changes the pages module asked for and confirmed, bound to where
//! they are made (`changes.rs`), reach here. Each one is checked against
//! Notion first:
//!
//! - **Update**: the block is read, and one Notion edited since the last
//!   sync (its edit time is not the one the pages module recorded) is **not
//!   written**: it comes back as a conflict on the next read.
//! - **Append**: the block goes under the block or page it names, after the
//!   block it names; a block of the same request it goes under or after
//!   must have been made.
//! - **Delete**: only a block Campbooks mapped and the member deleted, and
//!   one that has not been edited since the last sync. **A page is never
//!   trashed**: a `child_page` or `child_database` block is refused before
//!   Notion is asked to remove anything.
//!
//! Nothing here archives or deletes a Notion page.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{
    Notion,
    blocks::{self, Block},
    changes::{Recorded, Written},
    pace::{Pace, Stop},
};
use crate::{events::NotionReason, provider::ProviderError};

/// How making a change ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    /// Written.
    Applied(Written),
    /// Not written, for this reason.
    Skipped(NotionReason),
    /// Refused by Notion.
    Failed(NotionReason),
}

/// The block types that are pages: never removed from here.
const PAGES: [&str; 2] = ["child_page", "child_database"];

/// When Notion last edited block `raw`.
fn edited(raw: &Value) -> Option<DateTime<Utc>> {
    raw.get("last_edited_time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.with_timezone(&Utc))
}

/// Whether Notion has the block in the trash.
fn trashed(raw: &Value) -> bool {
    raw.get("in_trash")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// What a failed call means for the change: a block that is not there is
/// skipped, a request Notion refuses fails, and the rest is trouble for the
/// run to deal with.
fn classify(err: ProviderError) -> Result<Made, Stop> {
    match err {
        ProviderError::NotFound => Ok(Made::Skipped(NotionReason::NotFound)),
        ProviderError::Unexpected(_) => Ok(Made::Failed(NotionReason::Rejected)),
        other => Err(Stop::Provider(other)),
    }
}

/// Whether the block at Notion was edited after the sync `base` records.
fn moved_on(raw: &Value, base: Option<DateTime<Utc>>) -> bool {
    match (edited(raw), base) {
        (Some(now), Some(base)) => now != base,
        // Without a time to compare there is nothing to protect: but a
        // block whose time cannot be read is not written over.
        _ => true,
    }
}

/// Make `row`. `item` is its block in the document the pages module stored
/// (`None` for a delete); `notion_page` is the page the stream reads; `made`
/// has the blocks added so far by their ids in the pages module, and gains
/// this one.
#[allow(
    clippy::implicit_hasher,
    reason = "the map is this module's own, never another hasher"
)]
pub async fn make(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    row: &Recorded,
    item: Option<&Block>,
    notion_page: Uuid,
    made: &mut HashMap<Uuid, Uuid>,
) -> Result<Made, Stop> {
    match row.kind.as_str() {
        "update" => update(notion, token, pace, row, item).await,
        "append" => append(notion, token, pace, row, item, notion_page, made).await,
        "delete" => delete(notion, token, pace, row).await,
        _ => Ok(Made::Skipped(NotionReason::Unsupported)),
    }
}

async fn update(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    row: &Recorded,
    item: Option<&Block>,
) -> Result<Made, Stop> {
    let (Some(id), Some(block)) = (row.notion_block_id, item) else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let Some(body) = blocks::update_json(block) else {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    };
    pace.take().await?;
    let current = match notion.block(token, id).await {
        Ok(current) => current,
        Err(err) => return classify(err),
    };
    if trashed(&current) {
        return Ok(Made::Skipped(NotionReason::NotFound));
    }
    if moved_on(&current, row.base_edited_at) {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    pace.take().await?;
    match notion.update_block(token, id, &body).await {
        Ok(written) => Ok(applied(&written, id)),
        Err(err) => classify(err),
    }
}

async fn append(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    row: &Recorded,
    item: Option<&Block>,
    notion_page: Uuid,
    made: &mut HashMap<Uuid, Uuid>,
) -> Result<Made, Stop> {
    let Some(block) = item else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let Some(body) = blocks::create_json(block) else {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    };
    // Under a block Notion has, or one of this request that was made, or
    // the page itself.
    let parent = match (row.parent_id, row.parent_block_id) {
        (Some(parent), _) => parent,
        (None, Some(block)) => match made.get(&block) {
            Some(parent) => *parent,
            // The block it goes under was not made: nothing says where.
            None => return Ok(Made::Skipped(NotionReason::NotFound)),
        },
        (None, None) => notion_page,
    };
    let after = match (row.after_notion_id, row.after_block_id) {
        (Some(after), _) => Some(after),
        (None, Some(block)) => match made.get(&block) {
            Some(after) => Some(*after),
            // The block it goes after was not made: nothing says where.
            None => return Ok(Made::Skipped(NotionReason::NotFound)),
        },
        (None, None) => None,
    };
    pace.take().await?;
    let created = match notion
        .append(token, parent, std::slice::from_ref(&body), after)
        .await
    {
        Ok(created) => created,
        Err(err) => return classify(err),
    };
    let Some(first) = created.first() else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    let Some(id) = first
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    made.insert(row.block_id, id);
    Ok(applied(first, id))
}

async fn delete(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    row: &Recorded,
) -> Result<Made, Stop> {
    let Some(id) = row.notion_block_id else {
        return Ok(Made::Failed(NotionReason::Rejected));
    };
    pace.take().await?;
    let current = match notion.block(token, id).await {
        Ok(current) => current,
        Err(err) => return classify(err),
    };
    if trashed(&current) {
        return Ok(Made::Skipped(NotionReason::NotFound));
    }
    // A page is never trashed from here, whatever the request says.
    let kind = current
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if PAGES.contains(&kind) {
        return Ok(Made::Skipped(NotionReason::Unsupported));
    }
    if moved_on(&current, row.base_edited_at) {
        return Ok(Made::Skipped(NotionReason::ChangedInNotion));
    }
    pace.take().await?;
    match notion.delete_block(token, id).await {
        Ok(removed) => Ok(applied(&removed, id)),
        Err(err) => classify(err),
    }
}

/// A write that went through: the block and the edit time Notion gave it.
fn applied(answer: &Value, id: Uuid) -> Made {
    Made::Applied(Written {
        notion_block: id,
        edited_at: edited(answer).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests;
