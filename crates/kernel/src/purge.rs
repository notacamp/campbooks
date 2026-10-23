//! What is left of a purged workspace in the kernel's tables, and its
//! tombstone (campbooks-cloud#65).
//!
//! The workspaces module's purge job deletes a workspace's kernel rows (its
//! change feed, its finished jobs, its events, its tracked events) in short
//! batches
//! ([`purge_workspace`]), then deletes the workspace's row and records its
//! [`tombstone`] in one transaction. Rows of the workspace that appear
//! afterwards (another module's purge running later publishes an event, its
//! job finishes) are deleted by the hourly [`SweepPurgedWorkspaces`],
//! whatever their age, except a delivery still pending or running.
//!
//! Tombstones live in the same database: a restored backup brings back the
//! tombstones of its moment, not the ones written since. Re-deleting the
//! workspaces purged after the backup is the operator's step
//! (`campbooks-server workspaces purge-again`).

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{Job, JobContext, JobError, tenancy::WorkspaceId};

#[cfg(test)]
mod tests;

/// Rows deleted per table and per transaction, so a large workspace never
/// holds one long lock or one statement past its timeout.
pub const PURGE_BATCH: i64 = 5_000;

/// How long a tombstone stays once nothing of its workspace is left.
const TOMBSTONE_KEPT_DAYS: i32 = 30;

/// What [`purge_workspace`] deleted, and in how many transactions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purged {
    /// Finished jobs (succeeded or failed).
    pub jobs: u64,
    /// Events no pending or running delivery reads.
    pub events: u64,
    /// Changes of the change feed.
    pub changes: u64,
    /// Tracked events in the outbox, published or not (ADR 0020).
    pub tracked: u64,
    /// Idempotency keys first used on the workspace's routes, expired or not.
    pub keys: u64,
    /// Transactions it took for the change feed, the jobs and the events,
    /// [`PURGE_BATCH`] rows per table each (the keys take their own).
    pub batches: u32,
}

/// Delete `workspace`'s change feed, finished jobs, events no pending or
/// running delivery reads and tracked events (through
/// `kernel.purge_workspace_tracked_events`: the runtime role may only append
/// to the outbox), [`PURGE_BATCH`] rows per table at a time, each
/// batch in a short transaction of its own working in `workspace` (row-level
/// security applies), then its idempotency keys, expired or not, in short
/// transactions of their own there too
/// ([`crate::idempotency::purge_workspace`]). Unfinished jobs stay: another
/// module's purge may still be queued. Idempotent: a second run deletes what
/// appeared since.
pub async fn purge_workspace(pool: &PgPool, workspace: &str) -> Result<Purged, sqlx::Error> {
    let mut purged = Purged::default();
    loop {
        let mut tx = crate::db::begin_in(pool, workspace).await?;
        let changes = crate::changes::purge_workspace(&mut tx, workspace, PURGE_BATCH).await?;
        let (jobs, events) = crate::jobs::purge_workspace(&mut tx, workspace, PURGE_BATCH).await?;
        let tracked: i64 =
            sqlx::query_scalar("SELECT kernel.purge_workspace_tracked_events($1, $2)")
                .bind(workspace)
                .bind(i32::try_from(PURGE_BATCH).unwrap_or(i32::MAX))
                .fetch_one(&mut *tx)
                .await?;
        let tracked = tracked.unsigned_abs();
        tx.commit().await?;
        purged.batches += 1;
        purged.changes += changes;
        purged.jobs += jobs;
        purged.events += events;
        purged.tracked += tracked;
        let full = PURGE_BATCH.unsigned_abs();
        if changes < full && jobs < full && events < full && tracked < full {
            break;
        }
    }
    purged.keys = crate::idempotency::purge_workspace(pool, workspace).await?;
    Ok(purged)
}

