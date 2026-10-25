//! Trash is kept 30 days (`pages.dispatch_purge_trash`, `pages.purge_trash`).
//!
//! - `pages.dispatch_purge_trash`, every day, reads which workspaces hold a
//!   trash root older than 30 days (the maintenance scope: ids and times,
//!   read only, through the table's maintenance policy) and enqueues
//!   `pages.purge_trash` in each, one pending job per workspace. It reads
//!   nothing else and deletes nothing.
//! - `pages.purge_trash`, in the workspace (row-level security applies),
//!   deletes those pages for good, with what was deleted with them.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::domain::{PageRow, SELECT_ONE, purge};

/// How often trash to purge is looked for.
pub const EVERY: Duration = Duration::from_hours(24);
/// Days a page is kept in the trash.
pub const KEEP_DAYS: i32 = 30;
/// Workspaces dispatched per tick; the next tick takes the rest.
const DISPATCH_BATCH: i64 = 500;
/// Pages purged per job; the next day takes the rest.
const PURGE_BATCH: i64 = 200;

/// The dispatcher tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchPurgeTrash;

impl Job for DispatchPurgeTrash {
    const NAME: &'static str = "pages.dispatch_purge_trash";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids and
        // times, read only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM pages.pages
              WHERE trash_root = id AND trashed_at < now() - make_interval(days => $1)
              ORDER BY workspace_id LIMIT $2",
        )
        .bind(KEEP_DAYS)
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_in(
                tx,
                &workspace,
                &PurgeTrash,
                EnqueueOptions::for_workspace(workspace.clone()).unique("pages.purge_trash"),
            )
            .await?;
        }
        Ok(())
    }
}

/// Deletes, in its workspace, the pages that have been in the trash more
/// than 30 days, with what was deleted with them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeTrash;

impl Job for PurgeTrash {
    const NAME: &'static str = "pages.purge_trash";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let Some(workspace) = ctx
            .workspace()
            .and_then(|workspace| workspace.parse::<WorkspaceId>().ok())
        else {
            return Err(JobError::fatal("purge enqueued outside a workspace"));
        };
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let roots: Vec<crate::domain::PageId> = sqlx::query_scalar(
            "SELECT id FROM pages.pages
              WHERE trash_root = id AND trashed_at < now() - make_interval(days => $1)
              ORDER BY trashed_at LIMIT $2",
        )
        .bind(KEEP_DAYS)
        .bind(PURGE_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        let mut purged = 0_u64;
        for root in roots {
            let row: Option<PageRow> = sqlx::query_as(SELECT_ONE)
                .bind(root)
                .fetch_optional(&mut **tx)
                .await?;
            if let Some(row) = row {
                purge::root(&state, tx, workspace, &row).await?;
                purged += 1;
            }
        }
        tracing::info!(roots = purged, "pages purged from the trash");
        Ok(())
    }
}

#[cfg(test)]
mod tests;
