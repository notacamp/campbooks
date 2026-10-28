//! A table kept in step with a Notion database (campbooks-cloud#377): the
//! link, what the last sync agreed about each column and cell, the way
//! changes cross, and the conflicts.
//!
//! - **Inbound** (`apply`): the connectors module reads the database into a
//!   snapshot of neutral values (`model`); this module compares each column
//!   and cell with ours and the base the last sync agreed, and writes
//!   Notion's changes in (`convert` is the mapping of kinds and values).
//! - **Outbound** (`diff`, `requests`): a debounced job compares the table
//!   with the bases and records what changed as a request for the
//!   connectors module, bound with a MAC and confirmed back through the
//!   kernel's page-requests port before anything is written at Notion.
//! - **Conflicts** (`conflicts`): a column or a cell changed on both sides
//!   between two syncs waits for the member.
//! - **Stopping** ends a link, with a reason, and deletes nothing on either
//!   side.
//!
//! Nothing here reads what a value says to decide anything. Ids, hashes,
//! kinds and times are all that reach an event or a log.

pub mod apply;
pub mod conflicts;
pub mod convert;
pub mod diff;
pub mod model;
pub mod requests;

use std::collections::HashMap;

use campbooks_kernel::{
    AppState, Result,
    blobs::BlobOwner,
    jobs::{self, EnqueueOptions},
    tenancy::{Access, UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::{StopReason, model::hash_hex};
use crate::{
    domain::{PageId, PropertyId, announce, tracked},
    events::{NotionLinkEnded, PageChangeKind},
    features::push_table_to_notion::PushTableToNotion,
};

/// The blob owner type of a table request's document.
pub const REQUEST_OWNER: &str = "pages.notion_table_request";

/// How long a table rests after a change before it is sent.
pub const DEBOUNCE_SECONDS: i64 = 10;

/// Most items one request holds.
pub const MAX_ITEMS: usize = 200;

/// A table's link to its Notion database.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExternalTable {
    pub table_id: PageId,
    pub connection_id: Uuid,
    pub notion_data_source_id: Uuid,
    pub state: String,
    pub stopped_reason: Option<String>,
    pub created_by: UserId,
    pub conflicts: i32,
    pub last_inbound_at: Option<DateTime<Utc>>,
    pub last_outbound_at: Option<DateTime<Utc>>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub schema_hash: Option<String>,
    pub last_title: Option<String>,
}

impl ExternalTable {
    /// Whether the link is syncing.
    pub fn syncing(&self) -> bool {
        self.state == "syncing"
    }

    /// The reason the link ended, when it did.
    pub fn reason(&self) -> Option<StopReason> {
        Some(match self.stopped_reason.as_deref()? {
            "stopped_by_member" => StopReason::StoppedByMember,
            "disconnected" => StopReason::Disconnected,
            "trashed_here" => StopReason::TrashedHere,
            "trashed_in_notion" => StopReason::TrashedInNotion,
            "not_shared" => StopReason::NotShared,
            "member_left" => StopReason::MemberLeft,
            "notion_unavailable" => StopReason::NotionUnavailable,
            _ => return None,
        })
    }
}

const TABLE_COLUMNS: &str = "table_id, connection_id, notion_data_source_id, state, \
                             stopped_reason, created_by, conflicts, last_inbound_at, \
                             last_outbound_at, last_read_at, schema_hash, last_title";

/// The link of `table`, if it has one.
pub async fn external_of(conn: &mut PgConnection, table: PageId) -> Result<Option<ExternalTable>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TABLE_COLUMNS} FROM pages.external_tables WHERE table_id = $1"
    )))
    .bind(table)
    .fetch_optional(conn)
    .await?)
}

/// The link of `table`, locked for the change.
pub async fn lock_external(
    conn: &mut PgConnection,
    table: PageId,
) -> Result<Option<ExternalTable>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TABLE_COLUMNS} FROM pages.external_tables WHERE table_id = $1 FOR UPDATE"
    )))
    .bind(table)
    .fetch_optional(conn)
    .await?)
}

