//! Needs for mail stored before attention existed (campbooks-cloud#123), or
//! before needs were worked out another way (the facts and the model,
//! ADR 0018).
//!
//! A need is worked out when mail is stored (`apply_changes`), after the
//! model pass (`decide_message`) and when a member acts; mail stored before
//! the attention module existed was never looked at, and mail read by the
//! retired content rules was looked at by rules that no longer count. The
//! migrations that changed this list every inbox of the time in
//! `conversations.needs_backfills`; then:
//!
//! - `conversations.dispatch_needs_backfills`, every minute, starts the
//!   pending rows (a transaction in the maintenance scope: it reads ids and
//!   states only, through the table's maintenance policies, and enqueues
//!   each page job in its inbox's workspace);
//! - `conversations.backfill_needs` pages through one inbox's conversations
//!   by id, in the inbox's workspace (row-level security applies), and runs
//!   [`needs::refresh`] on each one whose last message falls within the
//!   needs window, or that has an open need (so a need the stored answers no
//!   longer support is withdrawn). `refresh` reads only stored messages and
//!   decisions: no model and no rules are asked again there. It publishes
//!   `conversations.need_changed` as for live mail, and the attention module
//!   projects the items.
//! - When a `decide` model is configured and has not failed its answers
//!   check on its answers (a check still running, or one the provider did
//!   not answer, still queues: the pass waits for it), each of those conversations' newest inbound message sent within
//!   the decide window (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`)
//!   that no checked model answered, or that lacks a checked answer to a
//!   triage question added since (the money question, campbooks-cloud#167),
//!   is queued for the model pass
//!   (`conversations.decide_backlog`, one job per message, ids only), which
//!   sorts it and works out its need again.
//!
//! Running it again changes nothing: `refresh` publishes only when a need
//! changed, and a message a checked model answered every question about is
//! not asked again.

use std::time::Duration;

use campbooks_ai::decide::signals::email::triage_questions;
use campbooks_kernel::{
    EventBus, Job, JobContext, JobError, JobHandler, db,
    jobs::{self, EnqueueOptions, Enqueued},
    tenancy::WorkspaceId,
};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::needs::{self, NeedSettings},
    features::decide_message::{Backlog, DecideBacklog, ModelPass},
    model::{ConversationId, InboxId, MessageId},
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
        // Across workspaces, through the table's maintenance policies (ids
        // and states only): the maintenance scope, allowed for this file in
        // `architecture.toml`. It reaches no kernel table: each page job is
        // enqueued in its inbox's workspace, set just before.
        let tx = ctx.maintenance_tx().await?;
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
            db::set_workspace(tx, &row.workspace.to_string()).await?;
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
    model: Option<ModelPass>,
    decide_window: TimeDelta,
}

impl BackfillNeeds {
    /// A backfill raising needs as `needs` says (the live window and floor),
    /// queueing the model pass (`model`, if any) for recent mail sent within
    /// `decide_backfill_days` that no checked model answered.
    pub fn new(needs: NeedSettings, model: Option<ModelPass>, decide_backfill_days: u32) -> Self {
        Self {
            needs,
            model,
            decide_window: TimeDelta::days(i64::from(decide_backfill_days)),
        }
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
        let mut work = db::savepoint(tx).await?;
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
        // A kept, read-only inbox (its connection removed) is history: it is
        // done with nothing to look at.
        let detached = sqlx::query_scalar!(
            r#"SELECT detached_at IS NOT NULL AS "detached!" FROM conversations.inboxes
                WHERE id = $1"#,
            page.inbox as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(true);
        if detached {
            sqlx::query!(
                "UPDATE conversations.needs_backfills
                    SET state = 'done', finished_at = now()
                  WHERE inbox_id = $1",
                page.inbox as InboxId,
            )
            .execute(&mut **tx)
            .await?;
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
            // One conversation that cannot be worked out is skipped, not the
            // page: its own savepoint is undone, and the rest go on.
            let mut one = db::savepoint(tx).await?;
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

        if self.model.as_ref().is_some_and(|m| !m.refused()) {
            self.queue_model_pass(tx, workspace, &ids).await?;
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

    /// Queue the model pass for the newest inbound message of each of
    /// `conversations` sent within the decide window that a checked model
    /// has not answered every triage question about.
    async fn queue_model_pass(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace: WorkspaceId,
        conversations: &[ConversationId],
    ) -> Result<(), JobError> {
        let since = Utc::now() - self.decide_window;
        let questions: Vec<String> = triage_questions().into_iter().map(|q| q.id).collect();
        let unanswered = sqlx::query_scalar!(
            r#"SELECT newest.id AS "id!: MessageId"
                 FROM (SELECT DISTINCT ON (m.conversation_id) m.id, m.sent_at
                         FROM conversations.messages m
                        WHERE m.conversation_id = ANY($1) AND m.direction = 'inbound'
                          AND NOT m.in_spam AND NOT m.in_trash
                        ORDER BY m.conversation_id, m.sent_at DESC, m.id DESC) newest
                WHERE newest.sent_at >= $2
                  AND EXISTS (SELECT 1 FROM unnest($3::text[]) q (id)
                               WHERE NOT EXISTS (
                                     SELECT 1 FROM conversations.decisions d
                                      WHERE d.message_id = newest.id AND d.source = 'model'
                                        AND d.model_version LIKE 'checked:%'
                                        AND d.question = q.id))"#,
            conversations as &[ConversationId],
            since,
            &questions,
        )
        .fetch_all(&mut **tx)
        .await?;
        for message in unanswered {
            let backlog = Backlog { message };
            jobs::enqueue_to::<DecideBacklog>(tx, &backlog, backlog.options(workspace)).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
