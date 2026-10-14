//! The kernel's own recurring maintenance: retention of finished jobs and of
//! the event log, so neither keeps history (ids, and the names of what
//! happened) for longer than the operator chose.

use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use super::{Job, JobContext, JobError};

/// Rows deleted per statement, so one prune never holds a long lock.
const PRUNE_BATCH: i64 = 5_000;

/// Delete succeeded jobs older than `CAMPBOOKS_JOBS__RETENTION_HOURS` and
/// failed jobs older than `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS`, by when
/// they finished. Scheduled hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneJobs;

impl Job for PruneJobs {
    const NAME: &'static str = "kernel.prune_jobs";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let jobs = &ctx.state().config.jobs;
        let hours = i32::try_from(jobs.retention_hours).unwrap_or(i32::MAX);
        let failed_days = i32::try_from(jobs.failed_retention_days).unwrap_or(i32::MAX);
        // Failed deliveries of events that must settle stay until an
        // operator acts: their absence would read as settled.
        let settling = ctx.state().events.registry().settling_events();
        let mut pruned = 0_u64;
        loop {
            let deleted = sqlx::query!(
                r#"
                DELETE FROM kernel.jobs
                 WHERE id IN (
                     SELECT id FROM kernel.jobs
                      WHERE (state = 'succeeded'
                             AND finished_at < now() - make_interval(hours => $1))
                         OR (state = 'failed'
                             AND finished_at < now() - make_interval(days => $2)
                             AND NOT EXISTS (
                                 SELECT 1 FROM kernel.events e
                                  WHERE e.id = kernel.jobs.event_id AND e.name = ANY($4)))
                      LIMIT $3)
                "#,
                hours,
                failed_days,
                PRUNE_BATCH,
                &settling as &[&str],
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            pruned += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(
            pruned,
            retention_hours = hours,
            failed_retention_days = failed_days,
            "pruned finished jobs"
        );
        Ok(())
    }
}

/// Delete events published more than `CAMPBOOKS_EVENTS__RETENTION_DAYS`
/// ago, except one still waiting to be delivered (a pending or running job
/// reads it). Replay reaches back as far as the log does. Scheduled hourly
/// by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneEvents;

impl Job for PruneEvents {
    const NAME: &'static str = "kernel.prune_events";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let days = i32::try_from(ctx.state().config.events.retention_days).unwrap_or(i32::MAX);
        let settling = ctx.state().events.registry().settling_events();
        let mut pruned = 0_u64;
        loop {
            let deleted = sqlx::query!(
                r#"
                DELETE FROM kernel.events
                 WHERE id IN (
                     SELECT e.id FROM kernel.events e
                      WHERE e.occurred_at < now() - make_interval(days => $1)
                        AND NOT EXISTS (
                            SELECT 1 FROM kernel.jobs j
                             WHERE j.event_id = e.id
                               AND (j.state IN ('pending', 'running')
                                    OR (j.state = 'failed' AND e.name = ANY($3))))
                      LIMIT $2)
                "#,
                days,
                PRUNE_BATCH,
                &settling as &[&str],
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            pruned += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(pruned, retention_days = days, "pruned the event log");
        Ok(())
    }
}

/// Delete a purged workspace's finished jobs (succeeded or failed) and its
/// events, except events still waiting to be delivered. Unfinished jobs stay:
/// other modules' purges may still be queued. Returns how many jobs and
/// events were deleted.
pub async fn purge_workspace(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
) -> Result<(u64, u64), sqlx::Error> {
    let jobs = sqlx::query!(
        "DELETE FROM kernel.jobs
          WHERE workspace_id = $1 AND state IN ('succeeded', 'failed')",
        workspace
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let events = sqlx::query!(
        r#"
        DELETE FROM kernel.events e
         WHERE e.workspace_id = $1
           AND NOT EXISTS (
               SELECT 1 FROM kernel.jobs j
                WHERE j.event_id = e.id AND j.state IN ('pending', 'running'))
        "#,
        workspace
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok((jobs, events))
}
