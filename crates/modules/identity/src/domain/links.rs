//! Link tickets: what `identity.start_link` hands the browser so it can start
//! a signed-in round trip (add a way in, connect a mailbox, reconnect one).
//!
//! A ticket is a random 256-bit value stored as its SHA-256, usable once,
//! for [`TICKET_LIFETIME`]. The API call that issues it is authenticated and
//! scoped; the browser that redeems it must be signed in to the issuer as the
//! same account. So neither someone else's ticket in your browser nor your
//! ticket in someone else's browser starts anything (login CSRF on linking).

use campbooks_kernel::{
    Result,
    crypto::{hash, random_token},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::upstream::{Provider, Purpose};

/// How long a ticket waits for the browser.
pub const TICKET_LIFETIME: TimeDelta = TimeDelta::minutes(2);

/// What a ticket starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// The signed-in account.
    pub account_id: Uuid,
    /// The provider.
    pub provider: Provider,
    /// `Link`, `Connect` or `Reconnect`.
    pub purpose: Purpose,
    /// The workspace of a connect or reconnect.
    pub workspace_id: Option<Uuid>,
    /// The connection a reconnect re-authorises.
    pub connection_id: Option<String>,
    /// For a reconnect, the provider account (`sub`) that must come back.
    pub expected_subject: Option<String>,
    /// The account to suggest at the provider.
    pub login_hint: Option<String>,
    /// Same-origin path to come back to.
    pub return_to: String,
    /// Whether a connect also adds the account as a way in.
    pub adds_way_in: bool,
}

/// Store `ticket`; its value and when it expires.
pub async fn issue(
    db: &PgPool,
    ticket: &Ticket,
    now: DateTime<Utc>,
) -> Result<(String, DateTime<Utc>)> {
    let token = random_token();
    let expires_at = now + TICKET_LIFETIME;
    sqlx::query(
        "INSERT INTO identity.link_tickets
             (ticket_hash, account_id, provider, purpose, target_workspace_id, connection_id,
              expected_subject, login_hint, return_to, created_at, expires_at, adds_way_in)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(hash(&token))
    .bind(ticket.account_id)
    .bind(ticket.provider.as_str())
    .bind(ticket.purpose.as_str())
    .bind(ticket.workspace_id)
    .bind(&ticket.connection_id)
    .bind(&ticket.expected_subject)
    .bind(&ticket.login_hint)
    .bind(&ticket.return_to)
    .bind(now)
    .bind(expires_at)
    .bind(ticket.adds_way_in)
    .execute(db)
    .await?;
    Ok((token, expires_at))
}

/// Take (delete and return) ticket `token`, if it has not expired. A ticket
/// is spent whether or not what follows succeeds.
pub async fn take(db: &PgPool, token: &str, now: DateTime<Utc>) -> Result<Option<Ticket>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        account_id: Uuid,
        provider: String,
        purpose: String,
        workspace_id: Option<Uuid>,
        connection_id: Option<String>,
        expected_subject: Option<String>,
        login_hint: Option<String>,
        return_to: String,
        expires_at: DateTime<Utc>,
        adds_way_in: bool,
    }
    let row: Option<Row> = sqlx::query_as(
        "DELETE FROM identity.link_tickets WHERE ticket_hash = $1
         RETURNING account_id, provider, purpose, target_workspace_id AS workspace_id, connection_id,
                   expected_subject, login_hint,
                   return_to, expires_at, adds_way_in",
    )
    .bind(hash(token))
    .fetch_optional(db)
    .await?;
    Ok(row.filter(|row| row.expires_at > now).and_then(|row| {
        Some(Ticket {
            account_id: row.account_id,
            provider: Provider::parse(&row.provider)?,
            purpose: Purpose::parse(&row.purpose)?,
            workspace_id: row.workspace_id,
            connection_id: row.connection_id,
            expected_subject: row.expected_subject,
            login_hint: row.login_hint,
            return_to: row.return_to,
            adds_way_in: row.adds_way_in,
        })
    }))
}
