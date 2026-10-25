//! `attention.project_conversation_need` — keep one item per conversation
//! from `conversations.need_changed`.
//!
//! The projection is idempotent and order-proof: an event applies only when
//! its revision is newer than the item's, so a replayed or late delivery
//! changes nothing. A clear that arrives before its open leaves a tombstone
//! (a cleared row with no reason) holding its revision, so the late open
//! cannot reopen it. Two deliveries racing for a conversation with no item
//! yet (parallel workers, a replay) find no row to lock: the write itself
//! only ever moves the revision forward, so the older one, landing second,
//! changes nothing. Rebuilding is replaying this handler from the log;
//! snoozes are the members' own and survive it, until the sender writes
//! again.

use campbooks_kernel::{
    Event, EventHandler, JobContext, JobError, RecordRef, tenancy::WorkspaceId,
};

use sqlx::{Postgres, Transaction};

use crate::{
    domain::ItemId,
    events::{ConversationNeedChanged, ItemChanged, NeedFacts},
};

/// Record types of what a conversation's need refers to.
const CONVERSATION: &str = "conversations.conversation";
const INBOX: &str = "conversations.inbox";
const MESSAGE: &str = "conversations.message";

/// Projects conversations' needs into items.
pub struct ProjectConversationNeed;

impl EventHandler for ProjectConversationNeed {
    type Event = ConversationNeedChanged;
    const NAME: &'static str = "attention.project_conversation_need";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<ConversationNeedChanged>,
    ) -> Result<(), JobError> {
        let need = event.payload;
        let workspace: WorkspaceId = need
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("need_changed names an invalid workspace"))?;
        let latest = need
            .latest_message
            .as_ref()
            .unwrap_or(&need.message)
            .clone();
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let current = sqlx::query!(
            r#"SELECT id AS "id: ItemId", producer_revision, latest_id FROM attention.items
                WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3 FOR UPDATE"#,
            workspace.as_uuid(),
            CONVERSATION,
            need.conversation,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if current
            .as_ref()
            .is_some_and(|row| row.producer_revision >= need.revision)
        {
            // Stale or replayed.
            return Ok(());
        }
        let id = current.as_ref().map_or_else(ItemId::new, |row| row.id);
        // The sender wrote again: every member's Later of this item ends.
        if let Some(row) = &current
            && row.latest_id.as_deref() != Some(latest.as_str())
        {
            sqlx::query!(
                "DELETE FROM attention.snoozes WHERE item_id = $1",
                row.id as ItemId
            )
            .execute(&mut **tx)
            .await?;
        }
        let written = if let Some(facts) = &need.need {
            open(tx, workspace, id, &need, facts, &latest).await?
        } else {
            // Nobody saw a tombstone: announce a clear only over a row that
            // was there, seen before (`current`) or written by a delivery
            // racing this one (`updated`).
            clear(tx, workspace, id, &need, &latest)
                .await?
                .filter(|written| current.is_some() || written.updated)
        };
        // The row's own id: in a race for a new item, the other delivery's.
        if let Some(written) = written {
            events
                .publish(
                    tx,
                    &ItemChanged {
                        workspace: need.workspace.clone(),
                        item: written.id,
                        container: RecordRef::new(INBOX, &need.inbox),
                    },
                )
                .await?;
        }
        Ok(())
    }
}

/// An item row an upsert wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
struct Written {
    /// Its id (another delivery's when it inserted the row first).
    id: ItemId,
    /// Whether it updated a row that was already there rather than
    /// inserting one.
    updated: bool,
}

/// Open (or update) item `id` with `facts`, keeping the facts its band is
/// worked out from (whether it is a warning, whether a person sent it): the
/// row written, if any (an item a newer revision reached first, in a race,
/// is left as it is).
async fn open(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    need: &ConversationNeedChanged,
    facts: &NeedFacts,
    latest: &str,
) -> Result<Option<Written>, JobError> {
    let written = sqlx::query_as::<_, Written>(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, latest_id, reason, trust, due_on, urgent, since,
             state, producer_revision, lapses_at, warning, from_person)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'open', $15, $16,
                $17, $18)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET container_type = EXCLUDED.container_type,
               container_id = EXCLUDED.container_id,
               cause_type = EXCLUDED.cause_type, cause_id = EXCLUDED.cause_id,
               latest_id = EXCLUDED.latest_id, reason = EXCLUDED.reason,
               trust = EXCLUDED.trust, due_on = EXCLUDED.due_on,
               lapses_at = EXCLUDED.lapses_at, warning = EXCLUDED.warning,
               from_person = EXCLUDED.from_person,
               urgent = EXCLUDED.urgent, since = EXCLUDED.since, state = 'open',
               cleared_because = NULL, cleared_by = NULL, cleared_at = NULL,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id, (xmax <> 0) AS updated
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(CONVERSATION)
    .bind(&need.conversation)
    .bind(INBOX)
    .bind(&need.inbox)
    .bind(MESSAGE)
    .bind(&need.message)
    .bind(latest)
    .bind(facts.reason.as_str())
    .bind(facts.trust.as_str())
    .bind(facts.due_on)
    .bind(facts.urgent)
    .bind(need.sent_at)
    .bind(need.revision)
    .bind(facts.lapses_at)
    .bind(facts.warning)
    .bind(facts.from_person)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written)
}

/// Clear item `id`; without one yet, leave a tombstone at this revision.
/// The row written, if any (an item a newer revision reached first, in a
/// race, is left as it is).
async fn clear(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    need: &ConversationNeedChanged,
    latest: &str,
) -> Result<Option<Written>, JobError> {
    let written = sqlx::query_as::<_, Written>(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, latest_id, since, state, cleared_because,
             cleared_by, cleared_at, producer_revision)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'cleared', $11, $12, now(), $13)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET state = 'cleared', cleared_because = EXCLUDED.cleared_because,
               cleared_by = EXCLUDED.cleared_by, cleared_at = now(),
               latest_id = EXCLUDED.latest_id,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id, (xmax <> 0) AS updated
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(CONVERSATION)
    .bind(&need.conversation)
    .bind(INBOX)
    .bind(&need.inbox)
    .bind(MESSAGE)
    .bind(&need.message)
    .bind(latest)
    .bind(need.sent_at)
    .bind(&need.cleared_because)
    .bind(need.cleared_by.map(|u| *u.as_uuid()))
    .bind(need.revision)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written)
}

#[cfg(test)]
mod tests;
