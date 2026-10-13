//! Accounts and their provider ways in.
//!
//! A provider identity is keyed by provider, issuer and subject, and belongs
//! to at most one account. It is linked only by the round trip that created
//! the account (slice S1a) or one completed while signed in (S2); an email
//! address is never used to find or link an account.

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

/// The accounts an operator may mean by `email`: those whose primary
/// address, or the address of one of whose ways in, is `email` (compared
/// lower-cased) and verified. For the server's CLI only
/// (`campbooks-server admin grant --email`), which refuses anything but
/// exactly one match; never used to sign in or to link a way in.
pub async fn operator_lookup(conn: &mut PgConnection, email: &str) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT a.id FROM identity.accounts a
          WHERE lower(a.primary_email) = lower($1) AND a.primary_email_verified
         UNION
         SELECT p.account_id FROM identity.provider_identities p
          WHERE lower(p.email_at_link) = lower($1) AND p.email_verified_at_link
         ORDER BY 1",
    )
    .bind(email.trim())
    .fetch_all(conn)
    .await?)
}

/// The display name and primary address of account `id` (disabled or not),
/// for the server's CLI to show whom a profile belongs to.
pub async fn describe(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<(Option<String>, Option<String>)>> {
    Ok(
        sqlx::query_as("SELECT display_name, primary_email FROM identity.accounts WHERE id = $1")
            .bind(id)
            .fetch_optional(conn)
            .await?,
    )
}
