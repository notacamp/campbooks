//! A Notion stream's run (campbooks-cloud#376): one linked Notion page.
//!
//! A run first makes the block changes members' edits asked for
//! (`notion::write`, at most [`changes::PUSH_LIMIT`] a run), records how
//! each ended as it happens, and tells the pages module when a request has
//! no change left to wait for. Then it reads: the page itself (one call;
//! nothing more when Notion's edit time has not moved and no webhook asked
//! for a read), and when it changed the page's blocks (`notion::read`), as
//! a snapshot stored as a blob and announced to the pages module
//! (`connectors.notion_page_read`) in the transaction that moves the
//! cursor, so a snapshot is never lost or applied twice.
//!
//! A page Notion no longer shares with the integration answers 404, and a
//! page in the trash says so: either ends the link
//! (`connectors.notion_link_stopped`), deletes the stream and leaves
//! Notion alone. Like every run it takes its connection's budget and
//! follows the run's backoff and throttling.
//!
//! Errors are built from static words only: they are kept with the job.

use std::collections::HashMap;

use campbooks_kernel::tenancy::WorkspaceId;
use campbooks_kernel::{JobContext, JobError, blobs::BlobOwner, db::BeginFor as _};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{SyncStream, Target, prune_batches};
use crate::{
    calendar::changes::read_json,
    domain::{
        credentials::{self, TokenError},
        health,
        model::{Health, Phase},
        notion::{self as notion_db, Link},
        removal::BATCH_OWNER,
        schedule::{self, LEASE, SYNC_STREAM, SyncStreamPayload},
        tracked,
    },
    events::{NotionLinkStopped, NotionPageRead, NotionStop},
    metrics::{self, SyncOutcome},
    notion::{
        Notion, PageInfo,
        blocks::{ChangeDocument, SNAPSHOT_CONTENT_TYPE},
        changes::{self, At, MAX_ATTEMPTS, MAX_CHANGE_BYTES, State},
        pace::{Pace, Stop},
        read::{self, Files, Limits},
        write::{self, Made},
    },
    provider::ProviderError,
    tracking,
};

/// What a Notion stream remembers between runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cursor {
    /// The page's edit time when its blocks were last read.
    #[serde(default)]
    edited: Option<DateTime<Utc>>,
    /// A webhook asked for a read, whatever the edit time says.
    #[serde(default)]
    force: bool,
    /// The files downloaded, by block.
    #[serde(default)]
    files: Files,
}

/// What a run's push did.
#[derive(Debug, Default)]
struct Pushed {
    /// Something was written at Notion.
    wrote: bool,
    /// What stopped the push: the read does not follow.
    stop: Option<Stop>,
    /// The newest change taken.
    last: Option<Uuid>,
}

