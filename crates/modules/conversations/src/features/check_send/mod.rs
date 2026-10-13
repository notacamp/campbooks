//! `conversations.check_send` — a released send that has heard nothing back
//! for [`CHECK_AFTER`] stops waiting: the draft becomes `failed` with
//! `unknown_outcome` (it may have gone; the member checks Sent), editable
//! again, instead of staying "sending" for ever. The connector settles every
//! send it recorded well before this; this covers an intent it never got.
//! A late outcome still wins (`conversations.record_send`).

use campbooks_kernel::{JobContext, JobError, JobHandler};
use chrono::TimeDelta;
use serde::{Deserialize, Serialize};

use crate::{
    events::SendFailed,
    model::{DraftId, DraftState, InboxId, SendId},
};

/// How long a released send may stay without an outcome.
pub const CHECK_AFTER: TimeDelta = TimeDelta::hours(1);

/// The job's payload: ids only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSendPayload {
    /// The draft.
    pub draft: DraftId,
    /// The send it was released as.
    pub send_id: SendId,
}

/// Gives up waiting on sends that never reported back.
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckSend;

impl JobHandler for CheckSend {
    type Payload = CheckSendPayload;
    const NAME: &'static str = "conversations.check_send";
    const QUEUE: &'static str = "default";

    async fn run(&self, ctx: &mut JobContext, payload: CheckSendPayload) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .ok_or_else(|| JobError::fatal("check_send enqueued outside a workspace"))?
            .to_owned();
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let reason = "unknown_outcome";
        let stuck = sqlx::query_scalar!(
            r#"UPDATE conversations.drafts
                  SET state = $3, failure_reason = $4, send_at = NULL,
                      version = version + 1, updated_at = now()
                WHERE id = $1 AND send_id = $2 AND state = 'sending'
            RETURNING inbox_id AS "inbox: InboxId""#,
            payload.draft as DraftId,
            payload.send_id as SendId,
            DraftState::Failed.as_str(),
            reason,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(inbox) = stuck {
            events
                .publish(
                    tx,
                    &SendFailed {
                        workspace,
                        inbox,
                        draft: payload.draft,
                        reason: reason.to_owned(),
                    },
                )
                .await?;
            tracing::warn!(draft = %payload.draft, send_id = %payload.send_id, "a send never reported back");
        }
        Ok(())
    }
}
