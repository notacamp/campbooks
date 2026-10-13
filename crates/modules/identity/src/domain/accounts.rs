//! Accounts and their provider ways in.
//!
//! A provider identity is keyed by provider, issuer and subject, and belongs
//! to at most one account. It is linked only by the round trip that created
//! the account or one completed while signed in ([`link`]); an email address
//! is never used to find or link an account.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// A provider identity as the provider described it after a round trip.
#[derive(Debug, Clone)]
pub struct ProviderIdentity {
    /// `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The ID token's `iss`.
    pub issuer: String,
    /// The ID token's `sub`.
    pub subject: String,
    /// The address the provider gave, if any.
    pub email: Option<String>,
    /// Whether the provider says it verified it.
    pub email_verified: bool,
    /// A display name.
    pub name: Option<String>,
}

/// The account a provider identity belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct Linked {
    /// The provider identity's row.
    pub identity_id: Uuid,
    /// Its account.
    pub account_id: Uuid,
    /// Whether the account is disabled.
    pub disabled: bool,
}

/// What tokens say about an account.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Profile {
    /// The account id.
    pub id: Uuid,
    /// Display name.
    pub display_name: Option<String>,
    /// Primary email address.
    pub primary_email: Option<String>,
    /// Whether a provider verified it.
    pub primary_email_verified: bool,
}

/// The account that `identity` is a way into, if any.
pub async fn find(db: &PgPool, identity: &ProviderIdentity) -> Result<Option<Linked>> {
    Ok(sqlx::query_as(
        "SELECT p.id AS identity_id, p.account_id, a.disabled_at IS NOT NULL AS disabled
           FROM identity.provider_identities p
           JOIN identity.accounts a ON a.id = p.account_id
          WHERE p.provider = $1 AND p.issuer = $2 AND p.subject = $3",
    )
    .bind(&identity.provider)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .fetch_optional(db)
    .await?)
}

/// Record that `identity` was used to sign in.
pub async fn touch(db: &PgPool, identity_id: Uuid, now: DateTime<Utc>) -> Result<()> {
    sqlx::query("UPDATE identity.provider_identities SET last_used_at = $2 WHERE id = $1")
        .bind(identity_id)
        .bind(now)
        .execute(db)
        .await?;
    Ok(())
}

/// Create an account whose first way in is `identity`. `None` when another
/// account took that identity in the meantime.
pub async fn create(
    conn: &mut PgConnection,
    identity: &ProviderIdentity,
    now: DateTime<Utc>,
) -> Result<Option<Linked>> {
    let account_id = Uuid::now_v7();
    let identity_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.accounts (id, display_name, primary_email, primary_email_verified, created_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(account_id)
    .bind(&identity.name)
    .bind(identity.email.as_deref().map(str::to_lowercase))
    .bind(identity.email_verified)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    let inserted = sqlx::query(
        "INSERT INTO identity.provider_identities
             (id, account_id, provider, issuer, subject, email_at_link, email_verified_at_link,
              linked_via, linked_at, last_used_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'sign_up', $8, $8)
         ON CONFLICT (provider, issuer, subject) DO NOTHING",
    )
    .bind(identity_id)
    .bind(account_id)
    .bind(&identity.provider)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .bind(&identity.email)
    .bind(identity.email_verified)
    .bind(now)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    Ok((inserted == 1).then_some(Linked {
        identity_id,
        account_id,
        disabled: false,
    }))
}

/// What linking a provider identity to an account did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    /// It is a new way into the account (its row).
    Added(Uuid),
    /// It already was one (its row).
    Already(Uuid),
    /// It is another account's way in: nothing changed.
    Another,
}

/// Link `identity` to `account` (`via` = `link` or `connect`), after a round
/// trip that account completed while signed in.
pub async fn link(
    conn: &mut PgConnection,
    account: Uuid,
    identity: &ProviderIdentity,
    via: &str,
    now: DateTime<Utc>,
) -> Result<LinkOutcome> {
    let id = Uuid::now_v7();
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO identity.provider_identities
             (id, account_id, provider, issuer, subject, email_at_link, email_verified_at_link,
              linked_via, linked_at, last_used_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL)
         ON CONFLICT (provider, issuer, subject) DO NOTHING
         RETURNING id",
    )
    .bind(id)
    .bind(account)
    .bind(&identity.provider)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .bind(&identity.email)
    .bind(identity.email_verified)
    .bind(via)
    .bind(now)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(id) = inserted {
        return Ok(LinkOutcome::Added(id));
    }
    let owner: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, account_id FROM identity.provider_identities
          WHERE provider = $1 AND issuer = $2 AND subject = $3",
    )
    .bind(&identity.provider)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .fetch_optional(conn)
    .await?;
    Ok(match owner {
        Some((id, owner)) if owner == account => LinkOutcome::Already(id),
        _ => LinkOutcome::Another,
    })
}

/// A provider way in, as its owner sees it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WayIn {
    /// The provider identity.
    pub id: Uuid,
    /// `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The address the provider gave when it was linked.
    pub email: Option<String>,
    /// `sign_up`, `link` or `connect`.
    pub linked_via: String,
    /// When.
    pub linked_at: DateTime<Utc>,
    /// When it was last used to sign in.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// The provider ways into `account`, oldest first.
pub async fn ways_in(db: &PgPool, account: Uuid) -> Result<Vec<WayIn>> {
    Ok(sqlx::query_as(
        "SELECT id, provider, email_at_link AS email, linked_via, linked_at, last_used_at
           FROM identity.provider_identities
          WHERE account_id = $1
          ORDER BY linked_at, id",
    )
    .bind(account)
    .fetch_all(db)
    .await?)
}

/// Delete provider way in `id` of `account`; its provider, or `None` when it
/// is not theirs.
pub async fn remove(conn: &mut PgConnection, id: Uuid, account: Uuid) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "DELETE FROM identity.provider_identities WHERE id = $1 AND account_id = $2
         RETURNING provider",
    )
    .bind(id)
    .bind(account)
    .fetch_optional(conn)
    .await?)
}

/// What tokens say about account `id`, unless it is gone or disabled.
pub async fn profile(db: &PgPool, id: Uuid) -> Result<Option<Profile>> {
    Ok(sqlx::query_as(
        "SELECT id, display_name, primary_email, primary_email_verified
           FROM identity.accounts WHERE id = $1 AND disabled_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

/// Record `kind` in the account's audit trail. `metadata` holds ids,
/// provider names and methods only.
pub async fn audit(
    conn: &mut PgConnection,
    account: Option<Uuid>,
    kind: &str,
    metadata: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO identity.audit_events (id, account_id, kind, at, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(account)
    .bind(kind)
    .bind(now)
    .bind(metadata)
    .execute(conn)
    .await?;
    Ok(())
}