/// What the last sync agreed about a column.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PropertyBase {
    pub property_id: PropertyId,
    pub notion_property_id: String,
    pub notion_type: String,
    pub base_name: String,
    pub base_type_hash: Vec<u8>,
    pub base_options_hash: Vec<u8>,
    pub base_remote_hash: Vec<u8>,
    pub remove_confirmed: bool,
}

/// The bases of `table`'s columns.
pub async fn property_bases(conn: &mut PgConnection, table: PageId) -> Result<Vec<PropertyBase>> {
    Ok(sqlx::query_as(
        "SELECT property_id, notion_property_id, notion_type, base_name, base_type_hash,
                base_options_hash, base_remote_hash, remove_confirmed
           FROM pages.external_properties WHERE table_id = $1",
    )
    .bind(table)
    .fetch_all(conn)
    .await?)
}

/// The hash of a column's type, as the base keeps it.
pub fn type_hash(ty: crate::properties::PropertyType) -> [u8; 32] {
    *campbooks_kernel::blobs::BlobHash::of(ty.as_str().as_bytes()).as_bytes()
}

/// The hash of a column's config, as the base keeps it.
pub fn config_hash(config: &serde_json::Value) -> [u8; 32] {
    model::remote_hash(config)
}

/// Write a column's base.
#[allow(clippy::too_many_arguments, reason = "one row of the base")]
pub async fn write_property_base(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    table: PageId,
    property: PropertyId,
    notion_property: &str,
    notion_type: &str,
    (name, type_h, options_h, remote_h): (&str, &[u8; 32], &[u8; 32], &[u8; 32]),
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pages.external_properties
             (property_id, table_id, workspace_id, notion_property_id, notion_type, base_name,
              base_type_hash, base_options_hash, base_remote_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (property_id) DO UPDATE
            SET notion_property_id = EXCLUDED.notion_property_id,
                notion_type = EXCLUDED.notion_type, base_name = EXCLUDED.base_name,
                base_type_hash = EXCLUDED.base_type_hash,
                base_options_hash = EXCLUDED.base_options_hash,
                base_remote_hash = EXCLUDED.base_remote_hash, remove_confirmed = false",
    )
    .bind(property)
    .bind(table)
    .bind(workspace.as_uuid())
    .bind(notion_property)
    .bind(notion_type)
    .bind(name)
    .bind(type_h.as_slice())
    .bind(options_h.as_slice())
    .bind(remote_h.as_slice())
    .execute(conn)
    .await?;
    Ok(())
}

/// A cell's base: ours and theirs hashes, and the time Notion had it.
#[derive(Debug, Clone, Copy)]
pub struct CellBase {
    pub ours: [u8; 32],
    pub theirs: [u8; 32],
    pub edited: DateTime<Utc>,
}

/// The hash of nothing, in either form.
pub fn null_hash() -> [u8; 32] {
    model::remote_hash(&serde_json::Value::Null)
}

/// Write a cell's base.
pub async fn write_cell_base(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    table: PageId,
    row: PageId,
    property: PropertyId,
    base: &CellBase,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pages.external_cells
             (row_id, property_id, table_id, workspace_id, base_hash, base_remote_hash,
              base_edited_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (row_id, property_id) DO UPDATE
            SET base_hash = EXCLUDED.base_hash, base_remote_hash = EXCLUDED.base_remote_hash,
                base_edited_at = EXCLUDED.base_edited_at",
    )
    .bind(row)
    .bind(property)
    .bind(table)
    .bind(workspace.as_uuid())
    .bind(base.ours.as_slice())
    .bind(base.theirs.as_slice())
    .bind(base.edited)
    .execute(conn)
    .await?;
    Ok(())
}

/// A cell's base as the database keeps it.
type CellBaseRow = (PageId, PropertyId, Vec<u8>, Vec<u8>, DateTime<Utc>);

