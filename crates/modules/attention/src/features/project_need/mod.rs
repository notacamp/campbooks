//! `attention.project_conversation_need` — keep one item per conversation
//! from `conversations.need_changed`.
//!
//! The projection is idempotent and order-proof: an event applies only when
//! its revision is newer than the item's, so a replayed or late delivery
//! changes nothing. A clear that arrives before its open leaves a tombstone
//! (a cleared row with no reason) holding its revision, so the late open
//! cannot reopen it. Rebuilding is replaying this handler from the log;
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
        let announce = if let Some(facts) = &need.need {
            open(tx, workspace, id, &need, facts, &latest).await?;
            true
        } else {
            clear(tx, workspace, id, &need, &latest).await?;
            // Nobody saw a tombstone; announce only a real item.
            current.is_some()
        };
        if announce {
            events
                .publish(
                    tx,
                    &ItemChanged {
                        workspace: need.workspace.clone(),
                        item: id,
                        container: RecordRef::new(INBOX, &need.inbox),
                    },
                )
                .await?;
        }
        Ok(())
    }
}

/// Open (or update) item `id` with `facts`.
async fn open(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    need: &ConversationNeedChanged,
    facts: &NeedFacts,
    latest: &str,
) -> Result<(), JobError> {
    sqlx::query!(
        r#"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, latest_id, reason, trust, due_on, urgent, since,
             state, producer_revision)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'open', $15)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET container_type = EXCLUDED.container_type,
               container_id = EXCLUDED.container_id,
               cause_type = EXCLUDED.cause_type, cause_id = EXCLUDED.cause_id,
               latest_id = EXCLUDED.latest_id, reason = EXCLUDED.reason,
               trust = EXCLUDED.trust, due_on = EXCLUDED.due_on,
               urgent = EXCLUDED.urgent, since = EXCLUDED.since, state = 'open',
               cleared_because = NULL, cleared_by = NULL, cleared_at = NULL,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
        "#,
        id as ItemId,
        workspace.as_uuid(),
        CONVERSATION,
        need.conversation,
        INBOX,
        need.inbox,
        MESSAGE,
        need.message,
        latest,
        facts.reason.as_str(),
        facts.trust.as_str(),
        facts.due_on,
        facts.urgent,
        need.sent_at,
        need.revision,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Clear item `id`; without one yet, leave a tombstone at this revision.
async fn clear(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    need: &ConversationNeedChanged,
    latest: &str,
) -> Result<(), JobError> {
    sqlx::query!(
        r#"
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
        "#,
        id as ItemId,
        workspace.as_uuid(),
        CONVERSATION,
        need.conversation,
        INBOX,
        need.inbox,
        MESSAGE,
        need.message,
        latest,
        need.sent_at,
        need.cleared_because,
        need.cleared_by.map(|u| *u.as_uuid()),
        need.revision,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
