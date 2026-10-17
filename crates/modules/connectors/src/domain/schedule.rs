//! When streams run: the `sync_due` index, adaptive intervals, and the
//! per-provider budget.
//!
//! A stream's next run is one row in `connectors.sync_due`. The one-minute
//! dispatcher claims due rows and enqueues `connectors.sync_stream` (unique
//! per stream, so ticks, "sync now" and pushes collapse), pushing the row
//! [`LEASE`] into the future; a run that finishes sets the real next time.
//! A run that dies leaves the leased row, which comes due again: the
//! schedule never depends on a job surviving.

use std::time::Duration;

use campbooks_kernel::{
    Result,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

use super::model::{ConnectionId, Provider, StreamId};
use crate::config::PollConfig;

/// How long a dispatched stream's row stays out of the dispatcher's way.
pub const LEASE: TimeDelta = TimeDelta::minutes(15);

/// Most streams one dispatcher tick enqueues.
pub const DISPATCH_BATCH: i64 = 500;

/// The payload of `connectors.sync_stream`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStreamPayload {
    /// The stream.
    pub stream: StreamId,
}

/// The job kind's name (`features::sync_stream::SyncStream::NAME`).
pub const SYNC_STREAM: &str = "connectors.sync_stream";

/// Make `stream` due at `at`.
pub async fn set_due(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    stream: StreamId,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO connectors.sync_due (stream_id, workspace_id, due_at) VALUES ($1, $2, $3)
           ON CONFLICT (stream_id) DO UPDATE SET due_at = $3"#,
        stream as StreamId,
        workspace as WorkspaceId,
        at,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Take `stream` off the schedule (paused until the member reconnects).
pub async fn unschedule(conn: &mut PgConnection, stream: StreamId) -> Result<()> {
    sqlx::query!(
        "DELETE FROM connectors.sync_due WHERE stream_id = $1",
        stream as StreamId
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Enqueue a run of `stream` now, in `tx`, and lease its schedule row.
/// A run already pending absorbs this one.
pub async fn run_now(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    now: DateTime<Utc>,
) -> Result<()> {
    set_due(tx, workspace, stream, now + LEASE).await?;
    enqueue(tx, workspace, stream).await
}

/// Enqueue `connectors.sync_stream` for `stream`, in `tx`'s workspace.
async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<()> {
    jobs::enqueue_to::<crate::features::sync_stream::SyncStream>(
        tx,
        &SyncStreamPayload { stream },
        run_options(workspace, stream),
    )
    .await?;
    Ok(())
}

/// A run's enqueue options: one unfinished run per stream, in order.
fn run_options(workspace: WorkspaceId, stream: StreamId) -> EnqueueOptions {
    let key = format!("{SYNC_STREAM}:{stream}");
    EnqueueOptions::for_workspace(workspace.to_string())
        .unique(key.clone())
        .ordered_by(key)
}

/// Enqueue the runs of the `due` streams from the dispatcher's transaction,
/// which works in no workspace: each in its stream's workspace, set for
/// that enqueue only (`jobs::enqueue_to_in`), so nothing after it runs in
/// the last stream's workspace (campbooks-cloud#207).
async fn enqueue_due(
    tx: &mut Transaction<'_, Postgres>,
    due: &[(WorkspaceId, StreamId)],
) -> Result<()> {
    for &(workspace, stream) in due {
        jobs::enqueue_to_in::<crate::features::sync_stream::SyncStream>(
            tx,
            &workspace.to_string(),
            &SyncStreamPayload { stream },
            run_options(workspace, stream),
        )
        .await?;
    }
    Ok(())
}

/// Claim the streams due at `now` (at most [`DISPATCH_BATCH`]) and enqueue
/// their runs. Runs in the maintenance scope (`begin_maintenance`, allowed
/// for this file in `architecture.toml`): the scheduling index is the one
/// thing it reads across workspaces (ids and times, through its maintenance
/// policies), and each run is enqueued in its stream's workspace, set for
/// that enqueue only (the queue admits no other). Returns how many it
/// enqueued.
pub async fn dispatch(pool: &PgPool, now: DateTime<Utc>) -> Result<usize> {
    let mut tx = campbooks_kernel::db::begin_maintenance(pool).await?;
    let due = sqlx::query!(
        r#"UPDATE connectors.sync_due d SET due_at = $2
            WHERE d.stream_id IN (
                SELECT stream_id FROM connectors.sync_due
                 WHERE due_at <= $1
                 ORDER BY due_at
                 LIMIT $3
                 FOR UPDATE SKIP LOCKED)
        RETURNING d.stream_id AS "stream: StreamId", d.workspace_id AS "workspace: WorkspaceId""#,
        now,
        now + LEASE,
        DISPATCH_BATCH,
    )
    .fetch_all(&mut *tx)
    .await?;
    let due: Vec<(WorkspaceId, StreamId)> =
        due.iter().map(|row| (row.workspace, row.stream)).collect();
    enqueue_due(&mut tx, &due).await?;
    tx.commit().await?;
    Ok(due.len())
}

/// The next polling interval: the minimum after a run with changes, else
/// double the current one, never above the maximum.
pub fn next_interval(current_seconds: i32, changed: bool, poll: &PollConfig) -> i32 {
    let min = i32::try_from(poll.min_seconds).unwrap_or(i32::MAX);
    let max = i32::try_from(poll.max_seconds).unwrap_or(i32::MAX);
    if changed {
        min
    } else {
        current_seconds.saturating_mul(2).clamp(min, max)
    }
}

/// The budget key of one connection's own requests (a Zoho login).
pub fn connection_budget(connection: ConnectionId) -> String {
    format!("zoho:{connection}")
}

/// What a budget refills by and holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rate {
    /// Units added per second.
    pub per_second: f64,
    /// Seconds' worth the bucket holds at most.
    pub burst_seconds: f64,
}

impl Rate {
    /// Google's quota: `units` per second, two seconds' worth.
    pub fn per_second(units: u32) -> Self {
        Self {
            per_second: f64::from(units),
            burst_seconds: 2.0,
        }
    }

    /// A per-minute allowance, a minute's worth at once.
    pub fn per_minute(requests: u32) -> Self {
        Self {
            per_second: f64::from(requests) / 60.0,
            burst_seconds: 60.0,
        }
    }
}

/// Take `cost` units of `provider`'s budget (a token bucket refilled at
/// `rate` per second, holding at most two seconds' worth). `Err` holds how
/// long until there is enough.
pub async fn take_budget(
    pool: &PgPool,
    provider: Provider,
    cost: u32,
    rate: u32,
) -> Result<Result<(), Duration>> {
    take(pool, provider.as_str(), cost, Rate::per_second(rate)).await
}

/// Take `cost` units of the budget `key` (a token bucket shared by every
/// process, refilled at `rate`). `Err` holds how long until there is
/// enough.
pub async fn take(pool: &PgPool, key: &str, cost: u32, rate: Rate) -> Result<Result<(), Duration>> {
    let burst = rate.per_second * rate.burst_seconds;
    let rate = rate.per_second;
    let cost = f64::from(cost).min(burst);
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"INSERT INTO connectors.provider_budgets (provider, tokens, refilled_at)
           VALUES ($1, $2, clock_timestamp())
           ON CONFLICT (provider) DO NOTHING"#,
        key,
        burst,
    )
    .execute(&mut *tx)
    .await?;
    let available = sqlx::query_scalar!(
        r#"SELECT least($2, tokens + extract(epoch FROM clock_timestamp() - refilled_at)::float8 * $3)
                  AS "available!"
             FROM connectors.provider_budgets WHERE provider = $1 FOR UPDATE"#,
        key,
        burst,
        rate,
    )
    .fetch_one(&mut *tx)
    .await?;
    let granted = available >= cost;
    sqlx::query!(
        "UPDATE connectors.provider_budgets SET tokens = $2, refilled_at = clock_timestamp()
          WHERE provider = $1",
        key,
        if granted { available - cost } else { available },
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if granted {
        Ok(Ok(()))
    } else {
        Ok(Err(Duration::from_secs_f64(
            ((cost - available) / rate).max(0.1),
        )))
    }
}

/// Drop the budget `key` (its connection is gone).
pub async fn forget_budget(conn: &mut PgConnection, key: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM connectors.provider_budgets WHERE provider = $1",
        key
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_back_off_and_reset() {
        let poll = PollConfig {
            min_seconds: 60,
            max_seconds: 600,
        };
        assert_eq!(next_interval(60, false, &poll), 120);
        assert_eq!(next_interval(120, false, &poll), 240);
        assert_eq!(next_interval(480, false, &poll), 600);
        assert_eq!(next_interval(600, false, &poll), 600);
        assert_eq!(next_interval(600, true, &poll), 60);
        assert_eq!(next_interval(1, false, &poll), 60);
    }

    /// The dispatcher's enqueues each run in their stream's workspace, and
    /// leave its transaction working in none (campbooks-cloud#207).
    #[sqlx::test(migrations = "../../../migrations")]
    async fn dispatched_runs_leave_no_workspace_set(db: PgPool) {
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        let due = [
            (WorkspaceId::new(), StreamId::new()),
            (WorkspaceId::new(), StreamId::new()),
        ];
        let mut tx = campbooks_kernel::db::begin_maintenance(&pool)
            .await
            .unwrap();
        enqueue_due(&mut tx, &due).await.unwrap();
        let workspace: Option<String> = sqlx::query_scalar(
            "SELECT nullif(current_setting('campbooks.workspace_id', true), '')",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(workspace, None);
        tx.commit().await.unwrap();
        let mut queued: Vec<String> = sqlx::query_scalar(
            "SELECT workspace_id FROM kernel.jobs WHERE kind = 'connectors.sync_stream'",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        queued.sort();
        let mut expected: Vec<String> = due.iter().map(|(w, _)| w.to_string()).collect();
        expected.sort();
        assert_eq!(queued, expected);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_budget_is_shared_and_refills(db: PgPool) {
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        // 100 units/s holds 200.
        assert_eq!(
            take_budget(&pool, Provider::Google, 150, 100)
                .await
                .unwrap(),
            Ok(())
        );
        // 50 left (plus a trickle): 150 more has to wait about a second.
        let wait = take_budget(&pool, Provider::Google, 150, 100)
            .await
            .unwrap()
            .unwrap_err();
        assert!(
            wait > Duration::from_millis(500) && wait <= Duration::from_secs(2),
            "{wait:?}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn each_zoho_login_has_its_own_budget(db: PgPool) {
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        let (one, two) = (
            connection_budget(ConnectionId::new()),
            connection_budget(ConnectionId::new()),
        );
        // 60 a minute: a minute's worth at once, then wait.
        let rate = Rate::per_minute(60);
        assert_eq!(take(&pool, &one, 60, rate).await.unwrap(), Ok(()));
        let wait = take(&pool, &one, 30, rate).await.unwrap().unwrap_err();
        assert!(wait > Duration::from_secs(20), "{wait:?}");
        // Another login is untouched.
        assert_eq!(take(&pool, &two, 60, rate).await.unwrap(), Ok(()));
    }
}
