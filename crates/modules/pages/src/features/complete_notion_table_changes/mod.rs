//! `pages.complete_notion_table_changes` ←
//! `connectors.notion_table_changes_completed`: the connector made (or did
//! not make) the changes a request asked for, and the bases the last sync
//! agreed move to match (campbooks-cloud#377).
//!
//! A change that was applied moves its column's or cell's base to what was
//! written and the hash Notion answered with (a column added becomes the
//! same column as the one Notion made; one removed stops being mapped; a row
//! added is linked to the page Notion made, so its body syncs like a page).
//! A change that was skipped or failed leaves its base where it was: it
//! still differs from it, and what Notion changed meanwhile comes back with
//! the next read, as a conflict. Only when every change was applied is the
//! table looked at again at once.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};
use chrono::Utc;
use sqlx::types::Json;
use uuid::Uuid;

use crate::{
    domain::{
        PageId, PropertyId,
        notion::tables::{self, CellBase, model::unhex, requests::StoredItem},
        tracked,
    },
    events::{
        LinkKind, NotionLinkRequested, NotionOutcome, NotionTableChangesCompleted,
        NotionTableItemResult,
    },
};

/// Moves the bases for a request that ended.
pub struct CompleteNotionTableChanges;

impl EventHandler for CompleteNotionTableChanges {
    type Event = NotionTableChangesCompleted;
    const NAME: &'static str = "pages.complete_notion_table_changes";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<NotionTableChangesCompleted>,
    ) -> Result<(), JobError> {
        let done = event.payload;
        let workspace: WorkspaceId = done
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_table_changes_completed names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let waiting: Option<(Json<Vec<StoredItem>>,)> = sqlx::query_as(
            "SELECT items FROM pages.notion_table_requests
              WHERE id = $1 AND table_id = $2 AND state = 'waiting' FOR UPDATE",
        )
        .bind(done.request)
        .bind(done.table)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((items,)) = waiting else {
            return Ok(());
        };
        let items = items.0;
        sqlx::query(
            "UPDATE pages.notion_table_requests SET state = 'done', done_at = now() WHERE id = $1",
        )
        .bind(done.request)
        .execute(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owner(
                tx,
                &workspace.to_string(),
                &campbooks_kernel::blobs::BlobOwner::new(tables::REQUEST_OWNER, done.request),
            )
            .await?;
        let Some(ext) = tables::lock_external(tx, done.table).await? else {
            return Ok(());
        };
        let mut applied = 0_usize;
        for (item, result) in items.iter().zip(&done.results) {
            if result.subject != item.subject
                || result.kind != item.action
                || result.outcome != NotionOutcome::Applied
            {
                continue;
            }
            if write_base(tx, &state, workspace, done.table, &ext, item, result).await? {
                applied += 1;
            }
        }
        let all = applied == items.len() && done.results.len() == items.len();
        if applied > 0 {
            sqlx::query(
                "UPDATE pages.external_tables SET last_outbound_at = now() WHERE table_id = $1",
            )
            .bind(done.table)
            .execute(&mut **tx)
            .await?;
            tracked::notion_table_synced(
                &state,
                tx,
                workspace,
                done.table,
                tracked::Direction::Outbound,
                applied,
            )
            .await?;
        }
        if all {
            tables::schedule_push(tx, workspace, done.table).await?;
        }
        Ok(())
    }
}

