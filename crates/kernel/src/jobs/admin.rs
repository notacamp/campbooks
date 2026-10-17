//! What an administrator may see and do with the queue (ADR 0015).
//!
//! `kernel.jobs` is written only through the kernel, so the admin
//! operations of the `system` module call these functions rather than
//! touching the table. Each takes the [`AdminGrant`] that only the
//! [`Admin`](crate::admin::Admin) extractor builds.
//!
//! - **Metadata only.** Nothing here selects `payload`: a payload holds ids
//!   today and may hold more tomorrow, and an operator needs to know what
//!   failed and why (kind, queue, attempts, error), not what was in it.
//! - **Failed jobs only.** [`retry`], [`retry_kind`] and [`discard`] match
//!   `state = 'failed'` in the statement itself, so a job a worker has
//!   claimed, or that already succeeded, can never be touched, however the
//!   calls race with the worker. Retrying is idempotent: the second call
//!   finds the job pending (or running, or done) and changes nothing.
//! - **And a schedule's stuck row.** [`discard_schedule`] deletes the one
//!   unfinished row of a schedule (its `schedule:<name>` key) unless a live
//!   worker holds it, so a row that blocks the schedule's ticks (forged
//!   before campbooks-cloud#207, or wedged) can be cleared; the next tick
//!   enqueues afresh. The tick heals such rows itself; this is the
//!   operator's way not to wait for it.
//! - **Across workspaces.** The queue is every workspace's: each function
//!   runs in the kernel scope ([`begin_kernel`](crate::db::begin_kernel)).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{JobId, NOTIFY_CHANNEL, Registry};
use crate::{Error, admin::AdminGrant, i18n};

/// Most failed jobs [`retry_kind`] requeues per call; call again for more.
pub const RETRY_BATCH: i64 = 1_000;

/// Largest page of [`failed`].
pub const MAX_PAGE: i64 = 200;

/// Page size of [`failed`] when none is asked for.
pub const DEFAULT_PAGE: i64 = 50;

/// The queue at a glance: one row per queue and kind with unfinished or
/// failed work. Succeeded jobs are pruned and not counted.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QueueSummary {
    /// When the counts were taken.
    pub at: DateTime<Utc>,
    /// Age of the oldest job that is ready to run and waiting, in seconds.
    pub oldest_ready_seconds: Option<i64>,
    /// Counts per queue and kind, sorted by queue then kind.
    pub kinds: Vec<KindSummary>,
    /// The dead jobs of the kinds an operator must hear about, by what they
    /// are for (each kind's `Watch`), sorted by code; every watched code is
    /// listed, with 0 when nothing died.
    pub watched: Vec<WatchedSummary>,
}

/// The dead jobs of the kinds watched under one code.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WatchedSummary {
    /// What the kinds are for (`suspension_fanout`: a suspension, or its
    /// lift, reaching the rules a member taught); the client words it.
    pub code: String,
    /// The kinds watched under it, sorted.
    pub kinds: Vec<String>,
    /// How many of their jobs are dead: until each is retried, what it
    /// carried has not happened.
    pub failed: i64,
}

/// The jobs of one kind on one queue.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct KindSummary {
    /// The queue they run on.
    pub queue: String,
    /// The job's wire name (`<module>.<job>`).
    pub kind: String,
    /// Pending and due now: waiting for a worker.
    pub ready: i64,
    /// Pending for later: new delayed work, or waiting out a retry backoff.
    pub scheduled: i64,
    /// Claimed by a worker.
    pub running: i64,
    /// Running, but past its lease: the worker stopped renewing it and the
    /// next upkeep hands it back.
    pub expired_leases: i64,
    /// Dead: out of attempts, or failed for good. Kept for inspection.
    pub failed: i64,
    /// Age of the oldest ready job of this kind, in seconds.
    pub oldest_ready_seconds: Option<i64>,
}

