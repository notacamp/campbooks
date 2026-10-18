//! `campbooks-server workspaces purge-again <id> --confirm <id>`: purge a
//! workspace again after a database restore brought it back
//! (campbooks-cloud#65).
//!
//! A restored backup holds the workspaces as they were when it was taken:
//! one deleted and purged since is back, with its members and its data, and
//! the tombstones of the purges since are gone with the rest. The operator
//! re-deletes each such workspace with this command (the ids come from the
//! `workspace purged` log lines; see `deploy/README.md`, "Backups").
//!
//! Operator only: the server binary calls it, no route ever does. It is the
//! only path that sets `purge_after` to now; the app's deletion always waits
//! out the grace period.

use campbooks_kernel::{
    AppState,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions, Enqueued},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::PurgeWorkspace;
use crate::events::WorkspaceDeleting;

/// What [`purge_again`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeAgain {
    /// No row of that workspace: purged already, or never there. Nothing
    /// changed.
    NoSuchWorkspace,
    /// The workspace was not deleted and has no owner to name as the one
    /// who deleted it. Nothing changed.
    NoOwner,
    /// Marked for purge now, and a purge enqueued to run now.
    Scheduled {
        /// It was not deleted (the backup predates the deletion): deleted
        /// now too, and its deletion published, so every module purges its
        /// data again.
        newly_deleted: bool,
    },
    /// Marked for purge now; a purge of it was queued already (no second
    /// one is enqueued) and runs when its own time comes.
    AlreadyQueued {
        /// As for [`PurgeAgain::Scheduled`].
        newly_deleted: bool,
    },
}

/// Mark `workspace` for purge now and enqueue its purge, in one transaction
/// working in the workspace. A workspace that was not deleted is deleted
/// now, its deletion published with the first owner as who deleted it (an
/// owner did: only owners delete). Idempotent: a purge already queued means
/// no second one. Logs one line with the workspace's id only.
///
/// # Errors
/// When the database fails.
pub async fn purge_again(
    state: &AppState,
    workspace: WorkspaceId,
) -> Result<PurgeAgain, campbooks_kernel::Error> {
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<(Option<DateTime<Utc>>,)> =
        sqlx::query_as("SELECT deleted_at FROM workspaces.workspaces WHERE id = $1 FOR UPDATE")
            .bind(workspace.as_uuid())
            .fetch_optional(&mut *tx)
            .await?;
    let Some((deleted_at,)) = row else {
        return Ok(PurgeAgain::NoSuchWorkspace);
    };
    let newly_deleted = deleted_at.is_none();
    let owner: Option<Uuid> = if newly_deleted {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM workspaces.members
              WHERE workspace_id = $1 AND role = 'owner'
              ORDER BY joined_at, user_id LIMIT 1",
        )
        .bind(workspace.as_uuid())
        .fetch_optional(&mut *tx)
        .await?;
        if owner.is_none() {
            return Ok(PurgeAgain::NoOwner);
        }
        owner
    } else {
        None
    };
    let purge_after: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE workspaces.workspaces
            SET deleted_at = coalesce(deleted_at, now()), purge_after = now()
          WHERE id = $1
        RETURNING purge_after",
    )
    .bind(workspace.as_uuid())
    .fetch_one(&mut *tx)
    .await?;
    if let Some(owner) = owner {
        state
            .events
            .publish(
                &mut tx,
                &WorkspaceDeleting {
                    workspace: workspace.to_string(),
                    purge_after,
                    deleted_by: UserId::from_uuid(owner),
                },
            )
            .await?;
    }
    let enqueued = jobs::enqueue(
        &mut tx,
        &PurgeWorkspace { workspace },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(workspace.to_string())
            .run_at(purge_after),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(workspace = %workspace, "workspace marked for purge again");
    Ok(match enqueued {
        Enqueued::Created(_) => PurgeAgain::Scheduled { newly_deleted },
        Enqueued::Duplicate => PurgeAgain::AlreadyQueued { newly_deleted },
    })
}