impl SyncStream {
    /// One run of a Notion stream. `Ok(())` once it has ended (waiting,
    /// trouble recorded, or applied).
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: token, push, page, read, apply"
    )]
    pub(super) async fn run_notion(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let Some(notion) = self.deps.notion.clone() else {
            // Notion is no longer offered: the leased schedule row brings the
            // stream back later, and nothing is deleted meanwhile.
            return Ok(());
        };
        let link = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let link = notion_db::link_of(&mut tx, target.stream).await?;
            tx.commit().await?;
            link
        };
        let Some(link) = link else {
            return Ok(());
        };
        let token =
            match credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
                .await
            {
                Ok(token) => token,
                Err(TokenError::NeedsReauth(reason)) => {
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self.needs_reauth(ctx, workspace, target, reason, now).await;
                }
                Err(TokenError::Provider(err)) => {
                    return self.trouble(ctx, workspace, target, &err, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
            };
        let adapter = self.deps.config.notion.adapter();
        let mut pace = Pace::new(ctx.pool().clone(), target.connection, &adapter);
        let at = At {
            connection: target.connection,
            notion_page: link.notion_page_id,
        };
        let pushed = self
            .push_notion(ctx, workspace, target, &notion, &token, &mut pace, at, now)
            .await?;
        if let Some(stop) = pushed.stop {
            return self.notion_stopped(ctx, workspace, target, stop, now).await;
        }

        // The page itself: one call.
        if let Err(stop) = pace.take().await {
            return self.notion_stopped(ctx, workspace, target, stop, now).await;
        }
        let page = match notion.page(&token, link.notion_page_id).await {
            Ok(page) => page,
            Err(ProviderError::NotFound) => {
                return self
                    .stop_notion_link(ctx, workspace, target, &link, NotionStop::NotShared)
                    .await;
            }
            Err(err) => {
                return self
                    .notion_stopped(ctx, workspace, target, Stop::Provider(err), now)
                    .await;
            }
        };
        if page.trashed {
            return self
                .stop_notion_link(ctx, workspace, target, &link, NotionStop::TrashedInNotion)
                .await;
        }
        let cursor: Cursor = serde_json::from_value(target.cursor.clone()).unwrap_or_default();
        let first = target.phase == Phase::Backfill;
        let due = first || cursor.force || cursor.edited != Some(page.edited);
        let more = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let more = changes::more_pending(&mut tx, target.stream, pushed.last).await?;
            tx.commit().await?;
            more
        };
        if !due {
            return self
                .finish_notion(ctx, workspace, target, now, (pushed.wrote, more), None)
                .await;
        }
        let key = workspace.to_string();
        let limits = Limits {
            max_blocks: usize::try_from(adapter.max_blocks).unwrap_or(usize::MAX),
            max_depth: usize::try_from(adapter.max_depth).unwrap_or(usize::MAX),
        };
        let state = ctx.state().clone();
        let read = match read::read_page(
            &notion,
            &token,
            &mut pace,
            &state.blobs,
            &key,
            &page,
            cursor.files.clone(),
            limits,
            now,
        )
        .await
        {
            Ok(read) => read,
            Err(stop) => return self.notion_stopped(ctx, workspace, target, stop, now).await,
        };
        let bytes = serde_json::to_vec(&read.snapshot)
            .map_err(|_| JobError::fatal("a Notion snapshot did not encode"))?;
        let snapshot = state.blobs.put(&key, bytes, SNAPSHOT_CONTENT_TYPE).await?;
        let blocks = count(&read.snapshot.blocks);
        let applied = Applied {
            page: &page,
            files: read.files,
            held: read.held,
            snapshot,
            blocks,
        };
        self.finish_notion(
            ctx,
            workspace,
            target,
            now,
            (pushed.wrote, more),
            Some((&link, applied)),
        )
        .await
    }

    /// Make the pending block changes of the backlog, one at a time,
    /// recording how each ended before the next is made. A change that met
    /// Notion's trouble stays pending (until it has met it
    /// [`MAX_ATTEMPTS`] times) and stops the push.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one run read top to bottom: given up, refused, made"
    )]
    async fn push_notion(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        notion: &Notion,
        token: &str,
        pace: &mut Pace,
        at: At,
        now: DateTime<Utc>,
    ) -> Result<Pushed, JobError> {
        let state = ctx.state().clone();
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        let backlog = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let expired = changes::expire(&mut tx, target.stream, now - retention, now).await?;
            let backlog =
                changes::backlog(&mut tx, &state.binding, workspace, target.stream, at, now)
                    .await?;
            let mut requests: Vec<Uuid> = expired
                .iter()
                .chain(&backlog.refused)
                .map(|row| row.request_id)
                .collect();
            requests.sort_unstable();
            requests.dedup();
            for request in requests {
                changes::complete(&mut tx, &state, workspace, request).await?;
            }
            tx.commit().await?;
            backlog
        };
        let mut pushed = Pushed {
            last: backlog.last(),
            ..Pushed::default()
        };
        if backlog.pending.is_empty() {
            return Ok(pushed);
        }
        // What the pages module wrote for each request, and the blocks made
        // for them so far.
        let mut documents: HashMap<String, Option<ChangeDocument>> = HashMap::new();
        for row in &backlog.pending {
            let hash = row.body.0.hash.to_string();
            if !documents.contains_key(&hash) {
                let document = read_json::<ChangeDocument>(
                    &state.blobs,
                    workspace,
                    &row.body.0,
                    MAX_CHANGE_BYTES,
                )
                .await?;
                documents.insert(hash, document);
            }
        }
        let mut made: HashMap<Uuid, Uuid> = {
            let mut requests: Vec<Uuid> = backlog.pending.iter().map(|r| r.request_id).collect();
            requests.sort_unstable();
            requests.dedup();
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let made = changes::made(&mut tx, &requests).await?;
            tx.commit().await?;
            made
        };
        for row in &backlog.pending {
            let seq = u32::try_from(row.seq).unwrap_or(u32::MAX);
            let item = documents
                .get(&row.body.0.hash.to_string())
                .and_then(Option::as_ref)
                .and_then(|document| document.items.iter().find(|item| item.seq == seq))
                .and_then(|item| item.block.as_ref());
            let outcome =
                write::make(notion, token, pace, row, item, at.notion_page, &mut made).await;
            let (end, reason, written) = match outcome {
                Ok(Made::Applied(written)) => {
                    pushed.wrote = true;
                    (State::Applied, None, Some(written))
                }
                Ok(Made::Skipped(reason)) => (State::Skipped, Some(reason), None),
                Ok(Made::Failed(reason)) => (State::Failed, Some(reason), None),
                Err(Stop::Provider(ProviderError::Unavailable(detail))) => {
                    let mut tx = ctx.pool().begin_for(workspace).await?;
                    let attempts = changes::attempted(&mut tx, row.id).await?;
                    tx.commit().await?;
                    if attempts < MAX_ATTEMPTS {
                        pushed.stop = Some(Stop::Provider(ProviderError::Unavailable(detail)));
                        break;
                    }
                    (
                        State::Failed,
                        Some(crate::events::NotionReason::TooManyAttempts),
                        None,
                    )
                }
                Err(stop) => {
                    pushed.stop = Some(stop);
                    break;
                }
            };
            let mut tx = ctx.pool().begin_for(workspace).await?;
            changes::end(&mut tx, row.id, end, reason, written, now).await?;
            changes::complete(&mut tx, &state, workspace, row.request_id).await?;
            tx.commit().await?;
        }
        Ok(pushed)
    }

    /// What a run that stopped is: Notion's trouble, a wait for the
    /// budget, or the database failing.
    async fn notion_stopped(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        stop: Stop,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        match stop {
            Stop::Provider(err) => self.trouble(ctx, workspace, target, &err, now).await,
            Stop::Database(err) => Err(err.into()),
            Stop::Waiting(wait) => {
                metrics::sync_finished(target.provider, SyncOutcome::Throttled);
                Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await
            }
        }
    }

    /// The page is not read any more: its stream goes and the pages module
    /// is told, so it ends the link. Nothing is deleted at Notion.
    async fn stop_notion_link(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        link: &Link,
        reason: NotionStop,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        health::lock(tx, target.connection).await?;
        state
            .events
            .publish(
                tx,
                &NotionLinkStopped {
                    workspace: workspace.to_string(),
                    connection: target.connection,
                    notion_page: link.notion_page_id,
                    page: link.page_id,
                    reason,
                },
            )
            .await?;
        notion_db::drop_stream(tx, &state, workspace, target.stream).await?;
        metrics::sync_finished(target.provider, SyncOutcome::Ok);
        tracing::info!(%workspace, "a Notion page is no longer read");
        Ok(())
    }

    /// End the run: apply what was read (the snapshot, announced in the
    /// transaction that moves the cursor), settle the stream and set the
    /// next run. `pushed` says whether something was written and whether
    /// changes arrived meanwhile.
    #[allow(
        clippy::too_many_lines,
        reason = "one transaction read top to bottom: batch, cursor, health, next run"
    )]
    async fn finish_notion(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
        (wrote, more): (bool, bool),
        read: Option<(&Link, Applied<'_>)>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let key = workspace.to_string();
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        let changed = wrote || read.is_some();
        let poll = &self.deps.config.poll;
        let interval = if self.deps.notion_webhook.is_some() {
            // A webhook posts the changes: a safety poll at the longest
            // interval.
            i32::try_from(poll.max_seconds).unwrap_or(i32::MAX)
        } else {
            // Without one, the page is looked at every five minutes at
            // least, backing off as it stays quiet.
            schedule::next_interval(target.interval_seconds, changed, poll).max(300)
        };
        let mut cursor: Cursor = serde_json::from_value(target.cursor.clone()).unwrap_or_default();
        let tx = ctx.tx().await?;
        // The connection before the stream (removal's order).
        health::lock(tx, target.connection).await?;
        if let Some((link, applied)) = &read {
            let batch = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO connectors.batches
                     (id, workspace_id, stream_id, batch_hash, batch_size, cursor, changes,
                      created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(batch)
            .bind(workspace)
            .bind(target.stream)
            .bind(applied.snapshot.hash.to_string())
            .bind(i64::try_from(applied.snapshot.size).unwrap_or(i64::MAX))
            .bind(applied.page.edited.to_rfc3339())
            .bind(i32::try_from(applied.blocks).unwrap_or(i32::MAX))
            .bind(now)
            .execute(&mut **tx)
            .await?;
            let owner = BlobOwner::new(BATCH_OWNER, batch);
            state
                .blobs
                .attach(tx, &key, &applied.snapshot, &owner)
                .await?;
            for blob in &applied.held {
                state.blobs.attach(tx, &key, blob, &owner).await?;
            }
            state
                .events
                .publish(
                    tx,
                    &NotionPageRead {
                        workspace: key.clone(),
                        connection: target.connection,
                        notion_page: link.notion_page_id,
                        page: link.page_id,
                        snapshot: applied.snapshot.clone(),
                        read_at: now,
                    },
                )
                .await?;
            cursor = Cursor {
                edited: Some(applied.page.edited),
                force: false,
                files: applied.files.clone(),
            };
        }
        changes::prune(tx, target.stream, now - retention).await?;
        prune_batches(tx, &state, workspace, target.stream, now - retention).await?;
        let cursor_json = serde_json::to_value(&cursor)
            .map_err(|_| JobError::fatal("a Notion cursor did not encode"))?;
        sqlx::query(
            "UPDATE connectors.streams
                SET phase = 'incremental', cursor = $2, interval_seconds = $3,
                    last_synced_at = $4,
                    last_change_at = CASE WHEN $5 THEN $4 ELSE last_change_at END,
                    consecutive_failures = 0, throttled_until = NULL
              WHERE id = $1",
        )
        .bind(target.stream)
        .bind(cursor_json)
        .bind(interval)
        .bind(now)
        .bind(changed)
        .execute(&mut **tx)
        .await?;
        if matches!(target.health, Health::Throttled | Health::Failing) {
            health::set(
                tx,
                &state.tracker,
                workspace,
                target.connection,
                Health::Ok,
                None,
                now,
            )
            .await?;
        }
        // One summary per run, only when it stored something or moved on.
        if let Some((_, applied)) = &read {
            let (stream, provider) = tracked::stream(tx, target.stream).await?;
            state
                .tracker
                .track(
                    tx,
                    &tracking::StreamSynced {
                        workspace,
                        stream,
                        provider,
                        before_phase: tracked::phase(target.phase),
                        changes: i64::try_from(applied.blocks).unwrap_or(i64::MAX),
                        batches: 1,
                    },
                )
                .await?;
        }
        if more {
            // Changes arrived during the run: run again straight away,
            // holding the schedule row so the dispatcher does not double it.
            schedule::set_due(tx, workspace, target.stream, now + LEASE).await?;
            campbooks_kernel::jobs::enqueue_to::<Self>(
                tx,
                &SyncStreamPayload {
                    stream: target.stream,
                },
                campbooks_kernel::jobs::EnqueueOptions::for_workspace(key)
                    .ordered_by(format!("{SYNC_STREAM}:{}", target.stream)),
            )
            .await?;
        } else {
            let next = now + TimeDelta::seconds(i64::from(interval));
            schedule::set_due(tx, workspace, target.stream, next).await?;
        }
        metrics::sync_finished(target.provider, SyncOutcome::Ok);
        tracing::info!(
            %workspace, stream = %target.stream, read = read.is_some(), wrote, more,
            "Notion stream synced"
        );
        Ok(())
    }
}

/// What a read gave, to apply.
struct Applied<'a> {
    page: &'a PageInfo,
    files: Files,
    held: Vec<campbooks_kernel::blobs::BlobRef>,
    snapshot: campbooks_kernel::blobs::BlobRef,
    blocks: usize,
}

/// How many blocks `blocks` holds, with theirs.
fn count(blocks: &[crate::notion::blocks::Block]) -> usize {
    blocks.iter().map(|b| 1 + count(&b.children)).sum()
}
