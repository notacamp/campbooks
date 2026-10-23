//! Forgetting: a member who leaves loses their calendars, events, removed
//! items, settings and requests; a removed connection takes its calendars
//! with it (the member's Campbooks calendar stays); a deleted workspace's
//! rows go when its grace period ends.

use campbooks_kernel::{
    Event, EventHandler, Job, JobContext, JobError, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{
    domain::outbound::BLOB_OWNER,
    inbound::{ConnectionRemoved, MemberLeft, WorkspaceDeleting},
};

/// `workspaces.member_left` → delete what the member had.
pub struct ForgetMember;

impl EventHandler for ForgetMember {
    type Event = MemberLeft;
    const NAME: &'static str = "schedule.forget_member";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let left = event.payload;
        // Only what existed when they left: a member who came back since
        // keeps what they were given after.
        let before = event.occurred_at;
        let workspace: WorkspaceId = left
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left names an invalid workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let requests: Vec<Uuid> = sqlx::query_scalar(
            "DELETE FROM schedule.change_requests
              WHERE workspace_id = $1 AND member_id = $2 AND created_at <= $3
          RETURNING id",
        )
        .bind(workspace)
        .bind(left.user)
        .bind(before)
        .fetch_all(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owners(tx, &left.workspace, BLOB_OWNER, &requests)
            .await?;
        // The events go with their calendars.
        let calendars = sqlx::query(
            "DELETE FROM schedule.calendars
              WHERE workspace_id = $1 AND member_id = $2 AND created_at <= $3",
        )
        .bind(workspace)
        .bind(left.user)
        .bind(before)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        sqlx::query(
            "DELETE FROM schedule.removed_items
              WHERE workspace_id = $1 AND member_id = $2 AND removed_at <= $3",
        )
        .bind(workspace)
        .bind(left.user)
        .bind(before)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "DELETE FROM schedule.member_settings
              WHERE workspace_id = $1 AND member_id = $2 AND updated_at <= $3",
        )
        .bind(workspace)
        .bind(left.user)
        .bind(before)
        .execute(&mut **tx)
        .await?;
        tracing::info!(
            calendars,
            "a departed member's calendars and settings removed"
        );
        Ok(())
    }
}

/// `connectors.connection_removed` → delete the connection's calendars.
pub struct ForgetConnection;

impl EventHandler for ForgetConnection {
    type Event = ConnectionRemoved;
    const NAME: &'static str = "schedule.forget_connection";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<ConnectionRemoved>) -> Result<(), JobError> {
        let removed = event.payload;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        // Campbooks events that were added to these calendars are theirs
        // again: their copy goes with the calendar.
        sqlx::query(
            "UPDATE schedule.events SET pushed_to = NULL, updated_at = now()
              WHERE pushed_to IN (
                    SELECT e.id FROM schedule.events e
                      JOIN schedule.calendars c ON c.id = e.calendar_id
                     WHERE c.connection_id = $1)",
        )
        .bind(removed.connection)
        .execute(&mut **tx)
        .await?;
        let requests: Vec<Uuid> = sqlx::query_scalar(
            "DELETE FROM schedule.change_requests WHERE connection_id = $1 RETURNING id",
        )
        .bind(removed.connection)
        .fetch_all(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owners(tx, &removed.workspace, BLOB_OWNER, &requests)
            .await?;
        let calendars = sqlx::query("DELETE FROM schedule.calendars WHERE connection_id = $1")
            .bind(removed.connection)
            .execute(&mut **tx)
            .await?
            .rows_affected();
        tracing::info!(calendars, "the calendars of a removed connection deleted");
        Ok(())
    }
}

/// `workspaces.workspace_deleting` → schedule the purge for the end of the
/// grace period.
pub struct ScheduleWorkspacePurge;

impl EventHandler for ScheduleWorkspacePurge {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "schedule.schedule_workspace_purge";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<WorkspaceDeleting>) -> Result<(), JobError> {
        let deleting = event.payload;
        let options = EnqueueOptions::for_workspace(deleting.workspace.clone())
            .unique(format!("schedule.purge:{}", deleting.workspace))
            .run_at(deleting.purge_after);
        ctx.enqueue(&PurgeWorkspace, options).await?;
        Ok(())
    }
}

/// Deletes every row of the job's workspace in this module's schema.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PurgeWorkspace;

impl Job for PurgeWorkspace {
    const NAME: &'static str = "schedule.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("purge enqueued outside a workspace"))?;
        let tx = ctx.tx().await?;
        for table in [
            "change_requests",
            "removed_items",
            "member_settings",
            "events",
            "calendars",
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM schedule.{table} WHERE workspace_id = $1"
            )))
            .bind(workspace)
            .execute(&mut **tx)
            .await?;
        }
        tracing::info!("schedule purged for a deleted workspace");
        Ok(())
    }
}
