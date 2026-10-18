//! `campbooks-server workspaces purge-again <id> --confirm <id>`: purge a
//! workspace again after a database restore brought it back
//! (campbooks-cloud#65).
//!
//! A restored backup holds the workspaces as they were when it was taken:
//! one deleted (or deleted and purged) since is back, with its members and
//! its data, and the tombstones of the purges since are gone with the rest.
//! The operator deletes each such workspace again with this command (the
//! ids come from the `workspace deleted` and `workspace purged` log lines;
//! see `deploy/README.md`, "Backups").
//!
//! Operator only: the server binary calls it, no route ever does. A
//! workspace still deleted in the backup is purged now; one live in the
//! backup is deleted only when the operator says it was deleted after the
//! backup (`not_deleted_in_backup`), and purged an hour later at the
//! earliest, leaving time to undo a mistyped id through the restore.

use campbooks_kernel::{
    AppState,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use super::PurgeWorkspace;
use crate::events::WorkspaceDeleting;

/// The least time between deleting a workspace that was live in the backup
/// and purging it.
pub const LIVE_PURGE_NOTICE: TimeDelta = TimeDelta::hours(1);

/// What the operator says about the workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeAgainOptions {
    /// It is live in the restored database but was deleted after the
    /// backup was taken: delete it again. Without this, a live workspace is
    /// left as it is.
    pub not_deleted_in_backup: bool,
    /// For a live workspace deleted again: when to purge it (the
    /// `purge_after` of its `workspace deleted` line). At least
    /// [`LIVE_PURGE_NOTICE`] from now; that long from now by default.
    pub purge_after: Option<DateTime<Utc>>,
}

/// What [`purge_again`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeAgain {
    /// No row of that workspace: purged already, or never there. Nothing
    /// changed.
    NoSuchWorkspace,
    /// The workspace is live and the operator did not say it was deleted
    /// after the backup. Nothing changed; the facts to decide with.
    NotDeleted {
        /// Its members.
        members: i64,
        /// When it was created.
        created_at: DateTime<Utc>,
    },
    /// The purge time asked for is less than [`LIVE_PURGE_NOTICE`] away.
    /// Nothing changed.
    TooSoon {
        /// The earliest time accepted.
        earliest: DateTime<Utc>,
    },
    /// The workspace is live and has no owner to name in its deletion.
    /// Nothing changed.
    NoOwner,
    /// The live workspace is deleted (hidden at once), its deletion
    /// published so every module purges its data, and its purge queued for
    /// `purge_after`.
    Deleted {
        /// When it is purged.
        purge_after: DateTime<Utc>,
    },
    /// The deleted workspace is marked for purge now, and its purge queued
    /// to run now.
    Scheduled,
    /// The deleted workspace's purge is queued already: nothing changed (no
    /// second purge is enqueued); it runs at `run_at`.
    AlreadyQueued {
        /// When the queued purge runs.
        run_at: DateTime<Utc>,
    },
}

/// Delete `workspace` again after a restore, in one transaction working in
/// the workspace (see the module docs). A live workspace's deletion is
/// published naming its first owner as who deleted it: a stand-in for the
/// operator, since the event names a member. Idempotent: a purge already
/// queued means no second one. Logs one line with the workspace's id only
/// when it changes something.
///
/// # Errors
/// When the database fails.
pub async fn purge_again(
    state: &AppState,
    workspace: WorkspaceId,
    options: PurgeAgainOptions,
) -> Result<PurgeAgain, campbooks_kernel::Error> {
    let earliest = Utc::now() + LIVE_PURGE_NOTICE;
    if options.purge_after.is_some_and(|at| at < earliest) {
        return Ok(PurgeAgain::TooSoon { earliest });
    }
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<(Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT deleted_at, created_at FROM workspaces.workspaces WHERE id = $1 FOR UPDATE",
    )
    .bind(workspace.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;
    let Some((deleted_at, created_at)) = row else {
        return Ok(PurgeAgain::NoSuchWorkspace);
    };
    let key = workspace.to_string();
    let outcome = if deleted_at.is_some() {
        if let Some(run_at) = jobs::unfinished_run_at::<PurgeWorkspace>(&mut tx, &key, &key).await?
        {
            return Ok(PurgeAgain::AlreadyQueued { run_at });
        }
        sqlx::query(
            "UPDATE workspaces.workspaces SET purge_after = least(purge_after, now())
              WHERE id = $1",
        )
        .bind(workspace.as_uuid())
        .execute(&mut *tx)
        .await?;
        enqueue_purge(&mut tx, workspace, None).await?;
        PurgeAgain::Scheduled
    } else {
        match delete_live(state, &mut tx, workspace, created_at, options).await? {
            deleted @ PurgeAgain::Deleted { .. } => deleted,
            refused => return Ok(refused),
        }
    };
    tx.commit().await?;
    tracing::info!(workspace = %workspace, "workspace marked for purge again");
    Ok(outcome)
}

/// [`purge_again`] for a workspace live in the restored database: refused
/// unless the operator says it was deleted after the backup, else deleted
/// with its purge an hour away at the earliest.
async fn delete_live(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: WorkspaceId,
    created_at: DateTime<Utc>,
    options: PurgeAgainOptions,
) -> Result<PurgeAgain, campbooks_kernel::Error> {
    if !options.not_deleted_in_backup {
        let members: i64 =
            sqlx::query_scalar("SELECT count(*) FROM workspaces.members WHERE workspace_id = $1")
                .bind(workspace.as_uuid())
                .fetch_one(&mut **tx)
                .await?;
        return Ok(PurgeAgain::NotDeleted {
            members,
            created_at,
        });
    }
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM workspaces.members
          WHERE workspace_id = $1 AND role = 'owner'
          ORDER BY joined_at, user_id LIMIT 1",
    )
    .bind(workspace.as_uuid())
    .fetch_optional(&mut **tx)
    .await?;
    let Some(owner) = owner else {
        return Ok(PurgeAgain::NoOwner);
    };
    let purge_after = options
        .purge_after
        .unwrap_or_else(|| Utc::now() + LIVE_PURGE_NOTICE);
    sqlx::query(
        "UPDATE workspaces.workspaces SET deleted_at = now(), purge_after = $2 WHERE id = $1",
    )
    .bind(workspace.as_uuid())
    .bind(purge_after)
    .execute(&mut **tx)
    .await?;
    state
        .events
        .publish(
            tx,
            &WorkspaceDeleting {
                workspace: workspace.to_string(),
                purge_after,
                deleted_by: UserId::from_uuid(owner),
            },
        )
        .await?;
    enqueue_purge(tx, workspace, Some(purge_after)).await?;
    Ok(PurgeAgain::Deleted { purge_after })
}

/// Queue the workspace's purge under its unique key (no second one).
async fn enqueue_purge(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: WorkspaceId,
    run_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    let mut options =
        EnqueueOptions::for_workspace(workspace.to_string()).unique(workspace.to_string());
    if let Some(run_at) = run_at {
        options = options.run_at(run_at);
    }
    jobs::enqueue(tx, &PurgeWorkspace { workspace }, options).await?;
    Ok(())
}
