//! Checking an IMAP mailbox's address (campbooks-cloud#314): a password
//! login proves the username, not the address typed beside it, so a mailbox
//! whose username is not its address is sent a one-time code, from and to
//! the address, through its own SMTP server (`send_address_check`), and its
//! stream's runs look for it in that connection's Inbox (`imap::check`).
//! Found, the address is verified and the mailbox may send.
//!
//! One row per connection (`connectors.address_checks`): `sending` until the
//! message went out, `waiting` while the runs look for it, then `verified`
//! or `failed`. The row keeps the address it checks: a check proves that
//! address only, and ends as soon as the connection's address is another
//! ([`abandon`], and [`verified`] compares them in its transaction). The
//! code is never stored: it is derived from the instance key, the
//! connection, the row's nonce and the address ([`code`]), and travels only
//! in the check message's `Message-ID`. A check whose message went out more
//! than [`WINDOW`] ago is over, whether or not a run looked for it.

use campbooks_kernel::{
    Result, crypto::Signer, i18n::Locale, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{
    model::{AddressCheck, ConnectionId},
    schedule::{self, Rate},
};
use crate::features::{
    imap_idle::lease,
    send_address_check::{SendAddressCheck, SendAddressCheckPayload},
};

/// Checks each connection may have a day (the connect's and the retries').
pub const CHECKS_PER_DAY: u32 = 5;

/// How long after the message was sent the runs look for it.
pub const WINDOW: TimeDelta = TimeDelta::minutes(15);

/// The most runs that look for it.
pub const MAX_SEARCHES: i32 = 30;

/// A check still sending or waiting after this long went with its job (its
/// attempts spent, a process lost): it shows as failed and can be retried.
pub const STALE_MINUTES: i32 = 30;

/// The prefix of the code in the check message's `Message-ID`.
pub const PREFIX: &str = "cb-check-";

/// An address as checks compare it: trimmed and lower-cased (the database
/// side uses `lower(btrim(…))`; addresses are stored lower-cased).
pub fn normalized(address: &str) -> String {
    address.trim().to_lowercase()
}

/// The code of a check of `connection` with `nonce` for `address`: 128
/// bits of the instance's keyed digest, as 32 hex characters. Only the
/// instance can make it; the nonce alone proves nothing, and a code made
/// for one address is never another's.
pub fn code(signer: &Signer, connection: ConnectionId, nonce: Uuid, address: &str) -> String {
    lease::keyed(
        signer,
        b"connectors.address_check",
        &format!("{connection}:{nonce}:{}", normalized(address)),
    )
}

/// The `Message-ID` of the check message carrying `code`, at the address's
/// `domain`.
pub fn message_id(code: &str, domain: &str) -> String {
    format!("<{PREFIX}{code}@{domain}>")
}

/// Take one of `connection`'s daily checks: whether one was left.
pub async fn take_budget(pool: &PgPool, connection: ConnectionId) -> Result<bool> {
    Ok(schedule::take(
        pool,
        &format!("address_check:{connection}"),
        1,
        Rate::per_day(CHECKS_PER_DAY),
    )
    .await?
    .is_ok())
}

/// Start a check of `connection` in `tx` (its workspace's), of the address
/// the connection has now: a new nonce, written in `locale`, `sending`, and
/// its message's job enqueued. A check already under way is replaced (its
/// message, if it went, never counts).
pub async fn start(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    connection: ConnectionId,
    locale: Locale,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO connectors.address_checks
             (connection_id, workspace_id, state, nonce, locale, address, updated_at)
         SELECT c.id, $2, 'sending', $3, $4, lower(btrim(c.address)), $5
           FROM connectors.connections c WHERE c.id = $1
         ON CONFLICT (connection_id) DO UPDATE
            SET state = 'sending', reason = NULL, nonce = $3, locale = $4, searches = 0,
                address = EXCLUDED.address, sent_at = NULL, updated_at = $5",
    )
    .bind(connection)
    .bind(workspace)
    .bind(Uuid::now_v7())
    .bind(locale.tag())
    .bind(now)
    .execute(&mut **tx)
    .await?;
    let key = format!("address_check:{connection}");
    campbooks_kernel::jobs::enqueue_to::<SendAddressCheck>(
        tx,
        &SendAddressCheckPayload { connection },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(key.clone())
            .ordered_by(key),
    )
    .await?;
    Ok(())
}

