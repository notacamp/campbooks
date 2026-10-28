//! A Notion database stream's run (campbooks-cloud#377): one linked data
//! source, kept in step with a table.
//!
//! A run first makes the table changes members' edits asked for
//! (`notion::write_tables`, at most [`changes::PUSH_LIMIT`] a run), recording
//! how each ended as it happens and telling the pages module when a request
//! has no change left to wait for. Then it reads: the data source (its
//! properties) and the entries edited since the cursor
//! (`notion::read_tables`), as a snapshot stored as a blob and announced to
//! the pages module (`connectors.notion_data_source_read`) in the
//! transaction that moves the cursor, so a snapshot is never lost or
//! applied twice. A run that read nothing new announces nothing.
//!
//! A data source Notion no longer shares with the integration answers 404,
//! and one in the trash says so: either ends the link
//! (`connectors.notion_link_stopped`), deletes the stream and leaves Notion
//! alone. A database is never deleted from here.
//!
//! Errors are built from static words only: they are kept with the job.

use std::collections::HashMap;

use campbooks_kernel::tenancy::WorkspaceId;
use campbooks_kernel::{JobContext, JobError, blobs::BlobOwner, db::BeginFor as _};
use chrono::{DateTime, TimeDelta, Utc};
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
    events::{NotionDataSourceRead, NotionLinkStopped, NotionReason, NotionStop},
    metrics::{self, SyncOutcome},
    notion::{
        Notion,
        changes::{MAX_ATTEMPTS, State},
        databases::{ChangeDocument, SNAPSHOT_CONTENT_TYPE},
        pace::{Pace, Stop},
        read_tables::{self, Cursor, Read},
        table_changes::{self, At, MAX_CHANGE_BYTES},
        write_tables::{self, Made},
    },
    provider::ProviderError,
    tracking,
};

/// What a run's push did.
#[derive(Debug, Default)]
struct Pushed {
    wrote: bool,
    stop: Option<Stop>,
    last: Option<Uuid>,
}

