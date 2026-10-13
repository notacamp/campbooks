//! Numbering a workspace's changes, and pruning old ones.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use super::{NOTIFY_CHANNEL, QUEUE, lock_key};
use crate::jobs::{EnqueueOptions, Job, JobContext, JobError, enqueue};

/// Longest the sequencer waits for a workspace's producers in flight before
/// it gives up and retries. While it waits, new producers of that workspace
/// wait behind it, so this also bounds their stall.
const LOCK_TIMEOUT: &str = "2s";

/// Delay before a sequencer that timed out on the lock runs again.
const LOCK_RETRY: Duration = Duration::from_millis(250);

/// Unnumbered changes older than this are re-sequenced by
/// [`PruneChanges`] (their sequencer ran out of attempts).
const STRANDED_AFTER_SECONDS: f64 = 60.0;

/// Number every unnumbered change of `workspace`: enqueued (once per
/// workspace while one is unfinished) by every publish that declares
/// changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceChanges {
    /// The workspace whose changes to number.
    pub workspace: String,
}

impl Job for SequenceChanges {
    const NAME: &'static str = "kernel.sequence_changes";
    const QUEUE: &'static str = QUEUE;
    /// Lock timeouts count as attempts; a workspace whose producers keep a
    /// transaction open that long is rare, and the hourly sweep picks up
    /// whatever is left.
    const MAX_ATTEMPTS: u32 = 50;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let tx = ctx.tx().await?;
        sqlx::query_scalar!(
            "SELECT set_config('lock_timeout', $1, true)",
            LOCK_TIMEOUT
        )
        .fetch_one(&mut **tx)
        .await?;
        // Exclusive: waits until every producer holding the shared lock (one
        // that may have found this job already pending or running) has
        // committed, so all of their changes are visible below.
        let locked = sqlx::query_scalar!(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            lock_key(&self.workspace)
        )
        .fetch_one(&mut **tx)
        .await;
        if let Err(err) = locked {
            let timed_out = err
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .is_some_and(|code| code == "55P03");
            return Err(if timed_out {
                JobError::retry_after("producers of the workspace are still committing", LOCK_RETRY)
            } else {
                err.into()
            });
        }
        let numbered = sequence(tx, &self.workspace).await?;
        if numbered > 0 {
            sqlx::query_scalar!(
                "SELECT pg_notify($1, $2)",
                NOTIFY_CHANNEL,
                self.workspace
            )
            .fetch_one(&mut **tx)
            .await?;
        }
        tracing::debug!(numbered, "sequenced changes");
        Ok(())
    }
}

/// Number every unnumbered change of `workspace` from its head, in
/// insertion order. Returns how many were numbered. Callers hold the
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
            UPDATE kernel.changes c SET seq = n.seq
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

/// Delete changes older than `CAMPBOOKS_CHANGES__RETENTION_HOURS` (clients
/// resuming from before them are told to `reset`), and re-enqueue
/// sequencing for workspaces left with unnumbered changes. Runs hourly.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneChanges;

impl Job for PruneChanges {
    const NAME: &'static str = "kernel.prune_changes";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let hours = i64::try_from(ctx.state().config.changes.retention_hours).unwrap_or(i64::MAX);
        let tx = ctx.tx().await?;
        let pruned = sqlx::query!(
            "DELETE FROM kernel.changes
              WHERE seq IS NOT NULL AND occurred_at < now() - make_interval(hours => $1::int)",
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
            enqueue(
                tx,
                &SequenceChanges {
                    workspace: workspace.clone(),
                },
                EnqueueOptions::for_workspace(workspace.clone()).unique(workspace.clone()),
            )
            .await?;
        }
        tracing::info!(
            pruned,
            resequenced = stranded.len(),
            "pruned old changes of the change feed"
        );
        Ok(())
    }
}
