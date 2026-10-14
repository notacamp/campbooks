//! Needs for mail stored before attention existed (campbooks-cloud#123).
//!
//! A need is worked out when mail is stored (`apply_changes`), after the
//! model pass (`decide_message`) and when a member acts; mail stored before
//! the attention module existed was never looked at. The migration that
//! added this feature lists every inbox of the time in
//! `conversations.needs_backfills`; then:
//!
//! - `conversations.dispatch_needs_backfills`, every minute, starts the
//!   pending rows (a maintenance transaction, no workspace set: it reads ids
//!   and states only);
//! - `conversations.backfill_needs` pages through one inbox's conversations
//!   by id, in the inbox's workspace (row-level security applies), and runs
//!   [`needs::refresh`] on each one whose last message falls within the
//!   needs window, or that has an open need (so a need the stored answers no
//!   longer support is withdrawn). `refresh` reads only stored messages and
//!   decisions: no model and no rules are asked again. It publishes
//!   `conversations.need_changed` as for live mail, and the attention module
//!   projects the items.
//!
//! Running it again changes nothing: `refresh` publishes only when a need
//! changed.

use std::time::Duration;

use campbooks_kernel::{
    EventBus, Job, JobContext, JobError, JobHandler,
    jobs::{self, EnqueueOptions, Enqueued},
    tenancy::WorkspaceId,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Connection as _, Postgres, Transaction};

use crate::{
    domain::needs::{self, NeedSettings},
    model::{ConversationId, InboxId},
};

/// How often pending backfills are started.
pub const EVERY: Duration = Duration::from_mins(1);

/// Conversations refreshed per job.
pub const PAGE: i64 = 100;

/// Backfills started, and running ones checked, per tick.
const DISPATCH_BATCH: i64 = 500;

/// The dispatcher tick: start pending backfills, and give a running one
/// whose page job is gone (lost, or failed before it could record the
/// inbox as failed) its job again. Enqueueing a page that is already
/// queued or running is a no-op (one job per inbox and cursor).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchNeedsBackfills;

impl Job for DispatchNeedsBackfills {
    const NAME: &'static str = "conversations.dispatch_needs_backfills";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let tx = ctx.tx().await?;
        sqlx::query!(
            r#"UPDATE conversations.needs_backfills b SET state = 'running'
                WHERE b.inbox_id IN (
                    SELECT inbox_id FROM conversations.needs_backfills
                     WHERE state = 'pending'
                     ORDER BY created_at, inbox_id
                     LIMIT $1
                     FOR UPDATE SKIP LOCKED)"#,
            DISPATCH_BATCH,
        )
        .execute(&mut **tx)
        .await?;
        let running = sqlx::query!(
            r#"SELECT inbox_id AS "inbox: InboxId", workspace_id AS "workspace: WorkspaceId",
                      after_id AS "after: ConversationId"
                 FROM conversations.needs_backfills
                WHERE state = 'running'
                ORDER BY created_at, inbox_id
                LIMIT $1"#,
            DISPATCH_BATCH,
        )
        .fetch_all(&mut **tx)
        .await?;
        let mut queued = 0_usize;
        for row in &running {
            let page = Page {
                inbox: row.inbox,
                after: row.after,
            };
            let enqueued =
                jobs::enqueue_to::<BackfillNeeds>(tx, &page, page.options(row.workspace)).await?;
            if matches!(enqueued, Enqueued::Created(_)) {
                queued += 1;
            }
        }
        if queued > 0 {
            tracing::info!(inboxes = queued, "needs backfill pages queued");
        }
        Ok(())
    }
}

/// One page of an inbox's backfill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// The inbox.
    pub inbox: InboxId,
    /// The last conversation id of the previous page; `None` for the first.
    pub after: Option<ConversationId>,
}

impl Page {
    /// Its enqueue options in `workspace`: one pending job per page, behind
    /// live work.
    fn options(&self, workspace: WorkspaceId) -> EnqueueOptions {
        let after = self.after.map_or_else(|| "^".to_owned(), |a| a.to_string());
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!(
                "conversations.backfill_needs:{}:{after}",
                self.inbox
            ))
            .priority(-10)
    }
}

/// Works out the needs of one page of an inbox's conversations.
pub struct BackfillNeeds {
    needs: NeedSettings,
}

