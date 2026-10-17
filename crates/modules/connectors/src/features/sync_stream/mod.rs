//! `connectors.sync_stream` — one run of one stream: refresh the token if
//! needed, take budget, read one page through the adapter, then in the
//! job's transaction apply the adapter's index updates, store and announce
//! the batch (`connectors.messages_changed`), advance the cursor and set the
//! next run.
//!
//! A run first makes the stream's pending outbound changes at the provider
//! (`gmail::write`), records how each ended, and only then reads: pushes
//! and pulls of one stream never interleave, and share its budget, backoff
//! and health.
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
    contract::{BATCH_CONTENT_TYPE, MessageChange, MessageChangeBatch},
    deps::Deps,
    domain::{
        credentials::{self, TokenError},
        emit, health,
        model::{ConnectionId, Health, InboxId, Phase, Provider, StreamId},
        outbound::{self, Backlog},
        push::Push,
        removal::BATCH_OWNER,
        schedule::{self, LEASE, Rate, SYNC_STREAM, SyncStreamPayload},
    },
    gmail::{self, Reader},
    google::LabelInfo,
    mail::{IndexOp, ReadError},
    metrics::{self, SyncOutcome},
    provider::ProviderError,
    zoho,
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
    provider_server: Option<String>,
    mailbox_key: String,
    inbox: Option<InboxId>,
    phase: Phase,
    cursor: serde_json::Value,
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
        let configured = match target.provider {
            Provider::Google => self.deps.google.is_some(),
            Provider::Zoho => self.deps.zoho.is_some(),
        };
        let (true, Some(inbox)) = (configured, target.inbox) else {
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
                    metrics::sync_finished(target.provider, SyncOutcome::Refused);
                    return self
                        .needs_reauth(ctx, workspace, &target, reason, now)
                        .await;
                }
                Err(TokenError::Provider(err)) => {
                    return self.trouble(ctx, workspace, &target, &err, now).await;
                }
                Err(TokenError::Database(err)) => return Err(err.into()),
            };

        // Stale changes are given up before anything is taken: never made
        // late after a long pause.
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));
        outbound::expire(ctx.pool(), workspace, target.stream, now - retention, now).await?;
        let backlog = outbound::backlog(ctx.pool(), workspace, target.stream, now).await?;
        let read = match target.provider {
            Provider::Google => {
                self.run_gmail(ctx, workspace, &target, &token, &backlog, now)
                    .await
            }
            Provider::Zoho => {
                self.run_zoho(ctx, workspace, &target, &token, &backlog, now)
                    .await
            }
        }
        .inspect_err(|_| metrics::sync_finished(target.provider, SyncOutcome::Failed))?;
        // Without a page the run waits or has recorded its trouble, and
        // counted it.
        let Some(read) = read else {
            return Ok(());
        };
        let applied = self
            .apply(ctx, workspace, &target, inbox, read, backlog.last(), now)
            .await;
        let outcome = if applied.is_ok() {
            SyncOutcome::Ok
        } else {
            SyncOutcome::Failed
        };
        metrics::sync_finished(target.provider, outcome);
        applied
    }
}

/// What a run read, whatever the provider: applied in the job's
/// transaction.
#[derive(Debug)]
struct RunPage {
    changes: Vec<MessageChange>,
    index: Vec<IndexOp>,
    /// A fresh Gmail `labels.list`.
    labels: Option<Vec<LabelInfo>>,
    /// The cursor after the page, as stored.
    cursor: serde_json::Value,
    /// The cursor as the event carries it.
    token: String,
    phase: Phase,
    more: bool,
    /// Gmail's history id the run read up to (closes open write windows).
    read_up_to: Option<i64>,
}