impl SyncStream {
    /// One run of a Notion database stream. `Ok(())` once it has ended
    /// (waiting, trouble recorded, or applied).
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: token, push, data source, read, apply"
    )]
    pub(super) async fn run_notion_table(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        link: Link,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let Some(notion) = self.deps.notion.clone() else {
            return Ok(());
        };
        if !self.deps.notion_databases {
            // Databases are not offered (any more): the leased schedule row
            // brings the stream back later, and nothing is deleted meanwhile.
            return Ok(());
        }
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
            data_source: link.notion_page_id,
        };
        let pushed = self
            .push_notion_table(ctx, workspace, target, &notion, &token, &mut pace, at, now)
            .await?;
        if let Some(stop) = pushed.stop {
            return self.notion_stopped(ctx, workspace, target, stop, now).await;
        }
        if let Err(stop) = pace.take().await {
            return self.notion_stopped(ctx, workspace, target, stop, now).await;
        }
        let info = match notion.data_source(&token, link.notion_page_id).await {
            Ok(info) => info,
            Err(ProviderError::NotFound) => {
                return self
                    .stop_notion_table(ctx, workspace, target, &link, NotionStop::NotShared)
                    .await;
            }
            Err(err) => {
                return self
                    .notion_stopped(ctx, workspace, target, Stop::Provider(err), now)
                    .await;
            }
        };
        if info.trashed {
            return self
                .stop_notion_table(ctx, workspace, target, &link, NotionStop::TrashedInNotion)
                .await;
        }
        let cursor: Cursor = serde_json::from_value(target.cursor.clone()).unwrap_or_default();
        let first = target.phase == Phase::Backfill;
        let more = {
            let mut tx = ctx.pool().begin_for(workspace).await?;
            let more = table_changes::more_pending(&mut tx, target.stream, pushed.last).await?;
            tx.commit().await?;
            more
        };
        let read =
            match read_tables::read(&notion, &token, &mut pace, &info, &cursor, first, now).await {
                Ok(read) => read,
                Err(stop) => return self.notion_stopped(ctx, workspace, target, stop, now).await,
            };
        let announce = read.snapshot.schema.is_some()
            || !read.snapshot.entries.is_empty()
            || read.snapshot.sweep;
        let kept = if announce {
            let bytes = serde_json::to_vec(&read.snapshot)
                .map_err(|_| JobError::fatal("a Notion snapshot did not encode"))?;
            let key = workspace.to_string();
            let blob = ctx
                .state()
                .blobs
                .put(&key, bytes, SNAPSHOT_CONTENT_TYPE)
                .await?;
            Some(blob)
        } else {
            None
        };
        self.finish_notion_table(
            ctx,
            workspace,
            target,
            now,
            (pushed.wrote, more),
            &link,
            read,
            kept,
        )
        .await
    }

    /// Make the pending table changes of the backlog, one at a time,
    /// recording how each ended before the next is made. A change that met
    /// Notion's trouble stays pending (until it has met it [`MAX_ATTEMPTS`]
    /// times) and stops the push.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one run read top to bottom: given up, refused, made"
    )]
    async fn push_notion_table(
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
            let expired =
                table_changes::expire(&mut tx, target.stream, now - retention, now).await?;
            let backlog =
                table_changes::backlog(&mut tx, &state.binding, workspace, target.stream, at, now)
                    .await?;
            let mut requests: Vec<Uuid> = expired
                .iter()
                .chain(&backlog.refused)
                .map(|row| row.request_id)
                .collect();
            requests.sort_unstable();
            requests.dedup();
            for request in requests {
                table_changes::complete(&mut tx, &state, workspace, request).await?;
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
        let mut documents: HashMap<String, Option<ChangeDocument>> = HashMap::new();
        for row in &backlog.pending {
            let hash = row.body.0.hash.to_string();
            if let std::collections::hash_map::Entry::Vacant(slot) = documents.entry(hash) {
                let document = read_json::<ChangeDocument>(
                    &state.blobs,
                    workspace,
                    &row.body.0,
                    MAX_CHANGE_BYTES,
                )
                .await?;
                slot.insert(document);
            }
        }
        let mut run = write_tables::Run::default();
        for row in &backlog.pending {
            let seq = u32::try_from(row.seq).unwrap_or(u32::MAX);
            let item = documents
                .get(&row.body.0.hash.to_string())
                .and_then(Option::as_ref)
                .and_then(|document| write_tables::item_of(document, seq));
            let outcome =
                write_tables::make(notion, token, pace, &mut run, row, item, at.data_source).await;
            let (end, reason, written) = match outcome {
                Ok(Made::Applied(written)) => {
                    pushed.wrote = true;
                    (State::Applied, None, Some(written))
                }
                Ok(Made::Skipped(reason)) => (State::Skipped, Some(reason), None),
                Ok(Made::Failed(reason)) => (State::Failed, Some(reason), None),
                Err(Stop::Provider(ProviderError::Unavailable(detail))) => {
                    let mut tx = ctx.pool().begin_for(workspace).await?;
                    let attempts = table_changes::attempted(&mut tx, row.id).await?;
                    tx.commit().await?;
                    if attempts < MAX_ATTEMPTS {
                        pushed.stop = Some(Stop::Provider(ProviderError::Unavailable(detail)));
                        break;
                    }
                    (State::Failed, Some(NotionReason::TooManyAttempts), None)
                }
                Err(stop) => {
                    pushed.stop = Some(stop);
                    break;
                }
            };
            let mut tx = ctx.pool().begin_for(workspace).await?;
            table_changes::end(&mut tx, row.id, end, reason, written, now).await?;
            table_changes::complete(&mut tx, &state, workspace, row.request_id).await?;
            tx.commit().await?;
        }
        Ok(pushed)
    }

    /// The database is not read any more: its stream goes and the pages
    /// module is told, so it ends the link. Nothing is deleted at Notion.
    async fn stop_notion_table(
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
        tracing::info!(%workspace, "a Notion database is no longer read");
        Ok(())
    }

    /// End the run: announce what was read (in the transaction that moves
    /// the cursor), settle the stream and set the next run.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one transaction read top to bottom: batch, cursor, health, next run"
    )]
    async fn finish_notion_table(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
        (wrote, more): (bool, bool),
        link: &Link,
        read: Read,
        snapshot: Option<campbooks_kernel::blobs::BlobRef>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let key = workspace.to_string();
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        let changed = wrote || snapshot.is_some();
        let more = more || read.more;
        let poll = &self.deps.config.poll;
        let interval = if self.deps.notion_webhook.is_some() {
            i32::try_from(poll.max_seconds).unwrap_or(i32::MAX)
        } else {
            schedule::next_interval(target.interval_seconds, changed, poll).max(300)
        };
        let tx = ctx.tx().await?;
        health::lock(tx, target.connection).await?;
        if let Some(blob) = &snapshot {
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
            .bind(blob.hash.to_string())
            .bind(i64::try_from(blob.size).unwrap_or(i64::MAX))
            .bind(now.to_rfc3339())
            .bind(i32::try_from(read.entries).unwrap_or(i32::MAX))
            .bind(now)
            .execute(&mut **tx)
            .await?;
            state
                .blobs
                .attach(tx, &key, blob, &BlobOwner::new(BATCH_OWNER, batch))
                .await?;
            state
                .events
                .publish(
                    tx,
                    &NotionDataSourceRead {
                        workspace: key.clone(),
                        connection: target.connection,
                        data_source: link.notion_page_id,
                        table: link.page_id,
                        snapshot: blob.clone(),
                        read_at: now,
                    },
                )
                .await?;
        }
        table_changes::prune(tx, target.stream, now - retention).await?;
        prune_batches(tx, &state, workspace, target.stream, now - retention).await?;
        let cursor_json = serde_json::to_value(&read.cursor)
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
        if snapshot.is_some() {
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
                        changes: i64::try_from(read.entries).unwrap_or(i64::MAX),
                        batches: 1,
                    },
                )
                .await?;
        }
        if more {
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
            %workspace, stream = %target.stream, read = snapshot.is_some(), wrote, more,
            "Notion database stream synced"
        );
        Ok(())
    }
}