/// A check whose message is still to be sent: its nonce, locale and the
/// address it checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sending {
    pub nonce: Uuid,
    pub locale: Locale,
    pub address: String,
}

/// `connection`'s check, when its message is still to be sent.
pub async fn sending(conn: &mut PgConnection, connection: ConnectionId) -> Result<Option<Sending>> {
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT nonce, locale, address FROM connectors.address_checks
          WHERE connection_id = $1 AND state = 'sending'",
    )
    .bind(connection)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(nonce, locale, address)| Sending {
        nonce,
        locale: locale.parse().unwrap_or_default(),
        address,
    }))
}

/// A check whose message went out, while the runs look for it, and the
/// connection's address (still the one it checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub nonce: Uuid,
    pub sent_at: DateTime<Utc>,
    pub address: String,
}

/// `connection`'s check while its code may still be looked for: waiting,
/// its message sent less than [`WINDOW`] before `now`, for the address the
/// connection has now. One past the window ends `failed` (`not_found`)
/// here, whether or not a run looked for it.
pub async fn waiting(
    conn: &mut PgConnection,
    connection: ConnectionId,
    now: DateTime<Utc>,
) -> Result<Option<Waiting>> {
    sqlx::query(
        "UPDATE connectors.address_checks
            SET state = 'failed', reason = 'not_found', updated_at = $2
          WHERE connection_id = $1 AND state = 'waiting'
            AND (sent_at IS NULL OR sent_at <= $3)",
    )
    .bind(connection)
    .bind(now)
    .bind(now - WINDOW)
    .execute(&mut *conn)
    .await?;
    let row: Option<(Uuid, Option<DateTime<Utc>>, String)> = sqlx::query_as(
        "SELECT a.nonce, a.sent_at, c.address
           FROM connectors.address_checks a
           JOIN connectors.connections c ON c.id = a.connection_id
          WHERE a.connection_id = $1 AND a.state = 'waiting' AND a.sent_at > $2
            AND a.address = lower(btrim(c.address))",
    )
    .bind(connection)
    .bind(now - WINDOW)
    .fetch_optional(conn)
    .await?;
    Ok(row.and_then(|(nonce, sent_at, address)| {
        Some(Waiting {
            nonce,
            sent_at: sent_at?,
            address,
        })
    }))
}

/// End every check of `connection` still under way (`address_changed`):
/// its address was changed, or is no longer verified. In the transaction
/// that changes it, whatever the daily budget.
pub async fn abandon(
    conn: &mut PgConnection,
    connection: ConnectionId,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE connectors.address_checks
            SET state = 'failed', reason = 'address_changed', updated_at = $2
          WHERE connection_id = $1 AND state IN ('sending', 'waiting')",
    )
    .bind(connection)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// The message of check `nonce` went out (or may have): look for it now.