/// Move the base of the change `item` made. Whether it did.
#[allow(clippy::too_many_lines, reason = "one arm per kind of change")]
async fn write_base(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    table: PageId,
    ext: &tables::ExternalTable,
    item: &StoredItem,
    result: &NotionTableItemResult,
) -> Result<bool, JobError> {
    let now = Utc::now();
    let edited = result.edited_at.unwrap_or(now);
    let remote = result.hash.as_deref().and_then(unhex);
    match item.action.as_str() {
        "schema_add" | "schema_update" => {
            let (Some(notion), Some(kind), Some(name), Some(type_h), Some(options_h)) = (
                result
                    .external
                    .as_deref()
                    .or(item.external_property.as_deref()),
                item.notion_kind.as_deref(),
                item.name.as_deref(),
                item.type_hash.as_deref().and_then(unhex),
                item.options_hash.as_deref().and_then(unhex),
            ) else {
                return Ok(false);
            };
            let Some(remote) = remote else {
                return Ok(false);
            };
            tables::write_property_base(
                tx,
                workspace,
                table,
                PropertyId::from_uuid(item.subject),
                notion,
                kind,
                (name, &type_h, &options_h, &remote),
            )
            .await?;
            Ok(true)
        }
        "schema_remove" => {
            let property = PropertyId::from_uuid(item.subject);
            sqlx::query("DELETE FROM pages.external_properties WHERE property_id = $1")
                .bind(property)
                .execute(&mut **tx)
                .await?;
            sqlx::query("DELETE FROM pages.external_cells WHERE property_id = $1")
                .bind(property)
                .execute(&mut **tx)
                .await?;
            Ok(true)
        }
        "cell_update" => {
            let (Some(property), Some(ours), Some(remote)) =
                (item.property, item.hash.as_deref().and_then(unhex), remote)
            else {
                return Ok(false);
            };
            tables::write_cell_base(
                tx,
                workspace,
                table,
                PageId::from_uuid(item.subject),
                PropertyId::from_uuid(property),
                &CellBase {
                    ours,
                    theirs: remote,
                    edited,
                },
            )
            .await?;
            Ok(true)
        }
        "row_create" => {
            let Some(page) = result
                .external
                .as_deref()
                .and_then(|p| Uuid::parse_str(p).ok())
            else {
                return Ok(false);
            };
            let row = PageId::from_uuid(item.subject);
            let linked = sqlx::query(
                "INSERT INTO pages.external_links
                     (page_id, workspace_id, connection_id, notion_page_id, state, created_by)
                 VALUES ($1, $2, $3, $4, 'syncing', $5)
                 ON CONFLICT (page_id) DO UPDATE
                    SET notion_page_id = EXCLUDED.notion_page_id, state = 'syncing',
                        stopped_reason = NULL, stopped_at = NULL",
            )
            .bind(row)
            .bind(workspace.as_uuid())
            .bind(ext.connection_id)
            .bind(page)
            .bind(ext.created_by)
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if linked > 0 {
                state
                    .events
                    .publish(
                        tx,
                        &NotionLinkRequested {
                            workspace: workspace.to_string(),
                            page: row,
                            connection: ext.connection_id,
                            notion_page: page,
                            kind: LinkKind::Page,
                        },
                    )
                    .await?;
            }
            // What was sent is the base on this side; Notion's form of it
            // comes with the next read and settles without a conflict.
            let bases = tables::property_bases(tx, table).await?;
            let held: Vec<(PropertyId, serde_json::Value)> =
                sqlx::query_as("SELECT property_id, value FROM pages.row_values WHERE row_id = $1")
                    .bind(row)
                    .fetch_all(&mut **tx)
                    .await?;
            let title: String = sqlx::query_scalar("SELECT title FROM pages.pages WHERE id = $1")
                .bind(row)
                .fetch_one(&mut **tx)
                .await?;
            for base in bases.iter().filter(|b| !b.remove_confirmed) {
                let is_title = base.notion_type == "title";
                let two_way = matches!(
                    base.notion_type.as_str(),
                    "title"
                        | "rich_text"
                        | "number"
                        | "select"
                        | "multi_select"
                        | "status"
                        | "date"
                        | "checkbox"
                        | "url"
                        | "email"
                        | "phone_number"
                );
                if !two_way {
                    continue;
                }
                let value = if is_title {
                    serde_json::Value::String(title.clone())
                } else {
                    held.iter()
                        .find(|(p, _)| *p == base.property_id)
                        .map_or(serde_json::Value::Null, |(_, v)| v.clone())
                };
                tables::write_cell_base(
                    tx,
                    workspace,
                    table,
                    row,
                    base.property_id,
                    &CellBase {
                        ours: tables::apply::hash_of(&value),
                        theirs: tables::null_hash(),
                        edited,
                    },
                )
                .await?;
            }
            Ok(true)
        }
        "row_trash" => {
            sqlx::query("DELETE FROM pages.external_cells WHERE row_id = $1")
                .bind(PageId::from_uuid(item.subject))
                .execute(&mut **tx)
                .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
