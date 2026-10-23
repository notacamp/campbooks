//! Deleting a workspace's profiles and chats when the workspace is deleted,
//! and a former member's profile and chats when they leave (rejoining
//! starts afresh).

use campbooks_kernel::{
    Event, EventHandler, Job, JobContext, JobError, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::events::{MemberLeft, WorkspaceDeleting};

/// Schedules the purge for the end of the grace period.
pub struct ScheduleWorkspacePurge;

impl EventHandler for ScheduleWorkspacePurge {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "assistant.schedule_workspace_purge";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<WorkspaceDeleting>) -> Result<(), JobError> {
        let deleting = event.payload;
        let options = EnqueueOptions::for_workspace(deleting.workspace.clone())
            .unique(format!("assistant.purge:{}", deleting.workspace))
            .run_at(deleting.purge_after);
        ctx.enqueue(&PurgeWorkspace, options).await?;
        Ok(())
    }
}

/// Deletes every row of the job's workspace in this module's schema.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PurgeWorkspace;

impl Job for PurgeWorkspace {
    const NAME: &'static str = "assistant.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("purge enqueued outside a workspace"))?;
        let tx = ctx.tx().await?;
        let deleted = sqlx::query!(
            "DELETE FROM assistant.profiles WHERE workspace_id = $1",
            workspace.as_uuid()
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        sqlx::query("DELETE FROM assistant.chat_messages WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .execute(&mut **tx)
            .await?;
        let chats = sqlx::query("DELETE FROM assistant.chats WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .execute(&mut **tx)
            .await?
            .rows_affected();
        tracing::info!(
            profiles = deleted,
            chats,
            "assistant purged for a deleted workspace"
        );
        Ok(())
    }
}

/// Forgets a former member's profile and their chats.
pub struct ForgetMember;

impl EventHandler for ForgetMember {
    type Event = MemberLeft;
    const NAME: &'static str = "assistant.forget_member";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let left = event.payload;
        let workspace: WorkspaceId = left
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left names an invalid workspace"))?;
        let tx = ctx.tx().await?;
        sqlx::query!(
            "DELETE FROM assistant.profiles WHERE workspace_id = $1 AND user_id = $2",
            workspace.as_uuid(),
            left.user.as_uuid(),
        )
        .execute(&mut **tx)
        .await?;
        // Their chats with Scout, and the chats' messages.
        sqlx::query(
            "DELETE FROM assistant.chat_messages WHERE chat_id IN (
                 SELECT id FROM assistant.chats WHERE workspace_id = $1 AND user_id = $2)",
        )
        .bind(workspace.as_uuid())
        .bind(left.user.as_uuid())
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM assistant.chats WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace.as_uuid())
            .bind(left.user.as_uuid())
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}
