//! Explanations are kept 30 days after they were written (campbooks-cloud#407).
//!
//! - `conversations.dispatch_expire_explanations`, every day, reads which
//!   workspaces hold an explanation to expire (the maintenance scope: ids and
//!   times only, read only, through the table's maintenance policy) and
//!   enqueues `conversations.expire_explanations` in each, one pending job
//!   per workspace. It reads nothing else and writes nothing.
//! - `conversations.expire_explanations`, in the workspace (row-level
//!   security applies), deletes those explanations. A request that never
//!   wrote one counts from when it last changed.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::domain::explanation::KEEP_DAYS;

/// How often explanations to expire are looked for.
pub const EVERY: Duration = Duration::from_hours(24);
/// Workspaces dispatched per tick; the next tick takes the rest.
const DISPATCH_BATCH: i64 = 500;

/// The dispatcher tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchExpireExplanations;

impl Job for DispatchExpireExplanations {
    const NAME: &'static str = "conversations.dispatch_expire_explanations";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids and
        // times, read only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM conversations.explanations
              WHERE COALESCE(written_at, updated_at) < now() - make_interval(days => $1)
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
                &ExpireExplanations,
                EnqueueOptions::for_workspace(workspace.clone())
                    .unique("conversations.expire_explanations"),
            )
            .await?;
        }
        Ok(())
    }
}

/// Deletes, in its workspace, the explanations written more than 30 days ago.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpireExplanations;

impl Job for ExpireExplanations {
    const NAME: &'static str = "conversations.expire_explanations";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        if ctx.workspace().is_none() {
            return Err(JobError::fatal(
                "expire_explanations enqueued outside a workspace",
            ));
        }
        let tx = ctx.tx().await?;
        let expired = sqlx::query(
            "DELETE FROM conversations.explanations
              WHERE COALESCE(written_at, updated_at) < now() - make_interval(days => $1)",
        )
        .bind(KEEP_DAYS)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        tracing::info!(explanations = expired, "explanations expired");
        Ok(())
    }
}