/// Whether it was still `sending` (and is now `waiting`).
pub async fn sent(
    conn: &mut PgConnection,
    connection: ConnectionId,
    nonce: Uuid,
    now: DateTime<Utc>,
) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE connectors.address_checks
            SET state = 'waiting', sent_at = $3, searches = 0, updated_at = $3
          WHERE connection_id = $1 AND nonce = $2 AND state = 'sending'",
    )
    .bind(connection)
    .bind(nonce)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Check `nonce` ended without finding its code, for `reason` (a stable
/// code): whether it was still under way (and so ended now).
pub async fn failed(
    conn: &mut PgConnection,
    connection: ConnectionId,
    nonce: Uuid,
    reason: &'static str,
    now: DateTime<Utc>,
) -> Result<bool> {
    let ended = sqlx::query(
        "UPDATE connectors.address_checks
            SET state = 'failed', reason = $3, updated_at = $4
          WHERE connection_id = $1 AND nonce = $2 AND state IN ('sending', 'waiting')",
    )
    .bind(connection)
    .bind(nonce)
    .bind(reason)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(ended.rows_affected() == 1)
}

/// The connection's address is verified (by check `nonce`, or already):
/// the check ends `verified`, in the same transaction as the connection,
/// only while the connection's address is still the one the check checked.
pub async fn verified(
    conn: &mut PgConnection,
    connection: ConnectionId,
    nonce: Uuid,
    now: DateTime<Utc>,
) -> Result<bool> {
    let ended = sqlx::query(
        "UPDATE connectors.address_checks a
            SET state = 'verified', reason = NULL, updated_at = $3
           FROM connectors.connections c
          WHERE a.connection_id = $1 AND a.nonce = $2 AND a.state IN ('sending', 'waiting')
            AND c.id = a.connection_id AND c.provider = 'imap'
            AND a.address = lower(btrim(c.address))",
    )
    .bind(connection)
    .bind(nonce)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    if ended.rows_affected() != 1 {
        return Ok(false);
    }
    let checked = normalized(&check_address(&mut *conn, connection).await?);
    sqlx::query(
        "UPDATE connectors.connections SET address_verified = true, updated_at = $2
          WHERE id = $1 AND provider = 'imap' AND lower(btrim(address)) = $3",
    )
    .bind(connection)
    .bind(now)
    .bind(checked)
    .execute(conn)
    .await?;
    Ok(true)
}

/// The address connection `connection`'s check row checks (empty when none).
async fn check_address(conn: &mut PgConnection, connection: ConnectionId) -> Result<String> {
    let address: Option<String> = sqlx::query_scalar(
        "SELECT address FROM connectors.address_checks WHERE connection_id = $1",
    )
    .bind(connection)
    .fetch_optional(conn)
    .await?;
    Ok(address.unwrap_or_default())
}

/// What a run's look for check `nonce`'s code found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looked {
    /// The code, in the connection's Inbox.
    Found,
    /// Not there (yet).
    NotFound,
}

/// Record, in a transaction of `workspace`, what a run found for `check`:
/// found within [`WINDOW`] of the send, the address is verified; found
/// later, or not found, one search more, and the check fails past the
/// window or [`MAX_SEARCHES`].
pub async fn record(
    pool: &PgPool,
    workspace: WorkspaceId,
    connection: ConnectionId,
    check: &Waiting,
    looked: Looked,
    now: DateTime<Utc>,
) -> Result<()> {
    use campbooks_kernel::db::BeginFor as _;
    let over = now >= check.sent_at + WINDOW;
    let mut tx = pool.begin_for(workspace).await?;
    match looked {
        Looked::Found if !over => {
            if verified(&mut tx, connection, check.nonce, now).await? {
                tracing::info!(%workspace, %connection, "an IMAP mailbox's address was verified");
            }
        }
        Looked::Found | Looked::NotFound => {
            sqlx::query(
                "UPDATE connectors.address_checks
                    SET searches = searches + 1,
                        state = CASE WHEN $4 OR searches + 1 >= $5 THEN 'failed' ELSE state END,
                        reason = CASE WHEN $4 OR searches + 1 >= $5 THEN 'not_found' ELSE reason END,
                        updated_at = $3
                  WHERE connection_id = $1 AND nonce = $2 AND state = 'waiting'",
            )
            .bind(connection)
            .bind(check.nonce)
            .bind(now)
            .bind(over)
            .bind(MAX_SEARCHES)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Where connection `connection`'s address check is, as the API shows it;
/// none when `verified` or never checked. A check still under way after
/// [`STALE_MINUTES`] (by the database's clock) shows as failed.
pub async fn view(
    conn: &mut PgConnection,
    connection: ConnectionId,
    verified: bool,
) -> Result<Option<AddressCheck>> {
    if verified {
        return Ok(None);
    }
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT state, updated_at < now() - make_interval(mins => $2)
           FROM connectors.address_checks WHERE connection_id = $1",
    )
    .bind(connection)
    .bind(STALE_MINUTES)
    .fetch_optional(conn)
    .await?;
    Ok(row.and_then(|(state, stale)| match state.as_str() {
        "sending" | "waiting" if !stale => Some(AddressCheck::Checking),
        "sending" | "waiting" | "failed" => Some(AddressCheck::Failed),
        _ => None,
    }))
}
