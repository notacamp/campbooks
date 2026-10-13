//! Deleting a workspace's index when the workspace is deleted.
//!
//! `workspaces.workspace_deleting` hides the workspace at once (every request
//! to it answers 404); its data is kept until `purge_after`, when
//! `search.purge_workspace` deletes this module's index. From the event on,
//! each source's backfill row says `deleting`, so nothing more is indexed for
//! the workspace; those rows (two ids each, no content) stay after the purge
//! so a late delivery cannot start indexing it again. Search holds no blobs
//! and publishes no events.

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
        let workspace: WorkspaceId = deleting
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("event with a malformed workspace"))?;
        let group_types: Vec<String> = ctx
            .state()
            .search_sources
            .iter()
            .map(|source| source.group_type().to_owned())
            .collect();
        let tx = ctx.tx().await?;
        sqlx::query!(
            r#"INSERT INTO search.backfills (workspace_id, group_type, state, index_version)
               SELECT $1, group_type, 'deleting', $3 FROM unnest($2::text[]) AS group_type
               ON CONFLICT (workspace_id, group_type) DO UPDATE SET state = 'deleting'"#,
            workspace.as_uuid(),
            &group_types,
            crate::domain::index::INDEX_VERSION,
        )
        .execute(&mut **tx)
        .await?;
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
        tracing::info!(groups, "search index purged for a deleted workspace");
        Ok(())
    }
}
