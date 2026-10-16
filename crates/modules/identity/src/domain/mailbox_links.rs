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
//! The row is named with its connection by the round trip once the
//! connectors module accepts the grant (a fast path), and, durably, from
//! `connectors.connection_added`, published in the transaction that stores
//! the grant. A round trip whose grant is refused takes its row back, and
//! the way in it created with it. Rows still unnamed a day later are
//! checked with the connectors module (the grant-intake port): one whose
//! connection exists is named; the rest never got one (the round trip died
//! between writing the row and handing the grant over), so
//! [`settle_unnamed`] deletes them, and the way in they leave without a
//! mailbox goes as it would on a disconnect.
//!
//! Every change that can remove a way in locks the account first, then
//! touches the rows: the same order as removing a way in on Ways to sign in
//! (account, then the rows its deletion cascades to), so they never
//! deadlock and two of them are counted one after the other.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{accounts, sessions, ways_in};

/// What [`record`] found and did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// It wrote the row.
    Inserted,
    /// The row was there already.
    Existed,
    /// The way in is not tracked (linked before this record existed, or
    /// not by a connect): nothing written.
    Untracked,
}

impl Recorded {
    /// Whether the way in's mailboxes are recorded here.
    pub const fn tracked(self) -> bool {
        !matches!(self, Self::Untracked)
    }
}

/// Record that the mailbox of way in `identity_id` is connected in
/// `workspace`: always when the round trip created the way in (`created`),
/// otherwise only when the way in is already tracked. A row still unnamed
/// gets a fresh `created_at` (the sweep waits for this round trip too). Run
/// it in the transaction that creates the way in, or before the grant is
/// handed over.
pub async fn record(
    conn: &mut PgConnection,
    identity_id: Uuid,
    workspace: Uuid,
    created: bool,
    now: DateTime<Utc>,
) -> Result<Recorded> {
    let tracked: bool = created
        || sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM identity.mailbox_links WHERE provider_identity_id = $1)",
        )
        .bind(identity_id)
        .fetch_one(&mut *conn)
        .await?;
    if !tracked {
        return Ok(Recorded::Untracked);
    }
    // A row still unnamed is written afresh: a connect retried while an
    // earlier one never finished must not be swept mid-flight.
    let inserted: Option<bool> = sqlx::query_scalar(
        "INSERT INTO identity.mailbox_links (provider_identity_id, in_workspace_id, created_at)
         VALUES ($1, $2, $3)
         ON CONFLICT (provider_identity_id, in_workspace_id)
         DO UPDATE SET created_at = $3 WHERE identity.mailbox_links.connection_id IS NULL
         RETURNING xmax = 0",
    )
    .bind(identity_id)
    .bind(workspace)
    .bind(now)
    .fetch_optional(conn)
    .await?;
    Ok(if inserted == Some(true) {
        Recorded::Inserted
    } else {
        Recorded::Existed
    })
}

/// Name `connection` on the row of way in `identity_id` in `workspace`,
/// writing the row if it went missing meanwhile (another round trip's
/// take-back). For a tracked way in only: the caller says so.
pub async fn name_connection(
    conn: &mut PgConnection,
    identity_id: Uuid,
    workspace: Uuid,
    connection: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO identity.mailbox_links
             (provider_identity_id, in_workspace_id, connection_id, created_at)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (provider_identity_id, in_workspace_id)
         DO UPDATE SET connection_id = EXCLUDED.connection_id",
    )
    .bind(identity_id)
    .bind(workspace)
    .bind(connection)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// `connectors.connection_added`: connection `connection` of `provider`
/// account `subject` in `workspace` was granted by the account `grantor`.
/// When that account has a tracked way in of that provider account, name
/// the connection on its row (writing the row if it is missing). Nothing
/// for anyone else's way in, or an untracked one. Idempotent.
pub async fn name_from_event(
    conn: &mut PgConnection,
    workspace: Uuid,
    connection: &str,
    provider: &str,
    subject: &str,
    grantor: Uuid,
    now: DateTime<Utc>,
) -> Result<bool> {
    let way_in: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM identity.provider_identities
          WHERE provider = $1 AND subject = $2 AND account_id = $3",
    )
    .bind(provider)
    .bind(subject)
    .bind(grantor)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(way_in) = way_in else {
        return Ok(false);
    };
    // The account first, as every change that can remove a way in.
    ways_in::lock_and_count(conn, grantor).await?;
    let tracked: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM identity.mailbox_links WHERE provider_identity_id = $1)",
    )
    .bind(way_in)
    .fetch_one(&mut *conn)
    .await?;
    if !tracked {
        return Ok(false);
    }
    name_connection(conn, way_in, workspace, connection, now).await?;
    Ok(true)
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
    remove(conn, way_in, account, now).await
}

