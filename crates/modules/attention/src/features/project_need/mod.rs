//! `attention.project_conversation_need` — keep one item per conversation
//! from `conversations.need_changed`.
//!
//! The projection is idempotent and order-proof: an event applies only when
//! its revision is newer than the item's, so a replayed or late delivery
//! changes nothing. Rebuilding is replaying this handler from the log;
//! snoozes are the members' own and survive it.

use campbooks_kernel::{
    Event, EventHandler, JobContext, JobError, RecordRef, tenancy::WorkspaceId,
};

use crate::{
    domain::ItemId,
    events::{ConversationNeedChanged, ItemChanged},
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
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let changed = match &need.need {
            Some(facts) => {
                // A new cause (a newer message) ends every snooze of the item.
                sqlx::query!(
                    r#"
                    DELETE FROM attention.snoozes s
                     USING attention.items i
                     WHERE s.item_id = i.id AND i.workspace_id = $1 AND i.record_type = $2
                       AND i.record_id = $3 AND i.cause_id <> $4 AND i.producer_revision < $5
                    "#,
                    workspace.as_uuid(),
                    CONVERSATION,
                    need.conversation,
                    need.message,
                    need.revision,
                )
                .execute(&mut **tx)
                .await?;
                sqlx::query_scalar!(
                    r#"
                    INSERT INTO attention.items
                        (id, workspace_id, record_type, record_id, container_type, container_id,
                         cause_type, cause_id, reason, trust, due_on, urgent, since, state,
                         producer_revision)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'open', $14)
                    ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
                       SET container_type = EXCLUDED.container_type,
                           container_id = EXCLUDED.container_id,
                           cause_type = EXCLUDED.cause_type, cause_id = EXCLUDED.cause_id,
                           reason = EXCLUDED.reason, trust = EXCLUDED.trust,
                           due_on = EXCLUDED.due_on, urgent = EXCLUDED.urgent,
                           since = EXCLUDED.since, state = 'open', cleared_because = NULL,
                           cleared_by = NULL, cleared_at = NULL,
                           producer_revision = EXCLUDED.producer_revision, updated_at = now()
                     WHERE attention.items.producer_revision < EXCLUDED.producer_revision
                    RETURNING id AS "id: ItemId"
                    "#,
                    ItemId::new() as ItemId,
                    workspace.as_uuid(),
                    CONVERSATION,
                    need.conversation,
                    INBOX,
                    need.inbox,
                    MESSAGE,
                    need.message,
                    facts.reason.as_str(),
                    facts.trust.as_str(),
                    facts.due_on,
                    facts.urgent,
                    need.sent_at,
                    need.revision,
                )
                .fetch_optional(&mut **tx)
                .await?
            }
            // Cleared: an item never seen open is not created.
            None => {
                sqlx::query_scalar!(
                    r#"
                    UPDATE attention.items
                       SET state = 'cleared', cleared_because = $4, cleared_by = $5,
                           cleared_at = now(), producer_revision = $6, updated_at = now()
                     WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3
                       AND producer_revision < $6
                    RETURNING id AS "id: ItemId"
                    "#,
                    workspace.as_uuid(),
                    CONVERSATION,
                    need.conversation,
                    need.cleared_because,
                    need.cleared_by.map(|u| *u.as_uuid()),
                    need.revision,
                )
                .fetch_optional(&mut **tx)
                .await?
            }
        };
        if let Some(item) = changed {
            events
                .publish(
                    tx,
                    &ItemChanged {
                        workspace: need.workspace.clone(),
                        item,
                        container: RecordRef::new(INBOX, &need.inbox),
                    },
                )
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