/// A dead job: what ran and why it stopped, never its payload.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FailedJob {
    /// The job's id.
    pub id: JobId,
    /// The queue it ran on.
    pub queue: String,
    /// The job's wire name.
    pub kind: String,
    /// The workspace it belongs to; `null` for instance-wide work.
    pub workspace_id: Option<String>,
    /// Attempts made.
    pub attempts: i32,
    /// Attempts it was allowed.
    pub max_attempts: i32,
    /// The last failure, truncated; never carries secrets.
    pub last_error: Option<String>,
    /// When it was enqueued.
    pub created_at: DateTime<Utc>,
    /// When it died.
    pub finished_at: Option<DateTime<Utc>>,
}

/// A page of dead jobs, newest first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FailedPage {
    /// The jobs.
    pub items: Vec<FailedJob>,
    /// Pass as `after` for the next page; `null` on the last one.
    pub next: Option<String>,
}

/// Which dead jobs [`failed`] lists.
#[derive(Debug, Clone, Default)]
pub struct FailedFilter {
    /// Only this kind.
    pub kind: Option<String>,
    /// Only this queue.
    pub queue: Option<String>,
}

/// What [`retry`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// It was dead and is pending again, with its attempts reset.
    Requeued,
    /// It is not dead (already requeued, running, or done): nothing changed.
    Unchanged,
    /// Another unfinished job of the same kind holds its unique key, so
    /// running it again would duplicate that work: nothing changed.
    Superseded,
    /// There is no such job.
    Missing,
}

/// What [`discard`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discard {
    /// It was dead and is gone.
    Discarded,
    /// It is not dead (retried meanwhile, running, or done): kept.
    Unchanged,
    /// There is no such job (already discarded).
    Missing,
}

/// What [`discard_schedule`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardSchedule {
    /// Its pending or stuck row is gone; the next tick enqueues afresh.
    Discarded,
    /// Nothing to discard: no unfinished row, or a live worker is running it.
    Unchanged,
    /// No schedule has that name.
    Unknown,
}