/// The hand-over of a round trip that wrote the row of way in `identity_id`
/// in `workspace` failed: no connection came of it. Take the row back and,
/// when this round trip also created the way in (`created`) and it has no
/// other mailbox, remove the way in too, unless it is the account's last.
/// A row the connectors module named meanwhile stays.
pub async fn take_back(
    conn: &mut PgConnection,
    identity_id: Uuid,
    workspace: Uuid,
    created: bool,
    now: DateTime<Utc>,
) -> Result<Option<Removed>> {
    let account: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT account_id, linked_via FROM identity.provider_identities WHERE id = $1",
    )
    .bind(identity_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((account, linked_via)) = account else {
        return Ok(None);
    };
    let count = ways_in::lock_and_count(conn, account).await?;
    sqlx::query(
        "DELETE FROM identity.mailbox_links
          WHERE provider_identity_id = $1 AND in_workspace_id = $2 AND connection_id IS NULL",
    )
    .bind(identity_id)
    .bind(workspace)
    .execute(&mut *conn)
    .await?;
    if !created || linked_via != "connect" || count <= 1 || has_rows(conn, identity_id).await? {
        return Ok(None);
    }
    remove(conn, identity_id, account, now).await
}

/// A row unnamed since before a cutoff: either its connection was made and
/// never named here (both namings failed), or the round trip died before
/// handing its grant over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unnamed {
    /// The way in.
    pub way_in: Uuid,
    /// Its account.
    pub account: Uuid,
    /// How the way in was linked.
    pub linked_via: String,
    /// The provider.
    pub provider: String,
    /// The provider's id of the account (`sub`).
    pub subject: String,
    /// The workspace of the row.
    pub workspace: Uuid,
}

/// Rows unnamed since before `cutoff`, by account (in `account_id` order,
/// the order the sweep locks them in), then way in and workspace.
pub async fn unnamed_before(
    conn: &mut PgConnection,
    cutoff: DateTime<Utc>,
) -> Result<Vec<Unnamed>> {
    let rows: Vec<(Uuid, Uuid, String, String, String, Uuid)> = sqlx::query_as(
        "SELECT p.id, p.account_id, p.linked_via, p.provider, p.subject, l.in_workspace_id
           FROM identity.mailbox_links l
           JOIN identity.provider_identities p ON p.id = l.provider_identity_id
          WHERE l.connection_id IS NULL AND l.created_at < $1
          ORDER BY p.account_id, p.id, l.in_workspace_id",
    )
    .bind(cutoff)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(way_in, account, linked_via, provider, subject, workspace)| Unnamed {
                way_in,
                account,
                linked_via,
                provider,
                subject,
                workspace,
            },
        )
        .collect())
}

/// Settle `account`'s stale rows, each with the connection the connectors
/// module says it became (`None`: there is none). Locks the account first.
/// A row whose connection exists is named; one with none is deleted, if it
/// is still unnamed and still older than `cutoff` (a retried connect writes
/// it afresh); then each connect-made way in left with no row goes, unless
/// it is the account's last. What was removed.
pub async fn settle_unnamed(
    conn: &mut PgConnection,
    account: Uuid,
    checked: &[(Unnamed, Option<String>)],
    cutoff: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<Removed>> {
    let mut count = ways_in::lock_and_count(conn, account).await?;
    for (row, connection) in checked.iter().filter(|(row, _)| row.account == account) {
        match connection {
            Some(connection) => {
                sqlx::query(
                    "UPDATE identity.mailbox_links SET connection_id = $3
                      WHERE provider_identity_id = $1 AND in_workspace_id = $2
                        AND connection_id IS NULL",
                )
                .bind(row.way_in)
                .bind(row.workspace)
                .bind(connection)
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query(
                    "DELETE FROM identity.mailbox_links
                      WHERE provider_identity_id = $1 AND in_workspace_id = $2
                        AND connection_id IS NULL AND created_at < $3",
                )
                .bind(row.way_in)
                .bind(row.workspace)
                .bind(cutoff)
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    let mut ways: Vec<(Uuid, &str)> = checked
        .iter()
        .filter(|(row, _)| row.account == account)
        .map(|(row, _)| (row.way_in, row.linked_via.as_str()))
        .collect();
    ways.dedup();
    let mut removed = Vec::new();
    for (way_in, linked_via) in ways {
        if linked_via != "connect" || count <= 1 || has_rows(conn, way_in).await? {
            continue;
        }
        if let Some(gone) = remove(conn, way_in, account, now).await? {
            count -= 1;
            removed.push(gone);
        }
    }
    Ok(removed)
}

/// Whether way in `identity_id` still has a recorded mailbox.
async fn has_rows(conn: &mut PgConnection, identity_id: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM identity.mailbox_links WHERE provider_identity_id = $1)",
    )
    .bind(identity_id)
    .fetch_one(conn)
    .await?)
}

/// Remove way in `way_in` of `account` (its account already locked): end
/// its sessions and revoke their refresh tokens, then delete it.
async fn remove(
    conn: &mut PgConnection,
    way_in: Uuid,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Removed>> {
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
