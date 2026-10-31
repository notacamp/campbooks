//! A scheduled send must never stay scheduled for ever (campbooks-cloud#392).
//!
//! `conversations.release_send` is a queued job at the send's time. If the
//! queue lost it (a database restored from before it was queued, a job that
//! died), the draft would stay `scheduled` with a past `send_at`, and the
//! member would never hear.
//!
//! - `conversations.dispatch_overdue_sends`, every ten minutes, asks which
//!   workspaces have a send still scheduled past [`OVERDUE`]
//!   (`conversations.workspaces_with_overdue_sends`, a `SECURITY DEFINER`
//!   function that answers workspace ids only and may read three columns
//!   of the drafts, nothing else) and enqueues
//!   `conversations.sweep_overdue_sends` in each, one pending job per
//!   workspace: only workspaces with work to do. It reads nothing else and
//!   writes nothing.
//! - `conversations.sweep_overdue_sends`, in the workspace (row-level
//!   security applies), finds the drafts still scheduled fifteen minutes
//!   past their time with no unfinished release job for their generation,
//!   and marks them `failed`, `release_lost`, by the release's own guarded
//!   update (only the same generation, only while still scheduled). It never
//!   queues the release again: after a restore the old job may already have
//!   sent, and sending twice is worse than asking the member to check.
//!   A draft whose job is still queued (the workers are behind) is left to it.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError, JobHandler,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{
    features::release_send::{ReleaseSend, ReleaseSendPayload, stop_scheduled},
    model::DraftId,
    tracking,
};

/// How often overdue sends are looked for.
pub const EVERY: Duration = Duration::from_mins(10);

/// Workspaces looked at per tick.
const DISPATCH_BATCH: i32 = 5000;

/// How long past its time a scheduled send is left to its release job
/// before it counts as overdue.
const OVERDUE_MINUTES: i32 = 15;

/// Drafts settled per job; the next tick takes the rest.
const PER_JOB: i64 = 200;

/// The dispatcher tick: enqueue [`SweepOverdueSends`] in each workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchOverdueSends;

impl Job for DispatchOverdueSends {
    const NAME: &'static str = "conversations.dispatch_overdue_sends";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, ids only, through the definer function (no
        // policy lets this transaction read a draft): the maintenance scope,
        // allowed for this file in `architecture.toml`. Each job is enqueued
        // in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT conversations.workspaces_with_overdue_sends(
                 now() - make_interval(mins => $1), $2)",
        )
        .bind(OVERDUE_MINUTES)
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_to_in::<SweepOverdueSends>(
                tx,
                &workspace,
                &SweepPayload,
                EnqueueOptions::for_workspace(workspace.clone())
                    .unique("conversations.sweep_overdue_sends")
                    .priority(-5),
            )
            .await?;
        }
        Ok(())
    }
}

/// [`SweepOverdueSends`] takes nothing: it works on its workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepPayload;

/// Gives up, in its workspace, the scheduled sends whose release was lost.
#[derive(Debug, Clone, Copy, Default)]
pub struct SweepOverdueSends;

impl JobHandler for SweepOverdueSends {
    type Payload = SweepPayload;
    const NAME: &'static str = "conversations.sweep_overdue_sends";
    const QUEUE: &'static str = "default";

    async fn run(&self, ctx: &mut JobContext, _payload: SweepPayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("sweep_overdue_sends enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let ws = workspace.to_string();
        let tx = ctx.tx().await?;
        let overdue: Vec<(DraftId, i32)> = sqlx::query_as(
            "SELECT id, send_generation FROM conversations.drafts
              WHERE state = 'scheduled' AND send_at < now() - make_interval(mins => $2)
              ORDER BY send_at LIMIT $1",
        )
        .bind(PER_JOB)
        .bind(OVERDUE_MINUTES)
        .fetch_all(&mut **tx)
        .await?;
        let mut lost = 0_u32;
        for (draft, generation) in overdue {
            // Its release is still queued or running (the workers are
            // behind): it will settle the draft itself.
            let key = format!("release:{draft}:{generation}");
            if jobs::unfinished_run_at_to::<ReleaseSend>(tx, &ws, &key)
                .await?
                .is_some()
            {
                continue;
            }
            let payload = ReleaseSendPayload { draft, generation };
            let reason = ("release_lost", tracking::UnscheduleReason::ReleaseLost);
            if stop_scheduled(&state, tx, workspace, &payload, reason).await? {
                lost += 1;
            }
        }
        if lost > 0 {
            tracing::warn!(%workspace, drafts = lost, "scheduled sends found overdue with no release job; they were not sent");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
