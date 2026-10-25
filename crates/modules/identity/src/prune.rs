//! `identity.prune`: delete what the issuer no longer needs, every
//! [`PRUNE_EVERY`]. Expired sign-in state (authorization requests, round
//! trips, pending sign-ups, passkey challenges, codes), held mailbox grants nobody adopted in
//! time, ended sessions, and refresh-token families that expired or were
//! revoked a day ago (kept that long so reuse of their tokens is still
//! recognised as such), revoked consents after 30 days, clients that
//! registered themselves (or are known by a metadata document) and went
//! unused for 30 days, and mailbox records that never got a connection
//! within [`UNNAMED_GRACE`] (once the connectors module confirms there is
//! none; with the way in a connect created that they leave without a
//! mailbox, as a disconnect would remove it).

use std::time::Duration;

use campbooks_kernel::{Job, JobContext, JobError, redact, tenancy::WorkspaceId};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    domain::mailbox_links,
    features::forget_connected_way_in::{announce, track_removed},
};

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
        let unused = now - UNUSED_CLIENT;
        // Each in batches of short statements (campbooks-cloud#254).
        for (sql, at) in [
            (
                "DELETE FROM identity.authorize_requests WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.authorize_requests WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.upstream_flows WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.upstream_flows WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.link_tickets WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.link_tickets WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.pending_sign_ups WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.pending_sign_ups WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.webauthn_challenges WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.webauthn_challenges WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.held_grants WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.held_grants WHERE expires_at <= $1 LIMIT $2))",
                now,
            ),
            (
                "DELETE FROM identity.authorization_codes WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.authorization_codes WHERE expires_at <= $1 LIMIT $2))",
                ended,
            ),
            (
                "DELETE FROM identity.refresh_families WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.refresh_families WHERE expires_at <= $1 OR revoked_at <= $1 LIMIT $2))",
                ended,
            ),
            (
                "DELETE FROM identity.sessions WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.sessions WHERE ended_at <= $1 OR least(idle_expires_at, expires_at) <= $1 LIMIT $2))",
                ended,
            ),
            (
                "DELETE FROM identity.consents WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.consents WHERE revoked_at <= $1 LIMIT $2))",
                unused,
            ),
            (
                "DELETE FROM identity.notices WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.notices WHERE sent_at <= $1 LIMIT $2))",
                unused,
            ),
        ] {
            deleted += delete_in_batches(db, sql, at, PRUNE_BATCH).await?;
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
        .map_err(|e| JobError::retry(format!("pruning identity state: {}", redact::sqlx(&e))))?
        .rows_affected();
        tx.commit().await.map_err(|e| {
            JobError::retry(format!("pruning identity state: {}", redact::sqlx(&e)))
        })?;
        let ways_in = sweep_unnamed_mailboxes(ctx, now)
            .await
            .map_err(|e| JobError::retry(format!("pruning mailbox records: {e}")))?;
        tracing::info!(deleted, ways_in, "pruned expired sign-in state");
        Ok(())
    }
}

/// Rows one prune statement deletes at most.
const PRUNE_BATCH: i64 = 5_000;

/// Run `sql` (a `DELETE` of at most `$2` rows due by `$1`) with `size`
/// until it deletes fewer: each statement stays short, well within the
/// runtime pool's `statement_timeout`. How many rows went.
async fn delete_in_batches(
    db: &sqlx::PgPool,
    sql: &'static str,
    at: chrono::DateTime<Utc>,
    size: i64,
) -> Result<u64, JobError> {
    let mut deleted = 0;
    loop {
        let batch = sqlx::query(sql)
            .bind(at)
            .bind(size)
            .execute(db)
            .await
            .map_err(|e| JobError::retry(format!("pruning identity state: {}", redact::sqlx(&e))))?
            .rows_affected();
        deleted += batch;
        if batch < u64::try_from(size).unwrap_or(u64::MAX) {
            return Ok(deleted);
        }
    }
}

/// Settle mailbox records unnamed past [`UNNAMED_GRACE`]: ask the
/// connectors module whether each one's connection exists (then name it),
/// delete the rest, and remove the ways in they leave without a mailbox
/// (audited and announced as a disconnect would). One transaction per
/// account, in `account_id` order; a record the port could not answer for
/// is left for the next run. How many ways in went.
pub async fn sweep_unnamed_mailboxes(
    ctx: &mut JobContext,
    now: chrono::DateTime<Utc>,
) -> campbooks_kernel::Result<usize> {
    let state = ctx.state().clone();
    let cutoff = now - UNNAMED_GRACE;
    let unnamed = {
        let mut conn = state.db.acquire().await?;
        mailbox_links::unnamed_before(&mut conn, cutoff).await?
    };
    let mut gone = 0;
    for rows in unnamed.chunk_by(|a, b| a.account == b.account) {
        let account = rows[0].account;
        let mut checked = Vec::with_capacity(rows.len());
        for row in rows {
            let connection = state
                .grants
                .connection_of(
                    &state,
                    &account.to_string(),
                    WorkspaceId::from_uuid(row.workspace),
                    &row.provider,
                    &row.subject,
                )
                .await;
            match connection {
                Ok(connection) => checked.push((row.clone(), connection)),
                Err(error) => {
                    tracing::warn!(%error, "could not check a mailbox record's connection");
                }
            }
        }
        let mut tx = state.db.begin().await?;
        let removed =
            mailbox_links::settle_unnamed(&mut tx, account, &checked, cutoff, now).await?;
        for way_in in &removed {
            announce(&state, &mut tx, way_in, "mailbox_never_connected", now).await?;
            track_removed(
                &state,
                &mut tx,
                way_in,
                crate::tracking::WayInRemoval::MailboxNeverConnected,
            )
            .await?;
        }
        tx.commit().await?;
        gone += removed.len();
    }
    Ok(gone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn expired_rows_go_in_batches_and_live_ones_stay(db: sqlx::PgPool) {
        let now = Utc::now();
        for (n, expires) in [-5, -4, -3, -2, -1, 10].into_iter().enumerate() {
            sqlx::query(
                "INSERT INTO identity.webauthn_challenges
                     (challenge_hash, purpose, browser_hash, return_to, created_at, expires_at)
                 VALUES ($1, 'sign_in', '\\x00', '/', $2, $3)",
            )
            .bind(vec![u8::try_from(n).unwrap()])
            .bind(now - TimeDelta::minutes(10))
            .bind(now + TimeDelta::minutes(expires))
            .execute(&db)
            .await
            .unwrap();
        }
        let deleted = delete_in_batches(
            &db,
            "DELETE FROM identity.webauthn_challenges WHERE ctid = ANY(ARRAY(SELECT ctid FROM identity.webauthn_challenges WHERE expires_at <= $1 LIMIT $2))",
            now,
            2,
        )
        .await
        .unwrap();
        assert_eq!(deleted, 5);
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.webauthn_challenges")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(left, 1);
    }
}