impl BackfillNeeds {
    /// A backfill raising needs as `needs` says (the live window and floor).
    pub fn new(needs: NeedSettings) -> Self {
        Self { needs }
    }
}

impl JobHandler for BackfillNeeds {
    type Payload = Page;
    const NAME: &'static str = "conversations.backfill_needs";
    const QUEUE: &'static str = "maintenance";

    async fn run(&self, ctx: &mut JobContext, page: Page) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("needs backfill enqueued outside a workspace"))?;
        let events = ctx.state().events.clone();
        let last_attempt = ctx.is_last_attempt();
        let tx = ctx.tx().await?;
        let current = sqlx::query!(
            r#"SELECT state, after_id AS "after: ConversationId"
                 FROM conversations.needs_backfills WHERE inbox_id = $1 FOR UPDATE"#,
            page.inbox as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        // Gone with its inbox, finished, failed, or this page was done
        // already.
        let Some(current) = current else {
            return Ok(());
        };
        if current.state != "running" || current.after != page.after {
            return Ok(());
        }

        // The page in a savepoint: on the last attempt a failure is undone
        // and the inbox recorded as failed at this cursor, rather than left
        // running for ever.
        let mut work = (**tx).begin().await?;
        match self.page(&mut work, &events, workspace, page).await {
            Ok(()) => {
                work.commit().await?;
                Ok(())
            }
            Err(error) if last_attempt => {
                work.rollback().await?;
                sqlx::query!(
                    "UPDATE conversations.needs_backfills
                        SET state = 'failed', finished_at = now()
                      WHERE inbox_id = $1",
                    page.inbox as InboxId,
                )
                .execute(&mut **tx)
                .await?;
                tracing::warn!(
                    inbox = %page.inbox,
                    error = %error,
                    "needs backfill gave up on an inbox"
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

impl BackfillNeeds {
    /// Refresh one page of the inbox's conversations, record the cursor and
    /// enqueue the next page.
    async fn page(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        events: &EventBus,
        workspace: WorkspaceId,
        page: Page,
    ) -> Result<(), JobError> {
        // Recent conversations, and any whose need is open (an old one may
        // no longer be supported by its answers).
        let since = Utc::now() - self.needs.window;
        let ids = sqlx::query_scalar!(
            r#"SELECT c.id AS "id: ConversationId" FROM conversations.conversations c
                WHERE c.inbox_id = $1
                  AND ($2::uuid IS NULL OR c.id > $2)
                  AND (c.last_message_at >= $3
                       OR EXISTS (SELECT 1 FROM conversations.needs n
                                   WHERE n.conversation_id = c.id AND n.state = 'open'))
                ORDER BY c.id
                LIMIT $4"#,
            page.inbox as InboxId,
            page.after as Option<ConversationId>,
            since,
            PAGE,
        )
        .fetch_all(&mut **tx)
        .await?;
        let ws = workspace.to_string();
        for id in &ids {
            // One conversation that cannot be worked out is skipped, not the
            // page: its own savepoint is undone, and the rest go on.
            let mut one = (**tx).begin().await?;
            match needs::refresh(&mut one, events, &ws, *id, &self.needs).await {
                Ok(()) => one.commit().await?,
                Err(error) => {
                    one.rollback().await?;
                    tracing::warn!(
                        conversation = %id,
                        error = error.code(),
                        "needs backfill skipped a conversation"
                    );
                }
            }
        }

        let last = ids.last().copied().or(page.after);
        let finished = i64::try_from(ids.len()).unwrap_or(i64::MAX) < PAGE;
        let refreshed = sqlx::query_scalar!(
            r#"UPDATE conversations.needs_backfills
                  SET after_id = $2, refreshed = refreshed + $3,
                      state = CASE WHEN $4 THEN 'done' ELSE state END,
                      finished_at = CASE WHEN $4 THEN now() END
                WHERE inbox_id = $1
               RETURNING refreshed"#,
            page.inbox as InboxId,
            last as Option<ConversationId>,
            i64::try_from(ids.len()).unwrap_or(i64::MAX),
            finished,
        )
        .fetch_one(&mut **tx)
        .await?;
        if finished {
            tracing::info!(
                conversations = refreshed,
                "needs backfill finished for an inbox"
            );
            return Ok(());
        }
        let next = Page {
            inbox: page.inbox,
            after: last,
        };
        jobs::enqueue_to::<Self>(tx, &next, next.options(workspace)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