/// Counts of the queue by queue and kind, and of the dead jobs of the kinds
/// `registry` watches.
pub async fn summary(
    db: &PgPool,
    registry: &Registry,
    _grant: &AdminGrant,
) -> Result<QueueSummary, Error> {
    let mut tx = crate::db::begin_kernel(db).await?;
    let rows = sqlx::query!(
        r#"
        SELECT queue, kind,
               count(*) FILTER (WHERE state = 'pending' AND run_at <= now()) AS "ready!",
               count(*) FILTER (WHERE state = 'pending' AND run_at > now()) AS "scheduled!",
               count(*) FILTER (WHERE state = 'running') AS "running!",
               count(*) FILTER (WHERE state = 'running' AND locked_until < now())
                   AS "expired_leases!",
               count(*) FILTER (WHERE state = 'failed') AS "failed!",
               extract(epoch FROM now() - min(run_at)
                   FILTER (WHERE state = 'pending' AND run_at <= now()))::bigint
                   AS oldest_ready_seconds
          FROM kernel.jobs
         WHERE state <> 'succeeded'
         GROUP BY queue, kind
         ORDER BY queue, kind
        "#
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let kinds: Vec<KindSummary> = rows
        .into_iter()
        .map(|row| KindSummary {
            queue: row.queue,
            kind: row.kind,
            ready: row.ready,
            scheduled: row.scheduled,
            running: row.running,
            expired_leases: row.expired_leases,
            failed: row.failed,
            oldest_ready_seconds: row.oldest_ready_seconds,
        })
        .collect();
    let watched = registry
        .watched()
        .into_iter()
        .map(|(code, watched)| WatchedSummary {
            code: code.to_owned(),
            failed: kinds
                .iter()
                .filter(|k| watched.iter().any(|w| *w == k.kind))
                .map(|k| k.failed)
                .sum(),
            kinds: watched.into_iter().map(str::to_owned).collect(),
        })
        .collect();
    Ok(QueueSummary {
        at: Utc::now(),
        oldest_ready_seconds: kinds.iter().filter_map(|k| k.oldest_ready_seconds).max(),
        kinds,
        watched,
    })
}

/// A page of dead jobs, newest first. `after` is the `next` of the previous
/// page; `limit` is clamped to `1..=`[`MAX_PAGE`].
pub async fn failed(
    db: &PgPool,
    _grant: &AdminGrant,
    filter: &FailedFilter,
    after: Option<&str>,
    limit: Option<i64>,
) -> Result<FailedPage, Error> {
    let cursor = after.map(parse_cursor).transpose()?;
    let limit = limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let (after_at, after_id) = cursor.unzip();
    let mut tx = crate::db::begin_kernel(db).await?;
    let mut rows = sqlx::query_as!(
        FailedJob,
        r#"
        SELECT id AS "id: JobId", queue, kind, workspace_id, attempts, max_attempts,
               last_error, created_at, finished_at
          FROM kernel.jobs
         WHERE state = 'failed'
           AND ($1::text IS NULL OR kind = $1)
           AND ($2::text IS NULL OR queue = $2)
           AND ($3::timestamptz IS NULL OR (finished_at, id) < ($3, $4))
         ORDER BY finished_at DESC, id DESC
         LIMIT $5
        "#,
        filter.kind,
        filter.queue,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let more = i64::try_from(rows.len()).is_ok_and(|n| n > limit);
    if more {
        rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }
    let next = more
        .then(|| rows.last())
        .flatten()
        .and_then(|last| Some(format_cursor(last.finished_at?, last.id)));
    Ok(FailedPage { items: rows, next })
}

/// Put one dead job back on its queue: pending, due now, attempts reset.
pub async fn retry(db: &PgPool, _grant: &AdminGrant, id: JobId) -> Result<Retry, Error> {
    let id = Uuid::from(id);
    let mut tx = crate::db::begin_kernel(db).await?;
    let requeued = sqlx::query!(
        r#"
        UPDATE kernel.jobs j
           SET state = 'pending', run_at = now(), attempts = 0,
               finished_at = NULL, locked_by = NULL, locked_until = NULL,
               updated_at = now()
         WHERE j.id = $1 AND j.state = 'failed'
           AND (j.unique_key IS NULL OR NOT EXISTS (
                SELECT 1 FROM kernel.jobs o
                 WHERE o.kind = j.kind AND o.unique_key = j.unique_key
                   AND o.state IN ('pending', 'running')))
        RETURNING j.queue
        "#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = requeued {
        notify(&mut tx, &row.queue).await?;
        tx.commit().await?;
        return Ok(Retry::Requeued);
    }
    let state = sqlx::query_scalar!("SELECT state FROM kernel.jobs WHERE id = $1", id)
        .fetch_optional(&mut *tx)
        .await?;
    Ok(match state.as_deref() {
        None => Retry::Missing,
        Some("failed") => Retry::Superseded,
        Some(_) => Retry::Unchanged,
    })
}

/// Put dead jobs of one kind back on their queue (optionally one queue),
/// oldest first, at most [`RETRY_BATCH`] per call. Jobs whose unique key is
/// held by another unfinished job are skipped. Returns how many were
/// requeued; a result equal to the batch size means there may be more.
pub async fn retry_kind(
    db: &PgPool,
    _grant: &AdminGrant,
    kind: &str,
    queue: Option<&str>,
) -> Result<u64, Error> {
    let mut tx = crate::db::begin_kernel(db).await?;
    let queues = sqlx::query_scalar!(
        r#"
        WITH locked AS (
            SELECT j.id, j.unique_key
              FROM kernel.jobs j
             WHERE j.state = 'failed' AND j.kind = $1
               AND ($2::text IS NULL OR j.queue = $2)
               AND (j.unique_key IS NULL OR NOT EXISTS (
                    SELECT 1 FROM kernel.jobs o
                     WHERE o.kind = j.kind AND o.unique_key = j.unique_key
                       AND o.state IN ('pending', 'running')))
             ORDER BY j.finished_at, j.id
             LIMIT $3
               FOR UPDATE SKIP LOCKED
        ),
        picked AS (
            -- Two dead jobs sharing a unique key must not both come back.
            SELECT id FROM (
                SELECT id, unique_key,
                       row_number() OVER (PARTITION BY unique_key ORDER BY id) AS n
                  FROM locked
            ) l
             WHERE l.unique_key IS NULL OR l.n = 1
        )
        UPDATE kernel.jobs j
           SET state = 'pending', run_at = now(), attempts = 0,
               finished_at = NULL, locked_by = NULL, locked_until = NULL,
               updated_at = now()
          FROM picked
         WHERE j.id = picked.id
        RETURNING j.queue
        "#,
        kind,
        queue,
        RETRY_BATCH,
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut touched = queues.clone();
    touched.sort_unstable();
    touched.dedup();
    for queue in &touched {
        notify(&mut tx, queue).await?;
    }
    tx.commit().await?;
    Ok(queues.len() as u64)
}

/// Delete one dead job. A job that is not dead is never deleted.
pub async fn discard(db: &PgPool, _grant: &AdminGrant, id: JobId) -> Result<Discard, Error> {
    let id = Uuid::from(id);
    let mut tx = crate::db::begin_kernel(db).await?;
    let deleted = sqlx::query!(
        "DELETE FROM kernel.jobs WHERE id = $1 AND state = 'failed'",
        id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if deleted > 0 {
        tx.commit().await?;
        return Ok(Discard::Discarded);
    }
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM kernel.jobs WHERE id = $1) AS "exists!""#,
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(if exists {
        Discard::Unchanged
    } else {
        Discard::Missing
    })
}

/// Delete schedule `name`'s unfinished row (the job holding its
/// `schedule:<name>` unique key) when it is pending, belongs to a workspace
/// (a forgery), or runs on a lease that has expired or is longer than any
/// worker gives: whatever blocks the schedule's next tick. A run a live
/// worker holds is kept.
pub async fn discard_schedule(
    state: &crate::AppState,
    _grant: &AdminGrant,
    name: &str,
) -> Result<DiscardSchedule, Error> {
    let Some(schedule) = state
        .events
        .registry()
        .schedules()
        .iter()
        .find(|s| s.name == name)
    else {
        return Ok(DiscardSchedule::Unknown);
    };
    let (_, leased) = super::worker::stuck_after(&state.config.jobs, schedule.every);
    let mut tx = crate::db::begin_kernel(&state.db).await?;
    let deleted = sqlx::query!(
        r#"
        DELETE FROM kernel.jobs
         WHERE kind = $1 AND unique_key = 'schedule:' || $1
           AND state IN ('pending', 'running')
           AND (state = 'pending' OR workspace_id IS NOT NULL
                OR locked_until < now() OR locked_until > now() + make_interval(secs => $2))
        "#,
        schedule.name,
        leased,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    if deleted > 0 {
        tracing::warn!(
            schedule = schedule.name,
            "an administrator discarded a schedule's pending or stuck row"
        );
        return Ok(DiscardSchedule::Discarded);
    }
    Ok(DiscardSchedule::Unchanged)
}

async fn notify(conn: &mut sqlx::PgConnection, queue: &str) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT pg_notify($1, $2)", NOTIFY_CHANNEL, queue)
        .fetch_one(conn)
        .await?;
    Ok(())
}

fn format_cursor(at: DateTime<Utc>, id: JobId) -> String {
    format!("{}.{id}", at.timestamp_micros())
}

fn parse_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid), Error> {
    let parsed = cursor.split_once('.').and_then(|(micros, id)| {
        Some((
            DateTime::from_timestamp_micros(micros.parse().ok()?)?,
            id.parse::<Uuid>().ok()?,
        ))
    });
    parsed.ok_or_else(|| {
        Error::BadRequest(
            i18n::MESSAGES
                .reason("invalid_query")
                .arg("reason", "`after` is not a cursor from a previous page"),
        )
    })
}
