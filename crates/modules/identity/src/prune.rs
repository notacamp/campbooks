//! `identity.prune`: delete what the issuer no longer needs, every
//! [`PRUNE_EVERY`]. Expired sign-in state (authorization requests, round
//! trips, pending sign-ups, passkey challenges, codes), held mailbox grants nobody adopted in
//! time, ended sessions, and refresh-token families that expired or were
//! revoked a day ago (kept that long so reuse of their tokens is still
//! recognised as such), revoked consents after 30 days, clients that
//! registered themselves (or are known by a metadata document) and went
//! unused for 30 days, and mailbox records that never got a connection
//! within [`UNNAMED_GRACE`] (with the way in a connect created that they
//! leave without a mailbox, as a disconnect would remove it).

use std::time::Duration;

use campbooks_kernel::{Job, JobContext, JobError};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::{domain::mailbox_links, features::forget_connected_way_in::announce};

/// How often [`Prune`] runs.
pub const PRUNE_EVERY: Duration = Duration::from_mins(15);

/// How long a dynamically registered or metadata-document client may go
/// unused before it is deleted, how long revoked consents are kept, and
/// how long a sent notice is remembered (far longer than its delivery
/// retries).
const UNUSED_CLIENT: TimeDelta = TimeDelta::days(30);

/// How long a mailbox record may wait for its connection: the round trip
/// that wrote it names it within seconds, or `connectors.connection_added`
/// does; one still unnamed after this never got a connection.
pub const UNNAMED_GRACE: TimeDelta = TimeDelta::days(1);

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
        let unused = now - UNUSED_CLIENT;
        for sql in [
            "DELETE FROM identity.consents WHERE revoked_at <= $1",
            "DELETE FROM identity.notices WHERE sent_at <= $1",
        ] {
            deleted += sqlx::query(sql)
                .bind(unused)
                .execute(db)
                .await
                .map_err(|e| JobError::retry(format!("pruning identity state: {e}")))?
                .rows_affected();
        }
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
        let ways_in = sweep_unnamed_mailboxes(ctx, now)
            .await
            .map_err(|e| JobError::retry(format!("pruning mailbox records: {e}")))?;
        tracing::info!(deleted, ways_in, "pruned expired sign-in state");
        Ok(())
    }
}

/// Delete mailbox records that never got a connection, and remove the ways
/// in they leave without a mailbox (audited and announced as a disconnect
/// would). How many ways in went.
pub async fn sweep_unnamed_mailboxes(
    ctx: &mut JobContext,
    now: chrono::DateTime<Utc>,
) -> campbooks_kernel::Result<usize> {
    let state = ctx.state().clone();
    let mut tx = state.db.begin().await?;
    let removed = mailbox_links::sweep_unnamed(&mut tx, now - UNNAMED_GRACE, now).await?;
    for gone in &removed {
        announce(&state, &mut tx, gone, "mailbox_never_connected", now).await?;
    }
    tx.commit().await?;
    Ok(removed.len())
}
