//! `conversations.record_send` — the connector reports how a send ended
//! (`connectors.send_completed`): a sent draft is deleted, a failed one is
//! editable again with its reason. The sent message itself arrives in the
//! connector's change batch, like any message. A late outcome for a draft
//! that stopped waiting (`failed`, same send) still counts.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::UserId};

use crate::{
    events::{DraftSent, SendCompleted, SendFailed, SendOutcome},
    features::update_draft::discard,
    model::{ConversationId, DraftId, DraftState, InboxId, SendId},
};

/// Records send outcomes on drafts.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecordSend;

impl EventConsumer for RecordSend {
    type Event = SendCompleted;
    const NAME: &'static str = "conversations.record_send";
    const QUEUE: &'static str = "default";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<SendCompleted>,
    ) -> Result<(), JobError> {
        let completed = event.payload;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(draft) = sqlx::query!(
            r#"SELECT id AS "id: DraftId", inbox_id AS "inbox: InboxId",
                      conversation_id AS "conversation: ConversationId",
                      author AS "author: UserId"
                 FROM conversations.drafts
                WHERE send_id = $1 AND state IN ('sending', 'failed')
                  FOR UPDATE"#,
            completed.send_id as SendId,
        )
        .fetch_optional(&mut **tx)
        .await?
        else {
            return Ok(()); // Already recorded.
        };
        match completed.outcome {
            SendOutcome::Sent => {
                discard(&state, tx, &completed.workspace, draft.id).await?;
                state
                    .events
                    .publish(
                        tx,
                        &DraftSent {
                            workspace: completed.workspace,
                            inbox: draft.inbox,
                            draft: draft.id,
                            conversation: draft.conversation,
                            author: draft.author,
                        },
                    )
                    .await?;
            }
            SendOutcome::Failed => {
                let reason = completed
                    .reason
                    .unwrap_or_else(|| "unknown_outcome".to_owned());
                sqlx::query!(
                    r#"UPDATE conversations.drafts
                          SET state = $2, failure_reason = $3, send_at = NULL,
                              version = version + 1, updated_at = now()
                        WHERE id = $1"#,
                    draft.id as DraftId,
                    DraftState::Failed.as_str(),
                    reason,
                )
                .execute(&mut **tx)
                .await?;
                state
                    .events
                    .publish(
                        tx,
                        &SendFailed {
                            workspace: completed.workspace,
                            inbox: draft.inbox,
                            draft: draft.id,
                            reason,
                            author: draft.author,
                        },
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
