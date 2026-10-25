//! Deleting a workspace's pages when the workspace is deleted.

use campbooks_kernel::{
    Event, EventHandler, Job, JobContext, JobError, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::events::WorkspaceDeleting;

/// Schedules the purge for the end of the grace period.
pub struct ScheduleWorkspacePurge;

impl EventHandler for ScheduleWorkspacePurge {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "pages.schedule_workspace_purge";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<WorkspaceDeleting>) -> Result<(), JobError> {
        let deleting = event.payload;
        let options = EnqueueOptions::for_workspace(deleting.workspace.clone())
            .unique(format!("pages.purge:{}", deleting.workspace))
            .run_at(deleting.purge_after);
        ctx.enqueue(&PurgeWorkspace, options).await?;
        Ok(())
    }
}

/// Deletes every row of the job's workspace in this module's schema, and
/// lets go of the blobs its pages held.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PurgeWorkspace;

impl Job for PurgeWorkspace {
    const NAME: &'static str = "pages.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("purge enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let ids: Vec<sqlx::types::Uuid> =
            sqlx::query_scalar("SELECT id FROM pages.pages WHERE workspace_id = $1")
                .bind(workspace.as_uuid())
                .fetch_all(&mut **tx)
                .await?;
        // The blob store as a whole goes with the workspace; the references
        // are let go all the same, so nothing points at a page that is gone.
        state
            .blobs
            .detach_owners(tx, &workspace.to_string(), "pages.page", &ids)
            .await?;
        // Updates, snapshots, grants, links and files go with their pages.
        let deleted = sqlx::query("DELETE FROM pages.pages WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .execute(&mut **tx)
            .await?
            .rows_affected();
        tracing::info!(pages = deleted, "pages purged for a deleted workspace");
        Ok(())
    }
}

#[cfg(test)]
mod tests;
