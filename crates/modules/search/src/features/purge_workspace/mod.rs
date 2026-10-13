//! Deleting a workspace's index when the workspace is deleted.
//!
//! `workspaces.workspace_deleting` hides the workspace at once (every request
//! to it answers 404); its data is kept until `purge_after`, when
//! `search.purge_workspace` deletes this module's rows. Search holds no
//! blobs and publishes no events, so nothing else of it remains.

use campbooks_kernel::{
    Event, EventHandler, Job, JobContext, JobError, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::events::WorkspaceDeleting;

/// Schedules the purge for the end of the grace period.
pub struct ScheduleWorkspacePurge;

impl EventHandler for ScheduleWorkspacePurge {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "search.schedule_workspace_purge";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<WorkspaceDeleting>) -> Result<(), JobError> {
        let deleting = event.payload;
        let options = EnqueueOptions::for_workspace(deleting.workspace.clone())
            .unique(format!("search.purge:{}", deleting.workspace))
            .run_at(deleting.purge_after);
        ctx.enqueue(&PurgeWorkspace, options).await?;
        Ok(())
    }
}

/// Deletes every row of the job's workspace in this module's schema.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PurgeWorkspace;

impl Job for PurgeWorkspace {
    const NAME: &'static str = "search.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("purge enqueued outside a workspace"))?;
        let tx = ctx.tx().await?;
        // Entries go with their groups.
        let groups = sqlx::query!(
            "DELETE FROM search.groups WHERE workspace_id = $1",
            workspace.as_uuid()
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        sqlx::query!(
            "DELETE FROM search.backfills WHERE workspace_id = $1",
            workspace.as_uuid()
        )
        .execute(&mut **tx)
        .await?;
        tracing::info!(groups, "search index purged for a deleted workspace");
        Ok(())
    }
}
