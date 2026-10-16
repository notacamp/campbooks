//! The mailboxes a way in brought (`identity.mailbox_links`): a way in that
//! came only from connecting a mailbox goes with its last one
//! (campbooks-cloud#117).
//!
//! One row per workspace the account's mailbox is connected in (the
//! connectors module keeps one connection per workspace, provider and
//! account). It is written in the transaction that creates the way in, or
//! before a later grant is handed over, so a way in is never left
//! untracked: if the row cannot be written, the round trip fails. A way in
//! linked before this record existed has no rows and is never removed here
//! (some of its connections would be unknown).
//!
//! Removal locks the account first, then deletes the row and counts the
//! others: the same order as removing a way in on Ways to sign in (account,
//! then the rows its deletion cascades to), so the two never deadlock, and
//! two of its mailboxes removed at once are counted one after the other.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{accounts, sessions, ways_in};

/// Record that the mailbox of way in `identity_id` is connected in
/// `workspace`: always when the round trip created the way in (`created`),
/// otherwise only when the way in is already tracked. Run it in the
/// transaction that creates the way in, or before the grant is handed over.
/// Whether this call wrote the row (so a failed hand-over can take it back
/// with [`unrecord`]).
pub async fn record(
    conn: &mut PgConnection,
    identity_id: Uuid,
    workspace: Uuid,
    created: bool,
    now: DateTime<Utc>,
) -> Result<bool> {
    let inserted = sqlx::query(
        "INSERT INTO identity.mailbox_links (provider_identity_id, in_workspace_id, created_at)
         SELECT $1, $2, $3
          WHERE $4 OR EXISTS (SELECT 1 FROM identity.mailbox_links
                               WHERE provider_identity_id = $1)
         ON CONFLICT (provider_identity_id, in_workspace_id) DO NOTHING",
    )
    .bind(identity_id)
    .bind(workspace)
    .bind(now)
    .bind(created)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(inserted > 0)
}

/// Take back a row [`record`] wrote for a hand-over that then failed: no
/// connection came of it, so nothing may later be removed on its account.
/// Only a row no connection was named on yet.
pub async fn unrecord(conn: &mut PgConnection, identity_id: Uuid, workspace: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM identity.mailbox_links
          WHERE provider_identity_id = $1 AND in_workspace_id = $2 AND connection_id IS NULL",
    )
    .bind(identity_id)
    .bind(workspace)
    .execute(conn)
    .await?;
    Ok(())
}

/// Note which connection the mailbox of way in `identity_id` in `workspace`
/// became (for showing it; nothing is decided from it).
pub async fn name_connection(
    conn: &mut PgConnection,
    identity_id: Uuid,
    workspace: Uuid,
    connection: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE identity.mailbox_links SET connection_id = $3
          WHERE provider_identity_id = $1 AND in_workspace_id = $2",
    )
    .bind(identity_id)
    .bind(workspace)
    .bind(connection)
    .execute(conn)
    .await?;
    Ok(())
}

/// A way in that went with its last mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// Its account.
    pub account: Uuid,
    /// The way in.
    pub way_in: Uuid,
    /// Its provider.
    pub provider: String,
    /// Sessions signed in with it, now ended.
    pub sessions_ended: usize,
}

/// Connection `connection` (of `provider` account `subject`) in `workspace`
/// was disconnected: forget it and, when it was the last mailbox of a way in
/// a connect created and that way in is not the account's last, remove the
/// way in (ending its sessions and revoking their refresh tokens). Locks the
/// account before touching the record.
///
/// Only the row naming that very connection counts. A row with no
/// connection named yet (a hand-over still in flight, or one whose naming
/// failed) is never acted on: the way in stays. So another member's
/// connection of the same account in the same workspace (theirs, not this
/// way in's) can never remove it.
pub async fn forget(
    conn: &mut PgConnection,
    workspace: Uuid,
    connection: &str,
    provider: &str,
    subject: &str,
    now: DateTime<Utc>,
) -> Result<Option<Removed>> {
    let found: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT p.id, p.account_id, p.linked_via
           FROM identity.mailbox_links l
           JOIN identity.provider_identities p ON p.id = l.provider_identity_id
          WHERE l.in_workspace_id = $1 AND l.connection_id = $2
            AND p.provider = $3 AND p.subject = $4",
    )
    .bind(workspace)
    .bind(connection)
    .bind(provider)
    .bind(subject)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((way_in, account, linked_via)) = found else {
        return Ok(None);
    };
    // The account first, as removing a way in does.
    let count = ways_in::lock_and_count(conn, account).await?;
    let deleted = sqlx::query(
        "DELETE FROM identity.mailbox_links
          WHERE provider_identity_id = $1 AND in_workspace_id = $2 AND connection_id = $3",
    )
    .bind(way_in)
    .bind(workspace)
    .bind(connection)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    let others: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM identity.mailbox_links WHERE provider_identity_id = $1)",
    )
    .bind(way_in)
    .fetch_one(&mut *conn)
    .await?;
    if deleted == 0 || others || linked_via != "connect" || count <= 1 {
        return Ok(None);
    }
    let sessions_ended = sessions::end_signed_in_with(conn, way_in, now).await?;
    let Some(provider) = accounts::remove(conn, way_in, account).await? else {
        return Ok(None);
    };
    Ok(Some(Removed {
        account,
        way_in,
        provider,
        sessions_ended,
    }))
}

/// The mailboxes recorded for each of `account`'s ways in: how many, and the
/// connections known among them.
pub async fn of_account(
    conn: &mut PgConnection,
    account: Uuid,
) -> Result<Vec<(Uuid, i64, Vec<String>)>> {
    Ok(sqlx::query_as(
        "SELECT l.provider_identity_id, count(*),
                coalesce(array_agg(l.connection_id) FILTER (WHERE l.connection_id IS NOT NULL),
                         '{}')
           FROM identity.mailbox_links l
           JOIN identity.provider_identities p ON p.id = l.provider_identity_id
          WHERE p.account_id = $1
          GROUP BY l.provider_identity_id",
    )
    .bind(account)
    .fetch_all(conn)
    .await?)
}
