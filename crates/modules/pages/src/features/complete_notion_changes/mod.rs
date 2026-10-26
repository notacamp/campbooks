//! `pages.complete_notion_changes` ← `connectors.notion_changes_completed`:
//! the connector made (or did not make) the block changes a request asked
//! for, and the bases the last sync agreed move to match
//! (campbooks-cloud#376).
//!
//! A change that was applied moves its block's base to what was written and
//! the edit time Notion gave it (an added block becomes the same block as the
//! one Notion made; a removed one stops being mapped). A change that was
//! skipped or failed leaves its base where it was: the block still differs
//! from it, and what Notion changed meanwhile comes back with the next read,
//! as a conflict. Only when every change was applied is the page looked at
//! again at once (edits made while the request waited).

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use sqlx::types::Json;
use uuid::Uuid;

use crate::{
    domain::{
        PageId,
        notion::{self, model::hash_of_hex, requests::StoredItem},
        tracked,
    },
    events::{NotionChangesCompleted, NotionOutcome},
};

/// Moves the bases for a request that ended.
pub struct CompleteNotionChanges;

impl EventHandler for CompleteNotionChanges {
    type Event = NotionChangesCompleted;
    const NAME: &'static str = "pages.complete_notion_changes";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<NotionChangesCompleted>,
    ) -> Result<(), JobError> {
        let done = event.payload;
        let workspace: WorkspaceId = done
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_changes_completed names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let waiting: Option<(Json<Vec<StoredItem>>,)> = sqlx::query_as(
            "SELECT items FROM pages.notion_requests
              WHERE id = $1 AND page_id = $2 AND state = 'waiting' FOR UPDATE",
        )
        .bind(done.request)
        .bind(done.page)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((items,)) = waiting else {
            return Ok(());
        };
        let items = items.0;
        sqlx::query(
            "UPDATE pages.notion_requests SET state = 'done', done_at = now() WHERE id = $1",
        )
        .bind(done.request)
        .execute(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owner(
                tx,
                &workspace.to_string(),
                &campbooks_kernel::blobs::BlobOwner::new(notion::REQUEST_OWNER, done.request),
            )
            .await?;
        let now = Utc::now();
        let mut applied = 0_usize;
        for (item, result) in items.iter().zip(&done.results) {
            if result.block != item.bid || result.outcome != NotionOutcome::Applied {
                continue;
            }
            if write_base(
                tx,
                workspace,
                done.page,
                item,
                result.notion_block,
                result.edited_at,
                now,
            )
            .await?
            {
                applied += 1;
            }
        }
        let all = applied == items.len() && done.results.len() == items.len();
        if applied > 0 {
            sqlx::query(
                "UPDATE pages.external_links SET last_outbound_at = now() WHERE page_id = $1",
            )
            .bind(done.page)
            .execute(&mut **tx)
            .await?;
            tracked::notion_applied(
                &state,
                tx,
                workspace,
                done.page,
                tracked::Direction::Outbound,
                applied,
            )
            .await?;
        }
        if all {
            notion::schedule_push(tx, workspace, done.page, now).await?;
        }
        Ok(())
    }
}

/// Move the base of the block `item` changed. Whether it was.
async fn write_base(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    item: &StoredItem,
    notion_block: Option<Uuid>,
    edited_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<bool, JobError> {
    let edited = edited_at.unwrap_or(now);
    match item.action.as_str() {
        "update" => {
            let Some(hash) = hash_of_hex(&item.hash) else {
                return Ok(false);
            };
            let moved = sqlx::query(
                "UPDATE pages.external_blocks SET base_hash = $3, base_edited_at = $4
                  WHERE page_id = $1 AND bid = $2",
            )
            .bind(page)
            .bind(item.bid)
            .bind(hash.as_slice())
            .bind(edited)
            .execute(&mut **tx)
            .await?
            .rows_affected();
            Ok(moved > 0)
        }
        "append" => {
            let (Some(hash), Some(notion_block)) = (hash_of_hex(&item.hash), notion_block) else {
                return Ok(false);
            };
            // The same block as the one Notion made; whichever block held
            // that id before lets go.
            sqlx::query(
                "DELETE FROM pages.external_blocks
                  WHERE page_id = $1 AND notion_block_id = $2 AND bid <> $3",
            )
            .bind(page)
            .bind(notion_block)
            .bind(item.bid)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO pages.external_blocks
                     (page_id, workspace_id, bid, notion_block_id, kind, base_hash,
                      base_edited_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (page_id, bid) DO UPDATE
                    SET notion_block_id = EXCLUDED.notion_block_id, kind = EXCLUDED.kind,
                        base_hash = EXCLUDED.base_hash,
                        base_edited_at = EXCLUDED.base_edited_at",
            )
            .bind(page)
            .bind(workspace.as_uuid())
            .bind(item.bid)
            .bind(notion_block)
            .bind(&item.kind)
            .bind(hash.as_slice())
            .bind(edited)
            .execute(&mut **tx)
            .await?;
            Ok(true)
        }
        "delete" => {
            let gone =
                sqlx::query("DELETE FROM pages.external_blocks WHERE page_id = $1 AND bid = $2")
                    .bind(page)
                    .bind(item.bid)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected();
            Ok(gone > 0)
        }
        _ => Ok(false),
    }
}
