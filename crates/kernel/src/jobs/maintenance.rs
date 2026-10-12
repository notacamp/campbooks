//! The kernel's own recurring maintenance.

use serde::{Deserialize, Serialize};

use super::{Job, JobContext, JobError};

/// Rows deleted per statement, so one prune never holds a long lock.
const PRUNE_BATCH: i64 = 5_000;

/// Delete succeeded jobs older than `CAMPBOOKS_JOBS__RETENTION_HOURS`.
/// Failed jobs stay for inspection; the event log is never pruned. Scheduled
/// hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneJobs;

impl Job for PruneJobs {
    const NAME: &'static str = "kernel.prune_jobs";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let hours = i32::try_from(ctx.state().config.jobs.retention_hours).unwrap_or(i32::MAX);
        let mut pruned = 0_u64;
        loop {
            let deleted = sqlx::query!(
                r#"
                DELETE FROM kernel.jobs
                 WHERE id IN (
                     SELECT id FROM kernel.jobs
                      WHERE state = 'succeeded'
                        AND finished_at < now() - make_interval(hours => $1)
                      LIMIT $2)
                "#,
                hours,
                PRUNE_BATCH,
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            pruned += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(pruned, retention_hours = hours, "pruned succeeded jobs");
        Ok(())
    }
}