/// Record that `workspace` was purged, in `tx`: the transaction that deletes
/// its row, working in `workspace`. Holds the id and the time only.
/// Refused in a transaction that cannot reach the workspace's rows.
pub async fn tombstone(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
) -> Result<(), sqlx::Error> {
    crate::db::require_scope(tx, workspace, "a purged workspace's tombstone").await?;
    sqlx::query!(
        "INSERT INTO kernel.purged_workspaces (workspace_id) VALUES ($1)
         ON CONFLICT (workspace_id) DO UPDATE SET purged_at = now()",
        workspace
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Delete what reached the kernel's tables of a purged workspace after its
/// purge, for every tombstone ([`purge_workspace`]: changes, finished jobs,
/// events no pending or running delivery reads, tracked events), then drop the tombstones
/// older than 30 days whose workspace has nothing left. A workspace that is
/// live again is skipped with a warning, its tombstone kept; a workspace
/// whose purge fails is logged and tried again by the next sweep, and the
/// others go on. Scheduled hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SweepPurgedWorkspaces;

impl Job for SweepPurgedWorkspaces {
    const NAME: &'static str = "kernel.sweep_purged_workspaces";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let pool = ctx.pool().clone();
        // The tombstones, across workspaces; each workspace's rows are then
        // deleted in that workspace.
        let mut tx = crate::db::begin_kernel(&pool).await?;
        let tombstones = sqlx::query_scalar!(
            "SELECT workspace_id FROM kernel.purged_workspaces ORDER BY purged_at"
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut swept = Purged::default();
        // Tombstones left as they are this time: a live workspace, or one
        // whose purge failed (the next sweep tries again).
        let mut kept: Vec<String> = Vec::new();
        for workspace in &tombstones {
            match Self::sweep_one(ctx, &pool, workspace).await {
                Ok(Some(purged)) => {
                    swept.jobs += purged.jobs;
                    swept.events += purged.events;
                    swept.changes += purged.changes;
                    swept.tracked += purged.tracked;
                    swept.keys += purged.keys;
                }
                Ok(None) => kept.push(workspace.clone()),
                Err(error) => {
                    tracing::warn!(
                        workspace = %workspace,
                        %error,
                        "could not sweep a purged workspace; the next sweep tries again"
                    );
                    kept.push(workspace.clone());
                }
            }
        }
        let mut tx = crate::db::begin_kernel(&pool).await?;
        let dropped = sqlx::query!(
            r#"
            DELETE FROM kernel.purged_workspaces p
             WHERE p.purged_at < now() - make_interval(days => $1)
               AND p.workspace_id <> ALL($2)
               AND NOT EXISTS (SELECT 1 FROM kernel.jobs j WHERE j.workspace_id = p.workspace_id)
               AND NOT EXISTS (SELECT 1 FROM kernel.events e WHERE e.workspace_id = p.workspace_id)
               AND NOT EXISTS (SELECT 1 FROM kernel.changes c WHERE c.workspace_id = p.workspace_id)
               AND NOT EXISTS (
                   SELECT 1 FROM kernel.change_heads h WHERE h.workspace_id = p.workspace_id)
            "#,
            TOMBSTONE_KEPT_DAYS,
            &kept as &[String],
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        tracing::info!(
            tombstones = tombstones.len(),
            jobs = swept.jobs,
            events = swept.events,
            changes = swept.changes,
            tracked = swept.tracked,
            keys = swept.keys,
            kept = kept.len(),
            dropped,
            "swept what was left of purged workspaces"
        );
        Ok(())
    }
}

impl SweepPurgedWorkspaces {
    /// Purge what is left of tombstoned `workspace`, unless the workspace is
    /// live again (its row is back and not deleted: a restore, or a mistake):
    /// then nothing is deleted, its tombstone stays, and one warning names
    /// it (`None`). An id that is not a workspace id cannot be live.
    async fn sweep_one(
        ctx: &JobContext,
        pool: &PgPool,
        workspace: &str,
    ) -> Result<Option<Purged>, crate::Error> {
        if let Ok(id) = workspace.parse::<WorkspaceId>() {
            let state = ctx.state();
            if state.membership.workspace_live(pool, id).await? {
                tracing::warn!(
                    workspace = %workspace,
                    "a tombstoned workspace is live; its rows are not swept"
                );
                return Ok(None);
            }
        }
        Ok(Some(purge_workspace(pool, workspace).await?))
    }
}
