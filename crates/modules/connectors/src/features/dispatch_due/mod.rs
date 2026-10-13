//! `connectors.dispatch_due` — every minute, enqueue the runs of the
//! streams that are due (see `domain::schedule`).

use std::time::Duration;

use campbooks_kernel::{Job, JobContext, JobError};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::schedule;

/// How often the dispatcher ticks.
pub const EVERY: Duration = Duration::from_mins(1);

/// The dispatcher tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchDue;

impl Job for DispatchDue {
    const NAME: &'static str = "connectors.dispatch_due";
    const QUEUE: &'static str = "sync";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let dispatched = schedule::dispatch(ctx.pool(), Utc::now()).await?;
        if dispatched > 0 {
            tracing::debug!(dispatched, "streams due");
        }
        Ok(())
    }
}
