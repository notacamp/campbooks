//! `identity.prune`: delete what the issuer no longer needs, every
//! [`PRUNE_EVERY`]. Expired sign-in state (authorization requests, round
//! trips, pending sign-ups, codes), held mailbox grants nobody adopted in
//! time, ended sessions, and refresh-token families that expired or were
//! revoked a day ago (kept that long so reuse of their tokens is still
//! recognised as such), revoked consents after 30 days, and clients that
//! registered themselves (or are known by a metadata document) and went
//! unused for 30 days.

use std::time::Duration;

use campbooks_kernel::{Job, JobContext, JobError};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// How often [`Prune`] runs.
pub const PRUNE_EVERY: Duration = Duration::from_mins(15);

/// How long a dynamically registered or metadata-document client may go
/// unused before it is deleted, and how long revoked consents are kept.
const UNUSED_CLIENT: TimeDelta = TimeDelta::days(30);

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
        let unused = now - UNUSED_CLIENT;
        deleted += sqlx::query("DELETE FROM identity.consents WHERE revoked_at <= $1")
            .bind(unused)
            .execute(db)
            .await
            .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?
            .rows_affected();
        let mut tx = crate::domain::clients::begin_as_server(db)
            .await
            .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?;
        // Clients nobody set up deliberately and nobody uses: registered
        // dynamically, or known by a document, and idle for 30 days.
        deleted += sqlx::query(
            "DELETE FROM identity.clients
              WHERE kind IN ('dynamic', 'metadata_document')
                AND coalesce(last_used_at, created_at) <= $1",
        )
        .bind(unused)
        .execute(&mut *tx)
        .await
        .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?
        .rows_affected();
        tx.commit()
            .await
            .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?;
        tracing::info!(deleted, "pruned expired sign-in state");
        Ok(())
    }
}
