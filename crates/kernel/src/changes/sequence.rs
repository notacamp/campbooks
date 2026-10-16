//! Numbering a workspace's changes, and pruning old ones.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use super::{NOTIFY_CHANNEL, QUEUE, lock_key};
use crate::jobs::{Job, JobContext, JobError};

/// A sequencer that finds producers of its workspace in flight runs again
/// after a delay between these, chosen at random so retries spread out.
const BUSY_RETRY_MS: (u64, u64) = (50, 250);

/// Unnumbered changes older than this are re-sequenced by
/// [`PruneChanges`] (their sequencer ran out of attempts).
const STRANDED_AFTER_SECONDS: f64 = 60.0;

/// Number every unnumbered change of `workspace`: enqueued by publishes
/// that declare changes, unless one is already pending for the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceChanges {
    /// The workspace whose changes to number.
    pub workspace: String,
}

impl Job for SequenceChanges {
    const NAME: &'static str = "kernel.sequence_changes";
    const QUEUE: &'static str = QUEUE;
    /// Attempts are spent on database failures only: finding producers in
    /// flight is not a failure (see `run`).
    const MAX_ATTEMPTS: u32 = 10;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let tx = ctx.tx().await?;
        // Exclusive, and never waited for: a waiting exclusive request would
        // queue every new producer's shared request behind it. Holding it
        // means no producer of the workspace is in flight, so every change
        // that will ever sit below the new cursors is visible below.
        let locked = sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0)) AS "locked!""#,
            lock_key(&self.workspace)
        )
        .fetch_one(&mut **tx)
        .await?;
        if !locked {
            // Producers are committing: come back shortly. This run succeeds
            // (it is not a failure) and leaves a pending successor, which the
            // producers in flight also see and skip enqueueing their own.
            let delay = BUSY_RETRY_MS.0
                + u64::from(uuid::Uuid::now_v7().as_bytes()[15])
                    * (BUSY_RETRY_MS.1 - BUSY_RETRY_MS.0)
                    / 255;
            super::request_sequencing(tx, &self.workspace, Some(Duration::from_millis(delay)))
                .await?;
            tracing::debug!(delay_ms = delay, "producers in flight; sequencing later");
            return Ok(());
        }
        let numbered = sequence(tx, &self.workspace).await?;
        if numbered > 0 {
            sqlx::query_scalar!("SELECT pg_notify($1, $2)", NOTIFY_CHANNEL, self.workspace)
                .fetch_one(&mut **tx)
                .await?;
        }
        tracing::debug!(numbered, "sequenced changes");
        Ok(())
    }
}

/// Number every unnumbered change of `workspace` from its head, in
/// insertion order, stamping when (`sequenced_at`: the statement's start,
/// so a later run, which can only start once this one committed, stamps a
/// later time). Returns how many were numbered. Callers hold the
/// workspace's exclusive lock.
pub(super) async fn sequence(conn: &mut PgConnection, workspace: &str) -> Result<i64, sqlx::Error> {
    sqlx::query!(
        "INSERT INTO kernel.change_heads (workspace_id, head) VALUES ($1, 0)
         ON CONFLICT (workspace_id) DO NOTHING",
        workspace
    )
    .execute(&mut *conn)
    .await?;
    let numbered = sqlx::query_scalar!(
        r#"
        WITH head AS (
            SELECT head FROM kernel.change_heads WHERE workspace_id = $1 FOR UPDATE
        ), numbered AS (
            SELECT c.id, head.head + row_number() OVER (ORDER BY c.published) AS seq
              FROM kernel.changes c, head
             WHERE c.workspace_id = $1 AND c.seq IS NULL
        ), updated AS (
            UPDATE kernel.changes c SET seq = n.seq, sequenced_at = statement_timestamp()
              FROM numbered n
             WHERE c.id = n.id
            RETURNING c.seq
        )
        UPDATE kernel.change_heads h
           SET head = coalesce((SELECT max(seq) FROM updated), h.head), updated_at = now()
         WHERE h.workspace_id = $1
        RETURNING (SELECT count(*) FROM updated) AS "numbered!"
        "#,
        workspace
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(numbered)
}

/// Delete changes numbered longer ago than
/// `CAMPBOOKS_CHANGES__RETENTION_HOURS` (clients resuming from before them
/// are told to `reset`; by numbering time, so a resume token's age decides
/// that alone, see `stream`), and re-enqueue
/// sequencing for workspaces left with unnumbered changes. Runs hourly.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneChanges;

impl Job for PruneChanges {
    const NAME: &'static str = "kernel.prune_changes";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let hours = i64::try_from(ctx.state().config.changes.retention_hours).unwrap_or(i64::MAX);
        // Every workspace's feed.
        let tx = ctx.kernel_tx().await?;
        let pruned = sqlx::query!(
            "DELETE FROM kernel.changes
              WHERE seq IS NOT NULL AND sequenced_at < now() - make_interval(hours => $1::int)",
            i32::try_from(hours).unwrap_or(i32::MAX)
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        let stranded = sqlx::query_scalar!(
            "SELECT DISTINCT workspace_id FROM kernel.changes
              WHERE seq IS NULL AND occurred_at < now() - make_interval(secs => $1)",
            STRANDED_AFTER_SECONDS
        )
        .fetch_all(&mut **tx)
        .await?;
        for workspace in &stranded {
            super::request_sequencing(tx, workspace, None).await?;
        }
        tracing::info!(
            pruned,
            resequenced = stranded.len(),
            "pruned old changes of the change feed"
        );
        Ok(())
    }
}
