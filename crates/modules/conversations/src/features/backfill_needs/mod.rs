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
    Job, JobContext, JobError, JobHandler,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    domain::needs::{self, NeedSettings},
    model::{ConversationId, InboxId},
};

/// How often pending backfills are started.
pub const EVERY: Duration = Duration::from_mins(1);

/// Conversations refreshed per job.
pub const PAGE: i64 = 100;

/// Backfills started per tick.
const DISPATCH_BATCH: i64 = 100;

/// The dispatcher tick: start every pending backfill.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchNeedsBackfills;

impl Job for DispatchNeedsBackfills {
    const NAME: &'static str = "conversations.dispatch_needs_backfills";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let tx = ctx.tx().await?;
        let started = sqlx::query!(
            r#"UPDATE conversations.needs_backfills b SET state = 'running'
                WHERE b.inbox_id IN (
                    SELECT inbox_id FROM conversations.needs_backfills
                     WHERE state = 'pending'
                     ORDER BY created_at
                     LIMIT $1
                     FOR UPDATE SKIP LOCKED)
            RETURNING b.inbox_id AS "inbox: InboxId", b.workspace_id AS "workspace: WorkspaceId",
                      b.after_id AS "after: ConversationId""#,
            DISPATCH_BATCH,
        )
        .fetch_all(&mut **tx)
        .await?;
        for row in &started {
            let page = Page {
                inbox: row.inbox,
                after: row.after,
            };
            jobs::enqueue_to::<BackfillNeeds>(tx, &page, page.options(row.workspace)).await?;
        }
        if !started.is_empty() {
            tracing::info!(inboxes = started.len(), "needs backfills started");
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
        let tx = ctx.tx().await?;
        let current = sqlx::query!(
            r#"SELECT state, after_id AS "after: ConversationId"
                 FROM conversations.needs_backfills WHERE inbox_id = $1 FOR UPDATE"#,
            page.inbox as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        // Gone with its inbox, finished, or this page was done already.
        let Some(current) = current else {
            return Ok(());
        };
        if current.state != "running" || current.after != page.after {
            return Ok(());
        }

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
            needs::refresh(tx, &events, &ws, *id, &self.needs).await?;
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