impl SyncStream {
    /// The number of distinct messages `backlog` touches.
    fn messages(backlog: &Backlog) -> usize {
        backlog
            .pending
            .iter()
            .map(|p| p.provider_message_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    /// Record a push's outcome on its own (the writes happened, whatever
    /// the read does next). Returns the trouble that stopped it.
    async fn record_push(
        ctx: &JobContext,
        workspace: WorkspaceId,
        stream: StreamId,
        pushed: Push,
        now: DateTime<Utc>,
    ) -> Result<Option<ProviderError>, JobError> {
        let mut tx = ctx.pool().begin_for(workspace).await?;
        outbound::record_outcome(&mut tx, stream, &pushed.outcome, now).await?;
        tx.commit().await?;
        Ok(pushed.trouble)
    }

    /// Wait for `wait` before the stream's next run.
    async fn wait_for_budget(
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        stream: StreamId,
        wait: std::time::Duration,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let at = now + TimeDelta::from_std(wait).unwrap_or(TimeDelta::seconds(1));
        schedule::set_due(ctx.tx().await?, workspace, stream, at).await?;
        Ok(())
    }

    /// A Gmail run: budget, push, read. `None` once the run has ended
    /// without a page (waiting, trouble recorded).
    async fn run_gmail(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        token: &str,
        backlog: &Backlog,
        now: DateTime<Utc>,
    ) -> Result<Option<RunPage>, JobError> {
        let Some(google) = self.deps.google.clone() else {
            return Ok(None);
        };
        let messages = Self::messages(backlog);
        let cost = gmail::UNITS_PER_RUN
            + if messages == 0 {
                0
            } else {
                gmail::write::units(messages)
            };
        let rate = self.deps.config.google.units_per_second;
        if let Err(wait) = schedule::take_budget(ctx.pool(), target.provider, cost, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(None);
        }
        if backlog.last().is_some() {
            let pushed = gmail::write::push(&google.gmail(token), backlog).await;
            if let Some(err) = Self::record_push(ctx, workspace, target.stream, pushed, now).await?
            {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
        }
        let cursor: gmail::Cursor = serde_json::from_value(target.cursor.clone()).map_err(|e| {
            JobError::fatal(format!(
                "stream {} has an unreadable cursor: {e}",
                target.stream
            ))
        })?;
        let state = ctx.state().clone();
        let reader = Reader {
            gmail: google.gmail(token),
            pool: ctx.pool(),
            blobs: &state.blobs,
            workspace,
            stream: target.stream,
            phase: target.phase,
            cursor,
            now,
            backfill_days: self.deps.config.gmail.backfill_days,
            max_message_bytes: self.deps.config.gmail.max_message_bytes,
            run_budget_bytes: self.deps.config.gmail.run_budget_bytes,
        };
        let page = match reader.read().await {
            Ok(page) => page,
            Err(ReadError::Provider(err)) => {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
            Err(other) => return Err(JobError::retry(other.to_string())),
        };
        Ok(Some(RunPage {
            cursor: serde_json::to_value(&page.cursor)
                .map_err(|e| JobError::fatal(format!("encoding a cursor: {e}")))?,
            token: page.cursor.token(),
            read_up_to: page
                .cursor
                .history_id
                .as_deref()
                .and_then(|h| h.parse::<i64>().ok()),
            changes: page.changes,
            index: page.index,
            labels: page.labels,
            phase: page.phase,
            more: page.more,
        }))
    }

    /// A Zoho run: the data centre, the login's budget, push, read. `None`
    /// once the run has ended without a page.
    async fn run_zoho(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        token: &str,
        backlog: &Backlog,
        now: DateTime<Utc>,
    ) -> Result<Option<RunPage>, JobError> {
        let Some(zoho) = self.deps.zoho.clone() else {
            return Ok(None);
        };
        let Some(centre) = target
            .provider_server
            .as_deref()
            .and_then(|server| zoho.centre(server))
        else {
            self.needs_reauth(ctx, workspace, target, "unknown_data_centre", now)
                .await?;
            return Ok(None);
        };
        let cursor: zoho::read::Cursor =
            serde_json::from_value(target.cursor.clone()).map_err(|e| {
                JobError::fatal(format!(
                    "stream {} has an unreadable cursor: {e}",
                    target.stream
                ))
            })?;
        let config = &self.deps.config.zoho;
        let cost = zoho::read::requests_per_run(cursor.folders.len())
            + zoho::write::requests(Self::messages(backlog));
        let key = schedule::connection_budget(target.connection);
        let rate = Rate::per_minute(config.requests_per_minute);
        if let Err(wait) = schedule::take(ctx.pool(), &key, cost, rate).await? {
            metrics::sync_finished(target.provider, SyncOutcome::Throttled);
            Self::wait_for_budget(ctx, workspace, target.stream, wait, now).await?;
            return Ok(None);
        }
        let mail = zoho.mail(&centre, token, &target.mailbox_key);
        if backlog.last().is_some() {
            let pushed = zoho::write::push(&mail, backlog, cursor.inbox()).await;
            if let Some(err) = Self::record_push(ctx, workspace, target.stream, pushed, now).await?
            {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
        }
        let state = ctx.state().clone();
        let reader = zoho::read::Reader {
            mail,
            pool: ctx.pool(),
            blobs: &state.blobs,
            workspace,
            stream: target.stream,
            phase: target.phase,
            cursor,
            now,
            config,
        };
        let page = match reader.read().await {
            Ok(page) => page,
            Err(ReadError::Provider(err)) => {
                self.trouble(ctx, workspace, target, &err, now).await?;
                return Ok(None);
            }
            Err(other) => return Err(JobError::retry(other.to_string())),
        };
        Ok(Some(RunPage {
            cursor: serde_json::to_value(&page.cursor)
                .map_err(|e| JobError::fatal(format!("encoding a cursor: {e}")))?,
            token: page.cursor.token(),
            read_up_to: None,
            changes: page.changes,
            index: page.index,
            labels: None,
            phase: page.phase,
            more: page.more,
        }))
    }

    /// The grant stopped working (`reason`).
    async fn needs_reauth(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        reason: &'static str,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
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
        Ok(())
    }
}

impl SyncStream {
    /// Apply a page in the job's transaction. `pushed_up_to` is the newest
    /// outbound change this run made: newer ones arrived during the run and
    /// call for another at once.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call site; the values are distinct"
    )]
    async fn apply(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        target: &Target,
        inbox: InboxId,
        mut page: RunPage,
        pushed_up_to: Option<uuid::Uuid>,
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
        let cursor_json = page.cursor;
        let interval =
            schedule::next_interval(target.interval_seconds, changed, &self.deps.config.poll);
        let retention = TimeDelta::days(i64::from(self.deps.config.batch_retention_days));

        let tx = ctx.tx().await?;
        // The connection before the stream (removal's order).
        health::lock(tx, target.connection).await?;
        if let Some(labels) = &page.labels {
            replace_labels(tx, workspace, target.stream, labels).await?;
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
                &page.token,
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
        if settle_outbound(
            tx,
            target.stream,
            pushed_up_to,
            now - retention,
            page.read_up_to,
        )
        .await?
        {
            page.more = true;
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
        health::lock(tx, target.connection).await?;
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
        err: &ProviderError,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let poll = &self.deps.config.poll;
        let outcome = match err {
            ProviderError::Unauthorized | ProviderError::InvalidGrant => SyncOutcome::Refused,
            ProviderError::RateLimited { .. } => SyncOutcome::Throttled,
            ProviderError::NotFound
            | ProviderError::Unavailable(_)
            | ProviderError::Unexpected(_) => SyncOutcome::Failed,
        };
        metrics::sync_finished(target.provider, outcome);
        match err {
            ProviderError::Unauthorized => self.refused(ctx, workspace, target, now).await,
            ProviderError::InvalidGrant => {
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
            ProviderError::RateLimited { retry_after } => {
                let wait = retry_after
                    .and_then(|d| TimeDelta::from_std(d).ok())
                    .unwrap_or(TimeDelta::seconds(i64::from(poll.min_seconds)));
                let until = now + wait;
                let tx = ctx.tx().await?;
                health::lock(tx, target.connection).await?;
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
            ProviderError::NotFound
            | ProviderError::Unavailable(_)
            | ProviderError::Unexpected(_) => {
                let failures = target.consecutive_failures.saturating_add(1);
                // Back off from the minimum, doubling per failure, up to
                // the maximum interval.
                let max = i64::from(poll.max_seconds);
                let backoff = i64::from(poll.min_seconds)
                    .saturating_mul(1_i64 << failures.clamp(0, 16))
                    .min(max);
                let tx = ctx.tx().await?;
                health::lock(tx, target.connection).await?;
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

/// Replace the cached Gmail label names of `stream` with `labels`.
async fn replace_labels(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    labels: &[LabelInfo],
) -> Result<(), JobError> {
    sqlx::query!(
        "DELETE FROM connectors.gmail_labels WHERE stream_id = $1",
        stream as StreamId
    )
    .execute(&mut **tx)
    .await?;
    for label in labels {
        sqlx::query!(
            r#"INSERT INTO connectors.gmail_labels (workspace_id, stream_id, label_id, name, kind)
                       VALUES ($1, $2, $3, $4, $5)"#,
            workspace as WorkspaceId,
            stream as StreamId,
            label.id,
            label.name,
            label.kind.as_deref().unwrap_or("system"),
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// The end of a run's outbound work, in its transaction (after the stream
/// row is locked by the run's update, so a change recorded concurrently is
/// either seen here or enqueues its own run): prune old changes and close
/// open write windows at the history id the run read up to. Returns whether changes arrived
/// during this run (another run is needed now).
async fn settle_outbound(
    tx: &mut Transaction<'static, Postgres>,
    stream: StreamId,
    pushed_up_to: Option<uuid::Uuid>,
    before: DateTime<Utc>,
    read_up_to: Option<i64>,
) -> Result<bool, JobError> {
    outbound::prune(tx, stream, before, read_up_to).await?;
    Ok(outbound::more_pending(tx, stream, pushed_up_to).await?)
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
                  c.provider, c.provider_server, c.granted_by AS "granted_by: UserId", c.health,
                  s.mailbox_key, s.inbox_id AS "inbox: InboxId", s.phase, s.cursor,
                  s.interval_seconds,
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
    Ok(Some(Target {
        stream: row.stream,
        connection: row.connection,
        provider: Provider::parse(&row.provider)?,
        provider_server: row.provider_server,
        mailbox_key: row.mailbox_key,
        granted_by: row.granted_by,
        health: Health::parse(&row.health)?,
        inbox: row.inbox,
        phase: Phase::parse(&row.phase)?,
        cursor: row.cursor,
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

/// Read one page of `stream` without pushing or applying it: the changes
/// it would emit, as JSON (tests of the reader's overlay).
#[cfg(test)]
pub(crate) async fn read_page(
    state: &campbooks_kernel::AppState,
    deps: &Deps,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Vec<serde_json::Value> {
    let mut tx = state.db.begin_for(workspace).await.unwrap();
    let row = sqlx::query!(
        r#"SELECT connection_id AS "connection: ConnectionId", phase, cursor
             FROM connectors.streams WHERE id = $1"#,
        stream as StreamId
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let token = credentials::access_token(&state.db, deps, workspace, row.connection)
        .await
        .unwrap_or_else(|_| panic!("a token"));
    let google = deps.google.clone().unwrap();
    let reader = Reader {
        gmail: google.gmail(&token),
        pool: &state.db,
        blobs: &state.blobs,
        workspace,
        stream,
        phase: Phase::parse(&row.phase).unwrap(),
        cursor: serde_json::from_value(row.cursor).unwrap(),
        now: deps.clock.now(),
        backfill_days: deps.config.gmail.backfill_days,
        max_message_bytes: deps.config.gmail.max_message_bytes,
        run_budget_bytes: deps.config.gmail.run_budget_bytes,
    };
    let page = reader.read().await.unwrap();
    page.changes
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect()
}

#[cfg(test)]
mod tests;
