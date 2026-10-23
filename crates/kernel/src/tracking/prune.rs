//! `kernel.prune_tracked_events`: the outbox keeps published rows for
//! `CAMPBOOKS_TRACKING__PUBLISHED_RETENTION_HOURS`, and unpublished rows
//! until the relay publishes them, however long the broker is away.

use serde::{Deserialize, Serialize};

use crate::{Job, JobContext, JobError};

/// Rows deleted per statement, so one prune never holds a long lock.
const PRUNE_BATCH: i32 = 5_000;

/// Delete tracked events published more than
/// `CAMPBOOKS_TRACKING__PUBLISHED_RETENTION_HOURS` ago
/// (`kernel.prune_tracked_events`). Never an unpublished one. Scheduled
/// hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneTrackedEvents;

impl Job for PruneTrackedEvents {
    const NAME: &'static str = "kernel.prune_tracked_events";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let hours = i32::try_from(ctx.state().config.tracking.published_retention_hours)
            .unwrap_or(i32::MAX);
        let mut pruned = 0_i64;
        loop {
            // Every workspace's rows; one transaction per batch.
            let mut tx = crate::db::begin_kernel(ctx.pool()).await?;
            let deleted: i64 = sqlx::query_scalar("SELECT kernel.prune_tracked_events($1, $2)")
                .bind(hours)
                .bind(PRUNE_BATCH)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            pruned += deleted;
            if deleted < i64::from(PRUNE_BATCH) {
                break;
            }
        }
        tracing::info!(
            pruned,
            retention_hours = hours,
            "pruned published tracked events"
        );
        Ok(())
    }
}
