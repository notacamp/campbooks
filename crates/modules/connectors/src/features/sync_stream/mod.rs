//! `connectors.sync_stream` — one run of one stream: refresh the token if
//! needed, take budget, read one page through the adapter, then in the
//! job's transaction apply the adapter's index updates, store and announce
//! the batch (`connectors.messages_changed`), advance the cursor and set the
//! next run.
//!
//! The stream row is the schedule: provider trouble (throttling, outages)
//! is recorded on it and the run ends normally, with the next run set by
//! the backoff, rather than failing the job.

use campbooks_kernel::{
    JobContext, JobError,
    blobs::BlobOwner,
    db::BeginFor as _,
    jobs::{EnqueueOptions, JobHandler},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{Postgres, Transaction};

use crate::{
    contract::{BATCH_CONTENT_TYPE, MessageChangeBatch},
    deps::Deps,
    domain::{
        credentials::{self, TokenError},
        emit, health,
        model::{ConnectionId, Health, InboxId, Phase, Provider, StreamId},
        removal::BATCH_OWNER,
        schedule::{self, LEASE, SYNC_STREAM, SyncStreamPayload},
    },
    gmail::{self, Cursor, Page, Reader},
    google::GoogleError,
};

/// Runs `connectors.sync_stream`.
#[derive(Debug, Clone)]
pub struct SyncStream {
    deps: Deps,
}

impl SyncStream {
    /// The job handler, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// A stream and its connection, as a run needs them.
#[derive(Debug)]
struct Target {
    stream: StreamId,
    connection: ConnectionId,
    provider: Provider,
    granted_by: UserId,
    health: Health,
    inbox: Option<InboxId>,
    phase: Phase,
    cursor: Cursor,
    interval_seconds: i32,
    paused: bool,
    consecutive_failures: i32,
    throttled_until: Option<DateTime<Utc>>,
}

impl JobHandler for SyncStream {
    type Payload = SyncStreamPayload;
    const NAME: &'static str = SYNC_STREAM;
    const QUEUE: &'static str = "sync";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(&self, ctx: &mut JobContext, payload: SyncStreamPayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("sync enqueued outside a workspace"))?;
        let now = self.deps.clock.now();
        let Some(target) = load(ctx, workspace, payload.stream).await? else {
            return Ok(()); // Disconnected meanwhile.
        };
        if target.paused || target.health == Health::NeedsReauth {
            return Ok(());
        }
        if let Some(until) = target.throttled_until.filter(|until| *until > now) {
            schedule::set_due(ctx.tx().await?, workspace, target.stream, until).await?;
            return Ok(());
        }
        let (Some(google), Some(inbox)) = (self.deps.google.clone(), target.inbox) else {
            // The provider is no longer configured (or the stream has no
            // adapter yet): the leased schedule row brings it back later.
            tracing::debug!(stream = %target.stream, "no adapter for this stream");
            return Ok(());
        };

        let token =
            match credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
                .await
            {
                Ok(token) => token,
                Err(TokenError::NeedsReauth(reason)) => {
                    let events = ctx.state().events.clone();
                    health::needs_reauth(
                        ctx.tx().await?,
                        &events,
                        &workspace.to_string(),
                        target.connection,
                        target.granted_by,
                        reason,
                        now,
                    )
                    .await?;
                    return Ok(());
                }
                Err(TokenError::Provider(err)) => {
                    return self.trouble(ctx, workspace, &target, &err, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
            };

        let rate = self.deps.config.google.units_per_second;
        if let Err(wait) =
            schedule::take_budget(ctx.pool(), target.provider, gmail::UNITS_PER_RUN, rate).await?
        {
            let at = now + TimeDelta::from_std(wait).unwrap_or(TimeDelta::seconds(1));
            schedule::set_due(ctx.tx().await?, workspace, target.stream, at).await?;
            return Ok(());
        }

        let state = ctx.state().clone();
        let reader = Reader {
            gmail: google.gmail(&token),
            pool: ctx.pool(),
            blobs: &state.blobs,
            workspace,
            stream: target.stream,
            phase: target.phase,
            cursor: target.cursor.clone(),
            now,
            backfill_days: self.deps.config.gmail.backfill_days,
            max_message_bytes: self.deps.config.gmail.max_message_bytes,
            run_budget_bytes: self.deps.config.gmail.run_budget_bytes,
        };
        let page = match reader.read().await {
            Ok(page) => page,
            Err(gmail::ReadError::Google(err)) => {
                return self.trouble(ctx, workspace, &target, &err, now).await;
            }
            Err(other) => return Err(JobError::retry(other.to_string())),
        };
        self.apply(ctx, workspace, &target, inbox, page, now).await
    }
}

impl SyncStream {
    /// Apply a page in the job's transaction.
    async fn apply(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        inbox: InboxId,
        page: Page,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let key = workspace.to_string();
        let changed = !page.changes.is_empty();
        let change_count = page.changes.len();
        // Blobs are stored before the transaction that references them.
        let batch = if changed {
            let document = MessageChangeBatch::new(inbox, page.changes);
            let bytes = serde_json::to_vec(&document)
                .map_err(|e| JobError::fatal(format!("encoding a batch: {e}")))?;
            Some(state.blobs.put(&key, bytes, BATCH_CONTENT_TYPE).await?)
        } else {
            None
        };
        let cursor_json = serde_json::to_value(&page.cursor)
            .map_err(|e| JobError::fatal(format!("encoding a cursor: {e}")))?;
        let interval =
            schedule::next_interval(target.interval_seconds, changed, &self.deps.config.poll);
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));

        let tx = ctx.tx().await?;
        if let Some(labels) = &page.labels {
            sqlx::query!(
                "DELETE FROM connectors.gmail_labels WHERE stream_id = $1",
                target.stream as StreamId
            )
            .execute(&mut **tx)
            .await?;
            for label in labels {
                sqlx::query!(
                    r#"INSERT INTO connectors.gmail_labels (workspace_id, stream_id, label_id, name, kind)
                       VALUES ($1, $2, $3, $4, $5)"#,
                    workspace as WorkspaceId,
                    target.stream as StreamId,
                    label.id,
                    label.name,
                    label.kind.as_deref().unwrap_or("system"),
                )
                .execute(&mut **tx)
                .await?;
            }
        }
        emit::apply_index(tx, &state, workspace, target.stream, page.index).await?;
        if let Some(batch) = &batch {
            emit::store_batch(
                tx,
                &state,
                workspace,
                target.stream,
                inbox,
                batch,
                &page.cursor.token(),
                change_count,
                now,
            )
            .await?;
        }
        prune_batches(tx, &state, workspace, target.stream, now - retention).await?;
        sqlx::query!(
            r#"UPDATE connectors.streams
                  SET phase = $2, cursor = $3, interval_seconds = $4, last_synced_at = $5,
                      last_change_at = CASE WHEN $6 THEN $5 ELSE last_change_at END,
                      consecutive_failures = 0, throttled_until = NULL
                WHERE id = $1"#,
            target.stream as StreamId,
            page.phase.as_str(),
            cursor_json,
            interval,
            now,
            changed,
        )
        .execute(&mut **tx)
        .await?;
        if matches!(target.health, Health::Throttled | Health::Failing) {
            health::set(tx, target.connection, Health::Ok, None, now).await?;
        }
        if page.more {
            // More is waiting: run again straight away, holding the
            // schedule row so the dispatcher does not double it.
            schedule::set_due(tx, workspace, target.stream, now + LEASE).await?;
            // Ordered after this run; no unique key, since this run (which
            // may itself be a continuation) still holds it.
            campbooks_kernel::jobs::enqueue_to::<Self>(
                tx,
                &SyncStreamPayload {
                    stream: target.stream,
                },
                EnqueueOptions::for_workspace(workspace.to_string())
                    .ordered_by(format!("{SYNC_STREAM}:{}", target.stream)),
            )
            .await?;
        } else {
            let next = now + TimeDelta::seconds(i64::from(interval));
            schedule::set_due(tx, workspace, target.stream, next).await?;
        }
        tracing::info!(
            %workspace, stream = %target.stream, changes = change_count,
            phase = page.phase.as_str(), more = page.more, "stream synced"
        );
        Ok(())
    }

    /// The provider refused the access token: drop it so the next run
    /// refreshes it, backing off; a grant refused run after run although it
    /// refreshes needs the member (`needs_reauth`, `access_refused`).
    async fn refused(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        // The access token was refused: drop it so the next run
        // refreshes it (and finds out if the grant is gone). A
        // grant refused run after run although it refreshes needs
        // the member.
        let failures = target.consecutive_failures.saturating_add(1);
        if u32::try_from(failures).unwrap_or(u32::MAX) >= self.deps.config.failing_after {
            let events = ctx.state().events.clone();
            health::needs_reauth(
                ctx.tx().await?,
                &events,
                &workspace.to_string(),
                target.connection,
                target.granted_by,
                "access_refused",
                now,
            )
            .await?;
            return Ok(());
        }
        let backoff = i64::from(poll.min_seconds)
            .saturating_mul(1_i64 << failures.clamp(0, 16))
            .min(i64::from(poll.max_seconds));
        let tx = ctx.tx().await?;
        credentials::forget_access_token(tx, target.connection).await?;
        sqlx::query!(
            "UPDATE connectors.streams SET consecutive_failures = $2 WHERE id = $1",
            target.stream as StreamId,
            failures,
        )
        .execute(&mut **tx)
        .await?;
        schedule::set_due(
            tx,
            workspace,
            target.stream,
            now + TimeDelta::seconds(backoff),
        )
        .await?;
        Ok(())
    }

    /// The provider failed: record it on the stream and the connection,
    /// and set the next run.
    async fn trouble(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        err: &GoogleError,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        match err {
            GoogleError::Unauthorized => self.refused(ctx, workspace, target, now).await,
            GoogleError::InvalidGrant => {
                let events = ctx.state().events.clone();
                health::needs_reauth(
                    ctx.tx().await?,
                    &events,
                    &workspace.to_string(),
                    target.connection,
                    target.granted_by,
                    "grant_revoked",
                    now,
                )
                .await?;
                Ok(())
            }
            GoogleError::RateLimited { retry_after } => {
                let wait = retry_after
                    .and_then(|d| TimeDelta::from_std(d).ok())
                    .unwrap_or(TimeDelta::seconds(i64::from(poll.min_seconds)));
                let until = now + wait;
                let tx = ctx.tx().await?;
                sqlx::query!(
                    "UPDATE connectors.streams SET throttled_until = $2 WHERE id = $1",
                    target.stream as StreamId,
                    until,
                )
                .execute(&mut **tx)
                .await?;
                health::set(
                    tx,
                    target.connection,
                    Health::Throttled,
                    Some("rate_limited"),
                    now,
                )
                .await?;
                schedule::set_due(tx, workspace, target.stream, until).await?;
                tracing::info!(%workspace, stream = %target.stream, %until, "throttled by the provider");
                Ok(())
            }
            GoogleError::NotFound | GoogleError::Unavailable(_) | GoogleError::Unexpected(_) => {
                let failures = target.consecutive_failures.saturating_add(1);
                // Back off from the minimum, doubling per failure, up to
                // the maximum interval.
                let max = i64::from(poll.max_seconds);
                let backoff = i64::from(poll.min_seconds)
                    .saturating_mul(1_i64 << failures.clamp(0, 16))
                    .min(max);
                let tx = ctx.tx().await?;
                sqlx::query!(
                    "UPDATE connectors.streams SET consecutive_failures = $2 WHERE id = $1",
                    target.stream as StreamId,
                    failures,
                )
                .execute(&mut **tx)
                .await?;
                if u32::try_from(failures).unwrap_or(u32::MAX) >= self.deps.config.failing_after {
                    health::set(
                        tx,
                        target.connection,
                        Health::Failing,
                        Some("provider_errors"),
                        now,
                    )
                    .await?;
                }
                schedule::set_due(
                    tx,
                    workspace,
                    target.stream,
                    now + TimeDelta::seconds(backoff),
                )
                .await?;
                tracing::warn!(%workspace, stream = %target.stream, failures, error = %err, "sync run failed");
                Ok(())
            }
        }
    }
}

/// The stream and its connection, read in a short transaction of its own.
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<Option<Target>, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"SELECT s.id AS "stream: StreamId", s.connection_id AS "connection: ConnectionId",
                  c.provider, c.granted_by AS "granted_by: UserId", c.health,
                  s.inbox_id AS "inbox: InboxId", s.phase, s.cursor, s.interval_seconds,
                  s.paused, s.consecutive_failures, s.throttled_until
             FROM connectors.streams s
             JOIN connectors.connections c ON c.id = s.connection_id
            WHERE s.id = $1"#,
        stream as StreamId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let cursor: Cursor = serde_json::from_value(row.cursor)
        .map_err(|e| JobError::fatal(format!("stream {stream} has an unreadable cursor: {e}")))?;
    Ok(Some(Target {
        stream: row.stream,
        connection: row.connection,
        provider: Provider::parse(&row.provider)?,
        granted_by: row.granted_by,
        health: Health::parse(&row.health)?,
        inbox: row.inbox,
        phase: Phase::parse(&row.phase)?,
        cursor,
        interval_seconds: row.interval_seconds,
        paused: row.paused,
        consecutive_failures: row.consecutive_failures,
        throttled_until: row.throttled_until,
    }))
}

/// Release the batches of `stream` older than `before`.
async fn prune_batches(
    tx: &mut Transaction<'static, Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    stream: StreamId,
    before: DateTime<Utc>,
) -> Result<(), JobError> {
    let old = sqlx::query_scalar!(
        "DELETE FROM connectors.batches WHERE stream_id = $1 AND created_at < $2 RETURNING id",
        stream as StreamId,
        before,
    )
    .fetch_all(&mut **tx)
    .await?;
    let key = workspace.to_string();
    for id in old {
        state
            .blobs
            .detach_owner(tx, &key, &BlobOwner::new(BATCH_OWNER, id))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
