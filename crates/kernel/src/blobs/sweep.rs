//! The orphan sweep: the kernel's recurring blob maintenance.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::jobs::{Job, JobContext, JobError};

/// How often the kernel sweeps blobs nothing references.
pub const SWEEP_EVERY: Duration = Duration::from_hours(1);

/// What one sweep deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepStats {
    /// Blobs with no reference, past their grace period.
    pub blobs: u64,
    /// Uploads abandoned before they completed (a crash mid-upload).
    pub abandoned_uploads: u64,
}

/// Delete blobs no record references once their grace period
/// (`CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS`) has passed, and abandoned uploads.
/// Scheduled hourly by the kernel; never deletes a blob that gained a
/// reference meanwhile (see [`BlobStore`](super::BlobStore)).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SweepBlobs;

impl Job for SweepBlobs {
    const NAME: &'static str = "kernel.sweep_blobs";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let blobs = &ctx.state().blobs;
        if blobs.backend().is_none() {
            // A process without blob storage (tools, some tests) has nothing
            // to sweep.
            return Ok(());
        }
        let stats = blobs.sweep().await?;
        tracing::info!(
            blobs = stats.blobs,
            abandoned_uploads = stats.abandoned_uploads,
            "swept unreferenced blobs"
        );
        Ok(())
    }
}
