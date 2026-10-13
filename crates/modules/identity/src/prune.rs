//! `identity.prune`: delete what the issuer no longer needs, every
//! [`PRUNE_EVERY`]. Expired sign-in state (authorization requests, round
//! trips, pending sign-ups, passkey challenges, codes), held mailbox grants nobody adopted in
//! time, ended sessions, and refresh-token families that expired or were
//! revoked a day ago (kept that long so reuse of their tokens is still
//! recognised as such).

use std::time::Duration;

use campbooks_kernel::{Job, JobContext, JobError};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// How often [`Prune`] runs.
pub const PRUNE_EVERY: Duration = Duration::from_mins(15);

/// How long revoked or expired families and ended sessions are kept.
const KEEP_ENDED: TimeDelta = TimeDelta::days(1);

/// The prune job.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Prune;

impl Job for Prune {
    const NAME: &'static str = "identity.prune";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let now = Utc::now();
        let ended = now - KEEP_ENDED;
        let db = ctx.pool();
        let mut deleted = 0;
        for (sql, at) in [
            (
                "DELETE FROM identity.authorize_requests WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.upstream_flows WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.link_tickets WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.pending_sign_ups WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.webauthn_challenges WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.held_grants WHERE expires_at <= $1",
                now,
            ),
            (
                "DELETE FROM identity.authorization_codes WHERE expires_at <= $1",
                ended,
            ),
            (
                "DELETE FROM identity.refresh_families
                  WHERE expires_at <= $1 OR revoked_at <= $1",
                ended,
            ),
            (
                "DELETE FROM identity.sessions
                  WHERE ended_at <= $1 OR least(idle_expires_at, expires_at) <= $1",
                ended,
            ),
        ] {
            deleted += sqlx::query(sql)
                .bind(at)
                .execute(db)
                .await
                .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?
                .rows_affected();
        }
        tracing::info!(deleted, "pruned expired sign-in state");
        Ok(())
    }
}
