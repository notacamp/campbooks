//! `conversations.refresh_need`: work a conversation's need out again.
//!
//! A first need is not stored while another transaction is changing its
//! message (see `domain::needs::refresh`); this job, enqueued in the same
//! transaction, settles it a few seconds later. One job per deferral, never
//! unique: a unique key covers a running job too, which would drop a
//! deferral that happens while one runs (and may have read before it
//! committed). Each run is an idempotent refresh, and the count is bounded
//! by the deferrals. The job waits for the locks (bounded by a lock
//! timeout), so a busy message does not use up its attempts; if the timeout
//! passes it retries later.

use std::time::Duration;

use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    jobs::{EnqueueOptions, Enqueued},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::needs::{self, NeedSettings, Refreshed},
    model::ConversationId,
};

/// How long after a skip the first attempt runs, and between attempts.
const DELAY: Duration = Duration::from_secs(10);

/// The conversation to work out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshNeedPayload {
    /// The conversation.
    pub conversation: ConversationId,
}

/// Works a conversation's need out again.
pub struct RefreshNeed {
    needs: NeedSettings,
}

impl RefreshNeed {
    /// A job working needs out under `needs`.
    pub fn new(needs: NeedSettings) -> Self {
        Self { needs }
    }
}

impl JobHandler for RefreshNeed {
    type Payload = RefreshNeedPayload;
    const NAME: &'static str = "conversations.refresh_need";
    const QUEUE: &'static str = "rules";

    async fn run(&self, ctx: &mut JobContext, payload: RefreshNeedPayload) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .map(ToOwned::to_owned)
            .ok_or_else(|| JobError::fatal("refresh_need enqueued outside a workspace"))?;
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        match needs::refresh_now(
            tx,
            &events,
            &workspace,
            payload.conversation,
            &self.needs,
            true,
        )
        .await?
        {
            Refreshed::Settled => Ok(()),
            Refreshed::Deferred => Err(JobError::retry_after(
                "the message is still being changed",
                DELAY,
            )),
        }
    }
}

/// Enqueue a refresh of `conversation`.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    conversation: ConversationId,
) -> Result<Enqueued, sqlx::Error> {
    campbooks_kernel::jobs::enqueue_to::<RefreshNeed>(
        tx,
        &RefreshNeedPayload { conversation },
        EnqueueOptions::for_workspace(workspace.to_owned())
            .run_at(Utc::now() + chrono::TimeDelta::from_std(DELAY).unwrap_or_default()),
    )
    .await
}
