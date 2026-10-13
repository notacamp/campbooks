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

use super::model::{Provider, StreamId};
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

/// Enqueue `connectors.sync_stream` for `stream`.
async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<()> {
    let key = format!("{SYNC_STREAM}:{stream}");
    jobs::enqueue_to::<crate::features::sync_stream::SyncStream>(
        tx,
        &SyncStreamPayload { stream },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(key.clone())
            .ordered_by(key),
    )
    .await?;
    Ok(())
}

/// Claim the streams due at `now` (at most [`DISPATCH_BATCH`]) and enqueue
/// their runs. Runs in a transaction that sets no workspace: the scheduling
/// index is the one thing it reads across workspaces (ids and times).
/// Returns how many it enqueued.
pub async fn dispatch(pool: &PgPool, now: DateTime<Utc>) -> Result<usize> {
    let mut tx = pool.begin().await?;
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
    for row in &due {
        enqueue(&mut tx, row.workspace, row.stream).await?;
    }
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

/// Take `cost` units of `provider`'s budget (a token bucket refilled at
/// `rate` per second, holding at most two seconds' worth). `Err` holds how
/// long until there is enough.
pub async fn take_budget(
    pool: &PgPool,
    provider: Provider,
    cost: u32,
    rate: u32,
) -> Result<Result<(), Duration>> {
    let rate = f64::from(rate);
    let burst = rate * 2.0;
    let cost = f64::from(cost).min(burst);
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"INSERT INTO connectors.provider_budgets (provider, tokens, refilled_at)
           VALUES ($1, $2, clock_timestamp())
           ON CONFLICT (provider) DO NOTHING"#,
        provider.as_str(),
        burst,
    )
    .execute(&mut *tx)
    .await?;
    let available = sqlx::query_scalar!(
        r#"SELECT least($2, tokens + extract(epoch FROM clock_timestamp() - refilled_at)::float8 * $3)
                  AS "available!"
             FROM connectors.provider_budgets WHERE provider = $1 FOR UPDATE"#,
        provider.as_str(),
        burst,
        rate,
    )
    .fetch_one(&mut *tx)
    .await?;
    let granted = available >= cost;
    sqlx::query!(
        "UPDATE connectors.provider_budgets SET tokens = $2, refilled_at = clock_timestamp()
          WHERE provider = $1",
        provider.as_str(),
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
}