/// The bases of `table`'s cells, those of `rows` (all when none).
pub async fn cell_bases(
    conn: &mut PgConnection,
    table: PageId,
    rows: Option<&[PageId]>,
) -> Result<HashMap<(PageId, PropertyId), CellBase>> {
    let found: Vec<CellBaseRow> = sqlx::query_as(
        "SELECT row_id, property_id, base_hash, base_remote_hash, base_edited_at
           FROM pages.external_cells
          WHERE table_id = $1 AND ($2::uuid[] IS NULL OR row_id = ANY($2))",
    )
    .bind(table)
    .bind(rows)
    .fetch_all(conn)
    .await?;
    Ok(found
        .into_iter()
        .filter_map(|(row, property, ours, theirs, edited)| {
            Some((
                (row, property),
                CellBase {
                    ours: model::bytes32(&ours)?,
                    theirs: model::bytes32(&theirs)?,
                    edited,
                },
            ))
        })
        .collect())
}

/// How many conflicts of `table` wait for a choice.
pub async fn open_conflicts(conn: &mut PgConnection, table: PageId) -> Result<i32> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.table_conflicts WHERE table_id = $1 AND choice IS NULL",
    )
    .bind(table)
    .fetch_one(conn)
    .await?;
    Ok(i32::try_from(count).unwrap_or(i32::MAX))
}

/// End `table`'s link for `reason`: the table stays as it is, the requests
/// still waiting for the connector end, the rows' own page links end, the
/// connector is told to stop reading, and nothing is deleted at Notion.
/// `false` when there was no link, or it had ended already.
pub async fn stop_table(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    reason: StopReason,
) -> Result<bool> {
    let ended: Option<(Uuid, Uuid)> = sqlx::query_as(
        "UPDATE pages.external_tables
            SET state = 'stopped', stopped_reason = $2, stopped_at = now()
          WHERE table_id = $1 AND state <> 'stopped'
      RETURNING connection_id, notion_data_source_id",
    )
    .bind(table)
    .bind(reason.as_str())
    .fetch_optional(&mut **tx)
    .await?;
    let Some((connection, data_source)) = ended else {
        return Ok(false);
    };
    let waiting: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE pages.notion_table_requests SET state = 'done', done_at = now()
          WHERE table_id = $1 AND state = 'waiting' RETURNING id",
    )
    .bind(table)
    .fetch_all(&mut **tx)
    .await?;
    let key = workspace.to_string();
    for request in waiting {
        state
            .blobs
            .detach_owner(tx, &key, &BlobOwner::new(REQUEST_OWNER, request))
            .await?;
    }
    state
        .events
        .publish(
            tx,
            &NotionLinkEnded {
                workspace: key,
                page: table,
                connection,
                notion_page: data_source,
            },
        )
        .await?;
    // The rows' bodies are pages linked to Notion pages: they stop too.
    let rows: Vec<PageId> = sqlx::query_scalar(
        "SELECT l.page_id FROM pages.external_links l
           JOIN pages.table_rows tr ON tr.row_id = l.page_id
          WHERE tr.table_id = $1 AND l.state <> 'stopped' ORDER BY l.page_id",
    )
    .bind(table)
    .fetch_all(&mut **tx)
    .await?;
    for row in rows {
        super::stop_link(state, tx, workspace, row, reason).await?;
    }
    let access = Access {
        workspace,
        user: None,
        role: None,
    };
    announce(state, tx, &access, table, vec![PageChangeKind::Meta]).await?;
    tracked::notion_stopped(state, tx, workspace, table, reason).await?;
    Ok(true)
}

/// Ask for `table`'s changes to be looked at, [`DEBOUNCE_SECONDS`] from
/// now, when the table is kept in step with Notion. Costs one lookup for a
/// table that is not.
pub async fn schedule_push(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
) -> Result<()> {
    let syncing: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.external_tables
                         WHERE table_id = $1 AND state = 'syncing')",
    )
    .bind(table)
    .fetch_one(&mut **tx)
    .await?;
    if !syncing {
        return Ok(());
    }
    let at = Utc::now() + TimeDelta::seconds(DEBOUNCE_SECONDS);
    jobs::enqueue(
        tx,
        &PushTableToNotion { table },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!(
                "pages.push_table_to_notion:{table}:{}",
                at.timestamp() / DEBOUNCE_SECONDS
            ))
            .run_at(at),
    )
    .await?;
    Ok(())
}

/// A hash as the 64 hexadecimal digits requests carry.
pub fn hex(hash: &[u8; 32]) -> String {
    hash_hex(hash)
}

#[cfg(test)]
mod tests;
