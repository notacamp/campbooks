//! Re-tagging under a rotated instance secret key (campbooks-cloud#191).
//!
//! While a previous key is set (`CAMPBOOKS_SECRET_KEY_PREVIOUS`), what was
//! bound under it (a taught rule, a rule's application, a scheduled send)
//! still verifies, but stops verifying once the key is removed. Each module
//! that keeps bound rows registers a per-workspace job with
//! [`Background::rekey`](super::Background::rekey) that re-tags them with the
//! current key (`Binding::retag`: only what verifies under the previous key,
//! which adds no trust). [`DispatchRekeys`] enqueues those jobs in every
//! workspace that has a change feed, every [`REKEY_EVERY`] while a previous
//! key is set, and does nothing otherwise. Each job logs what it re-tagged
//! and what is left; the runbook waits for nothing left before the previous
//! key is removed.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{EnqueueOptions, Job, JobContext, JobError, NewJob};

/// How often the jobs are enqueued while a previous key is set (the first
/// tick within this long of a start).
pub const REKEY_EVERY: Duration = Duration::from_mins(15);

/// Enqueue every registered rekey job in every workspace, while a previous
/// key is set. Scheduled by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DispatchRekeys;

impl Job for DispatchRekeys {
    const NAME: &'static str = "kernel.dispatch_rekeys";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        if !ctx.state().binding.has_previous() {
            return Ok(());
        }
        let rekeys = ctx.state().events.registry().rekeys().to_vec();
        if rekeys.is_empty() {
            return Ok(());
        }
        // Every workspace with a change feed: ids only.
        let tx = ctx.kernel_tx().await?;
        let workspaces: Vec<String> =
            sqlx::query_scalar("SELECT workspace_id FROM kernel.change_heads ORDER BY 1")
                .fetch_all(&mut **tx)
                .await?;
        for workspace in &workspaces {
            for rekey in &rekeys {
                super::insert_row(
                    tx,
                    NewJob {
                        kind: rekey.name,
                        queue: rekey.queue,
                        max_attempts: rekey.max_attempts,
                        payload: rekey.payload.clone(),
                    },
                    EnqueueOptions::for_workspace(workspace.clone())
                        .unique(rekey.name)
                        .priority(-10),
                )
                .await?;
            }
        }
        tracing::info!(
            workspaces = workspaces.len(),
            jobs = rekeys.len(),
            "a previous instance secret key is set: re-tagging what it bound with the current \
             one, in every workspace"
        );
        Ok(())
    }
}
