//! Announcing, once, the invitations stored before invitations were
//! announced (campbooks-cloud#360).
//!
//! The migration that made invitations Decide items
//! (`…_schedule__invitations.sql`) queued, in
//! `schedule.invitation_backfills`, every workspace with an event the
//! member has not answered. Then:
//!
//! - `schedule.dispatch_invitation_backfills`, every minute, reads which
//!   workspaces still have a row there (the maintenance scope: ids only,
//!   through the table's maintenance policy) and enqueues
//!   `schedule.announce_invitations` in each. It reads nothing else and
//!   writes nothing.
//! - `schedule.announce_invitations`, in the workspace, announces every event
//!   that awaits the member's answer (`domain::invitations::announce`), from
//!   this module's own rows: no resync, no model. Then it removes the
//!   workspace's row. Running it again announces the same invitations with a
//!   newer revision, which changes nothing.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{domain::invitations, model::EventId};

/// How often workspaces with invitations still to announce are looked for.
pub const EVERY: Duration = Duration::from_mins(1);

/// Workspaces enqueued per tick.
const DISPATCH_BATCH: i64 = 500;

/// The dispatcher tick: enqueue [`AnnounceInvitations`] in each workspace
/// still queued for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchInvitationBackfills;

impl Job for DispatchInvitationBackfills {
    const NAME: &'static str = "schedule.dispatch_invitation_backfills";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids
        // only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT workspace_id FROM schedule.invitation_backfills
              ORDER BY workspace_id LIMIT $1",
        )
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_in(
                tx,
                &workspace,
                &AnnounceInvitations,
                EnqueueOptions::for_workspace(workspace.clone())
                    .unique("schedule.announce_invitations")
                    .priority(-10),
            )
            .await?;
        }
        Ok(())
    }
}

/// Announces, in the job's workspace, every event that awaits its member's
/// answer, then takes the workspace off the queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnounceInvitations;

impl Job for AnnounceInvitations {
    const NAME: &'static str = "schedule.announce_invitations";
    const QUEUE: &'static str = "maintenance";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("announce_invitations enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let unanswered: Vec<EventId> = sqlx::query_scalar(
            "SELECT id FROM schedule.events
              WHERE response = 'needs_action' AND series_id IS NULL
                AND status <> 'cancelled' AND deleted_at IS NULL
                AND (starts_at > now() OR starts_on >= current_date - 1)
              ORDER BY id",
        )
        .fetch_all(&mut **tx)
        .await?;
        for event in unanswered {
            invitations::announce(&state, tx, workspace, event).await?;
        }
        sqlx::query("DELETE FROM schedule.invitation_backfills WHERE workspace_id = $1")
            .bind(workspace)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
