//! Needs that change with time alone (campbooks-cloud#275): an ask whose
//! lapse time passed (a sign-in link valid for ten minutes) is cleared as
//! lapsed, and an undated ask whose message left the needs window becomes
//! "Still needed?". A wait on someone (campbooks-cloud#293) becomes a
//! Nudge at its nudge time.
//!
//! Every refresh writes, in `conversations.need_clocks`, when the
//! conversation's open need next changes on its own (`needs::next_wake`), or
//! removes the row. Then:
//!
//! - `conversations.dispatch_need_clocks`, every minute, reads which
//!   workspaces have a row that is due (the maintenance scope: ids and times
//!   only, read only, through the table's maintenance policy) and enqueues
//!   `conversations.wake_needs` in each, one pending job per workspace. It
//!   reads nothing else and writes nothing.
//! - `conversations.wake_needs`, in the workspace (row-level security
//!   applies), works each due conversation's need out again
//!   ([`needs::refresh`], which settles it against the time now and writes
//!   its next wake), one conversation per transaction so it never holds one
//!   conversation's locks while waiting for another's, then removes the row
//!   if it is still due. A conversation that cannot be worked out is logged
//!   by id and tried again an hour later, so it cannot spin. What one job
//!   leaves, the next tick takes.

use std::time::Duration;

use campbooks_kernel::{
    AppState, Job, JobContext, JobError, JobHandler,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{
    domain::needs::{self, NeedSettings},
    model::ConversationId,
};

/// How often due clocks are looked for.
pub const EVERY: Duration = Duration::from_mins(1);

/// Workspaces woken per tick.
const DISPATCH_BATCH: i64 = 500;

/// Conversations worked out per job; the next tick takes the rest.
const PER_JOB: i64 = 200;

/// The dispatcher tick: enqueue [`WakeNeeds`] in each workspace with a need
/// due to change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchNeedClocks;

impl Job for DispatchNeedClocks {
    const NAME: &'static str = "conversations.dispatch_need_clocks";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids and
        // times, read only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM conversations.need_clocks
              WHERE wake_at <= now()
              ORDER BY workspace_id LIMIT $1",
        )
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_to_in::<WakeNeeds>(
                tx,
                &workspace,
                &WakeNeedsPayload,
                EnqueueOptions::for_workspace(workspace.clone())
                    .unique("conversations.wake_needs")
                    .priority(-5),
            )
            .await?;
        }
        Ok(())
    }
}

/// [`WakeNeeds`] takes nothing: it works on its workspace's due clocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeNeedsPayload;

/// Works out again, in its workspace, the needs due to change with time.
pub struct WakeNeeds {
    needs: NeedSettings,
}

impl WakeNeeds {
    /// A job settling needs under `needs` (its window decides when an
    /// undated ask is asked again).
    pub fn new(needs: NeedSettings) -> Self {
        Self { needs }
    }
}

impl JobHandler for WakeNeeds {
    type Payload = WakeNeedsPayload;
    const NAME: &'static str = "conversations.wake_needs";
    const QUEUE: &'static str = "rules";

    async fn run(&self, ctx: &mut JobContext, _payload: WakeNeedsPayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("wake_needs enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let due: Vec<ConversationId> = {
            let mut tx = state.db.begin_for(workspace).await?;
            let due = sqlx::query_scalar(
                "SELECT conversation_id FROM conversations.need_clocks
                  WHERE wake_at <= now()
                  ORDER BY wake_at, conversation_id LIMIT $1",
            )
            .bind(PER_JOB)
            .fetch_all(&mut *tx)
            .await?;
            tx.commit().await?;
            due
        };
        for id in due {
            if let Err(error) = wake(&state, workspace, id, &self.needs).await {
                tracing::warn!(
                    conversation = %id,
                    error = error.code(),
                    "a need due to change could not be worked out; trying again in an hour"
                );
                let mut tx = state.db.begin_for(workspace).await?;
                sqlx::query(
                    "UPDATE conversations.need_clocks SET wake_at = now() + interval '1 hour'
                      WHERE conversation_id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
        }
        Ok(())
    }
}

/// Work conversation `id`'s need out again in a transaction of its own,
/// then forget its clock if the refresh left it due (it moves it ahead, or
/// removes it, whenever the need can still change).
async fn wake(
    state: &AppState,
    workspace: WorkspaceId,
    id: ConversationId,
    settings: &NeedSettings,
) -> campbooks_kernel::Result<()> {
    let mut tx = state.db.begin_for(workspace).await?;
    needs::refresh(&mut tx, &state.events, &workspace.to_string(), id, settings).await?;
    sqlx::query(
        "DELETE FROM conversations.need_clocks WHERE conversation_id = $1 AND wake_at <= now()",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests;
