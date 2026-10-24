//! Re-tagging this module's pending outbound changes after a rotation of
//! the instance secret key (campbooks-cloud#262).
//!
//! An outbound change (a label, archive, trash, spam or flag change a member
//! asked for, waiting to be made at the provider) carries a MAC under the
//! kernel's `Binding` (campbooks-cloud#227). While a previous key is set it
//! verifies under it too; once that key is removed, a change still pending
//! (its stream paused, or the provider down for the whole rotation) would
//! end `skipped` (`unverified`) and its message be read again from the
//! provider. So the kernel enqueues this job in every workspace while a
//! previous key is set (`Background::rekey`): it re-tags, with a
//! compare-and-set, every pending change whose MAC verifies under the
//! previous key only (`domain::outbound::retag`), and logs what is left,
//! like `conversations.retag_bindings`.

use campbooks_kernel::{Job, JobContext, JobError, tenancy::WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{calendar::changes, domain::outbound};

/// Re-tag, in the job's workspace, the pending outbound changes bound under
/// the previous key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetagBindings;

impl Job for RetagBindings {
    const NAME: &'static str = "connectors.retag_bindings";
    const QUEUE: &'static str = "maintenance";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("retag_bindings enqueued outside a workspace"))?;
        let key = ctx.state().binding.clone();
        if !key.has_previous() {
            return Ok(());
        }
        let tx = ctx.tx().await?;
        let (retagged, left) = outbound::retag(tx, &key, workspace).await?;
        // The calendar changes members asked for are bound the same way.
        let (cal_retagged, cal_left) = changes::retag(tx, &key, workspace).await?;
        let (retagged, left) = (retagged + cal_retagged, left + cal_left);
        if left == 0 {
            tracing::info!(
                %workspace,
                retagged,
                left = 0,
                "binding re-tag: no outbound change in this workspace is bound under the \
                 previous key any more"
            );
        } else {
            tracing::warn!(
                %workspace,
                retagged,
                left,
                "binding re-tag: outbound changes bound under the previous key changed while \
                 they were re-tagged; the next tick tries again"
            );
        }
        Ok(())
    }
}
