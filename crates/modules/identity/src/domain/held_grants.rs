//! Mailbox grants obtained at sign-up, held (sealed) until the person puts
//! them in a workspace, for at most [`HOLD_FOR`]. A grant never adopted is
//! deleted by `identity.prune`. It is not revoked at the provider: Google
//! revokes every token of an account's grant to the same app at once, which
//! would also cut a mailbox connected through that app.

use campbooks_kernel::{Error, Result, crypto::Cipher};
use chrono::{DateTime, TimeDelta, Utc};
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// How long a grant waits for a workspace.
pub const HOLD_FOR: TimeDelta = TimeDelta::days(7);

/// A provider's tokens, as kept sealed at rest.
#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderTokens {
    /// The access token.
    pub access_token: String,
    /// The refresh token.
    pub refresh_token: Option<String>,
    /// Seconds the access token lived when issued.
    pub expires_in: Option<u64>,
    /// The granted scopes.
    pub scopes: Vec<String>,
}

impl std::fmt::Debug for ProviderTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderTokens")
            .field("refresh_token", &self.refresh_token.is_some())
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

impl ProviderTokens {
    /// Seal for `table`'s row `id`.
    pub fn seal(&self, cipher: &Cipher, table: &str, id: Uuid) -> Result<Vec<u8>> {
        let json = SecretString::from(serde_json::to_string(self).map_err(Error::internal)?);
        Ok(cipher.seal(json.expose_secret(), &context(table, id)))
    }

    /// Open what [`seal`](Self::seal) sealed.
    pub fn open(cipher: &Cipher, sealed: &[u8], table: &str, id: Uuid) -> Result<Self> {
        let json = cipher
            .open(sealed, &context(table, id))
            .map_err(Error::internal)?;
        serde_json::from_str(&json).map_err(Error::internal)
    }
}

fn context(table: &str, id: Uuid) -> Vec<u8> {
    format!("identity.{table}:{id}:grant_tokens").into_bytes()
}

/// A held grant as the person sees it (never its tokens).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct HeldGrant {
    /// The grant.
    pub id: Uuid,
    /// `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The mailbox address.
    pub address: String,
    /// The provider's display name for it.
    pub display_name: Option<String>,
    /// When it is dropped if not adopted.
    pub expires_at: DateTime<Utc>,
}

/// What [`hold`] stores.
#[derive(Debug)]
pub struct NewHeldGrant<'a> {
    /// The account.
    pub account_id: Uuid,
    /// The provider identity it came with.
    pub identity_id: Uuid,
    /// The provider.
    pub provider: &'a str,
    /// The mailbox address.
    pub address: &'a str,
    /// Whether the provider verified it.
    pub address_verified: bool,
    /// A display name.
    pub display_name: Option<&'a str>,
    /// The tokens.
    pub tokens: &'a ProviderTokens,
}

/// Hold `grant` for its account.
pub async fn hold(
    conn: &mut PgConnection,
    cipher: &Cipher,
    grant: &NewHeldGrant<'_>,
    now: DateTime<Utc>,
) -> Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.held_grants
             (id, account_id, provider_identity_id, provider, address, address_verified,
              display_name, scopes, grant_tokens, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(grant.account_id)
    .bind(grant.identity_id)
    .bind(grant.provider)
    .bind(grant.address)
    .bind(grant.address_verified)
    .bind(grant.display_name)
    .bind(&grant.tokens.scopes)
    .bind(grant.tokens.seal(cipher, "held_grants", id)?)
    .bind(now)
    .bind(now + HOLD_FOR)
    .execute(conn)
    .await?;
    Ok(id)
}

/// The account's held grants that have not expired.
pub async fn list(db: &PgPool, account_id: Uuid, now: DateTime<Utc>) -> Result<Vec<HeldGrant>> {
    Ok(sqlx::query_as(
        "SELECT id, provider, address, display_name, expires_at
           FROM identity.held_grants
          WHERE account_id = $1 AND expires_at > $2
          ORDER BY created_at",
    )
    .bind(account_id)
    .bind(now)
    .fetch_all(db)
    .await?)
}

/// A held grant taken out of the store, with what the connection needs.
#[derive(Debug)]
pub struct Taken {
    /// The grant.
    pub grant: HeldGrant,
    /// The provider account's subject.
    pub account_subject: String,
    /// The issuer that vouched for it (the ID token's `iss`).
    pub issuer: String,
    /// Whether the provider verified the address.
    pub address_verified: bool,
    /// The tokens.
    pub tokens: ProviderTokens,
}

/// Lock and open held grant `id` of `account_id`; `None` when it is not
/// theirs, gone or expired. The caller deletes it with [`delete`] once
/// adopted.
pub async fn take(
    conn: &mut PgConnection,
    cipher: &Cipher,
    id: Uuid,
    account_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Taken>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        provider: String,
        address: String,
        address_verified: bool,
        display_name: Option<String>,
        expires_at: DateTime<Utc>,
        grant_tokens: Vec<u8>,
        subject: String,
        issuer: String,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT g.id, g.provider, g.address, g.address_verified, g.display_name, g.expires_at,
                g.grant_tokens,
                p.subject, p.issuer
           FROM identity.held_grants g
           JOIN identity.provider_identities p ON p.id = g.provider_identity_id
          WHERE g.id = $1 AND g.account_id = $2 AND g.expires_at > $3
          FOR UPDATE OF g",
    )
    .bind(id)
    .bind(account_id)
    .bind(now)
    .fetch_optional(conn)
    .await?;
    row.map(|row| {
        let tokens = ProviderTokens::open(cipher, &row.grant_tokens, "held_grants", row.id)?;
        Ok(Taken {
            grant: HeldGrant {
                id: row.id,
                provider: row.provider,
                address: row.address,
                display_name: row.display_name,
                expires_at: row.expires_at,
            },
            account_subject: row.subject,
            issuer: row.issuer,
            address_verified: row.address_verified,
            tokens,
        })
    })
    .transpose()
}

/// Delete held grant `id` of `account_id`; whether there was one.
pub async fn delete(conn: &mut PgConnection, id: Uuid, account_id: Uuid) -> Result<bool> {
    Ok(
        sqlx::query("DELETE FROM identity.held_grants WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(account_id)
            .execute(conn)
            .await?
            .rows_affected()
            == 1,
    )
}
