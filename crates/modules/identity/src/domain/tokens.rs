//! What the issuer hands out: authorization codes, refresh-token families
//! and signed access and ID tokens.
//!
//! - **Codes** live [`CODE_LIFETIME`], are single use, and are bound to the
//!   client, redirect URI and PKCE challenge. Redeeming one twice revokes the
//!   family the first redemption created (RFC 6749 §4.1.2).
//! - **Refresh tokens** are opaque, rotate on every use, and belong to a
//!   family (one sign-in). Presenting a rotated token again is reuse: the
//!   whole family is revoked (OAuth 2.1 §4.3.1).
//! - **Access tokens** are RFC 9068 JWTs (`typ: at+jwt`), **ID tokens** OIDC
//!   JWTs (`typ: JWT`); both `EdDSA`, both [`TOKEN_LIFETIME`].

use campbooks_kernel::{
    Error, Result,
    crypto::{hash, pkce_challenge, random_token},
};
use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{Algorithm, Header};
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{accounts::Profile, clients::Client, keys::Signer};

/// Lifetime of an authorization code.
pub const CODE_LIFETIME: TimeDelta = TimeDelta::seconds(60);
/// Lifetime of access and ID tokens.
pub const TOKEN_LIFETIME: TimeDelta = TimeDelta::minutes(10);
/// The scope that asks for a refresh token.
pub const OFFLINE_ACCESS: &str = "offline_access";

/// What a code or a refresh family stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The client it was issued to.
    pub client_id: String,
    /// The account.
    pub account_id: Uuid,
    /// The issuer session it was issued in.
    pub session_id: Option<Uuid>,
    /// Granted scopes, space-separated.
    pub scope: String,
    /// When the person authenticated.
    pub auth_time: DateTime<Utc>,
}

/// Why a code or refresh token was refused. All are `invalid_grant` on the
/// wire; [`Reused`](Self::Reused) also revoked a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Unknown, expired, revoked, or for another client or redirect URI, or
    /// the PKCE verifier does not match.
    Invalid,
    /// A code redeemed twice, or a rotated refresh token presented again.
    Reused {
        /// The account whose tokens were revoked.
        account_id: Uuid,
    },
}

/// A new code for `grant`, bound to `redirect_uri`, `code_challenge` and
/// `nonce`.
pub async fn create_code(
    conn: &mut PgConnection,
    grant: &Grant,
    redirect_uri: &str,
    code_challenge: &str,
    nonce: Option<&str>,
    now: DateTime<Utc>,
) -> Result<String> {
    let session_id = grant
        .session_id
        .ok_or_else(|| Error::internal("a code needs an issuer session"))?;
    let code = random_token();
    sqlx::query(
        "INSERT INTO identity.authorization_codes
             (code_hash, client_id, account_id, session_id, redirect_uri, scope, nonce,
              code_challenge, auth_time, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(hash(&code))
    .bind(&grant.client_id)
    .bind(grant.account_id)
    .bind(session_id)
    .bind(redirect_uri)
    .bind(&grant.scope)
    .bind(nonce)
    .bind(code_challenge)
    .bind(grant.auth_time)
    .bind(now + CODE_LIFETIME)
    .execute(conn)
    .await?;
    Ok(code)
}

#[derive(sqlx::FromRow)]
struct CodeRow {
    client_id: String,
    account_id: Uuid,
    session_id: Uuid,
    redirect_uri: String,
    scope: String,
    nonce: Option<String>,
    code_challenge: String,
    auth_time: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    redeemed_at: Option<DateTime<Utc>>,
    family_id: Option<Uuid>,
}

/// A redeemed code: its grant and the nonce of the sign-in.
#[derive(Debug)]
pub struct Redeemed {
    /// What the code stood for.
    pub grant: Grant,
    /// The `nonce` of the authorization request.
    pub nonce: Option<String>,
}

/// Redeem `code` for `client_id` at `redirect_uri` with the PKCE
/// `verifier`. On success the code is spent; the caller records the family
/// it creates with [`attach_family`].
pub async fn redeem_code(
    conn: &mut PgConnection,
    code: &str,
    client_id: &str,
    redirect_uri: &str,
    verifier: &str,
    now: DateTime<Utc>,
) -> Result<Result<Redeemed, Refusal>> {
    let row: Option<CodeRow> = sqlx::query_as(
        "SELECT client_id, account_id, session_id, redirect_uri, scope, nonce, code_challenge,
                auth_time, expires_at, redeemed_at, family_id
           FROM identity.authorization_codes WHERE code_hash = $1 FOR UPDATE",
    )
    .bind(hash(code))
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(Err(Refusal::Invalid));
    };
    if row.redeemed_at.is_some() {
        if let Some(family) = row.family_id {
            revoke_family(conn, family, "code_replayed", now).await?;
        }
        return Ok(Err(Refusal::Reused {
            account_id: row.account_id,
        }));
    }
    let verifier_ok = (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        && pkce_challenge(verifier) == row.code_challenge;
    if row.client_id != client_id
        || row.redirect_uri != redirect_uri
        || row.expires_at <= now
        || !verifier_ok
    {
        return Ok(Err(Refusal::Invalid));
    }
    sqlx::query("UPDATE identity.authorization_codes SET redeemed_at = $2 WHERE code_hash = $1")
        .bind(hash(code))
        .bind(now)
        .execute(&mut *conn)
        .await?;
    Ok(Ok(Redeemed {
        grant: Grant {
            client_id: row.client_id,
            account_id: row.account_id,
            session_id: Some(row.session_id),
            scope: row.scope,
            auth_time: row.auth_time,
        },
        nonce: row.nonce,
    }))
}

/// Remember that redeeming `code` created `family`, so a replay revokes it.
pub async fn attach_family(conn: &mut PgConnection, code: &str, family: Uuid) -> Result<()> {
    sqlx::query("UPDATE identity.authorization_codes SET family_id = $2 WHERE code_hash = $1")
        .bind(hash(code))
        .bind(family)
        .execute(conn)
        .await?;
    Ok(())
}

/// Start a refresh family for `grant` and return its id and first token.
pub async fn create_family(
    conn: &mut PgConnection,
    grant: &Grant,
    client: &Client,
    now: DateTime<Utc>,
) -> Result<(Uuid, String)> {
    let family = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.refresh_families
             (id, client_id, account_id, session_id, scope, auth_time, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(family)
    .bind(&grant.client_id)
    .bind(grant.account_id)
    .bind(grant.session_id)
    .bind(&grant.scope)
    .bind(grant.auth_time)
    .bind(now)
    .bind(now + client.refresh_absolute)
    .execute(&mut *conn)
    .await?;
    let token = insert_refresh_token(conn, family, client, now).await?;
    Ok((family, token))
}

async fn insert_refresh_token(
    conn: &mut PgConnection,
    family: Uuid,
    client: &Client,
    now: DateTime<Utc>,
) -> Result<String> {
    let token = random_token();
    sqlx::query(
        "INSERT INTO identity.refresh_tokens (token_hash, family_id, issued_at, idle_expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(hash(&token))
    .bind(family)
    .bind(now)
    .bind(now + client.refresh_idle)
    .execute(conn)
    .await?;
    Ok(token)
}

#[derive(sqlx::FromRow)]
struct RefreshRow {
    family_id: Uuid,
    idle_expires_at: DateTime<Utc>,
    rotated_at: Option<DateTime<Utc>>,
    client_id: String,
    account_id: Uuid,
    session_id: Option<Uuid>,
    scope: String,
    auth_time: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    disabled: bool,
}

/// Exchange refresh token `token` of `client` for its successor. A rotated
/// token presented again revokes its family.
pub async fn rotate(
    conn: &mut PgConnection,
    token: &str,
    client: &Client,
    now: DateTime<Utc>,
) -> Result<Result<(Grant, String), Refusal>> {
    let row: Option<RefreshRow> = sqlx::query_as(
        "SELECT t.family_id, t.idle_expires_at, t.rotated_at,
                f.client_id, f.account_id, f.session_id, f.scope, f.auth_time, f.expires_at,
                f.revoked_at, a.disabled_at IS NOT NULL AS disabled
           FROM identity.refresh_tokens t
           JOIN identity.refresh_families f ON f.id = t.family_id
           JOIN identity.accounts a ON a.id = f.account_id
          WHERE t.token_hash = $1
          FOR UPDATE OF t, f",
    )
    .bind(hash(token))
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(Err(Refusal::Invalid));
    };
    if row.client_id != client.id {
        return Ok(Err(Refusal::Invalid));
    }
    if row.rotated_at.is_some() {
        if row.revoked_at.is_none() {
            revoke_family(conn, row.family_id, "refresh_reused", now).await?;
        }
        return Ok(Err(Refusal::Reused {
            account_id: row.account_id,
        }));
    }
    if row.revoked_at.is_some()
        || row.disabled
        || row.idle_expires_at <= now
        || row.expires_at <= now
    {
        return Ok(Err(Refusal::Invalid));
    }
    sqlx::query("UPDATE identity.refresh_tokens SET rotated_at = $2 WHERE token_hash = $1")
        .bind(hash(token))
        .bind(now)
        .execute(&mut *conn)
        .await?;
    let next = insert_refresh_token(conn, row.family_id, client, now).await?;
    Ok(Ok((
        Grant {
            client_id: row.client_id,
            account_id: row.account_id,
            session_id: row.session_id,
            scope: row.scope,
            auth_time: row.auth_time,
        },
        next,
    )))
}

/// Revoke the family refresh token `token` of `client` belongs to. Unknown
/// tokens and other clients' tokens are ignored (RFC 7009 §2.2).
pub async fn revoke_token(
    conn: &mut PgConnection,
    token: &str,
    client_id: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let family: Option<Uuid> = sqlx::query_scalar(
        "SELECT f.id FROM identity.refresh_tokens t
           JOIN identity.refresh_families f ON f.id = t.family_id
          WHERE t.token_hash = $1 AND f.client_id = $2",
    )
    .bind(hash(token))
    .bind(client_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(family) = family {
        revoke_family(conn, family, "revoked", now).await?;
    }
    Ok(())
}

/// Revoke every token of `family`.
pub async fn revoke_family(
    conn: &mut PgConnection,
    family: Uuid,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = $3
          WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(family)
    .bind(now)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(Serialize)]
struct AccessClaims<'a> {
    iss: &'a str,
    sub: String,
    aud: &'a str,
    azp: &'a str,
    client_id: &'a str,
    scope: &'a str,
    iat: i64,
    nbf: i64,
    exp: i64,
    jti: String,
    auth_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    email_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

#[derive(Serialize)]
struct IdClaims<'a> {
    iss: &'a str,
    sub: String,
    aud: &'a str,
    azp: &'a str,
    iat: i64,
    exp: i64,
    auth_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sid: Option<String>,
    amr: [&'static str; 1],
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    email_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

/// Who the tokens are minted for, and by whom.
pub struct Mint<'a> {
    /// The issuer identifier.
    pub issuer: &'a str,
    /// The access tokens' audience.
    pub audience: &'a str,
    /// The key that signs.
    pub signer: &'a Signer,
    /// What was granted.
    pub grant: &'a Grant,
    /// The account's profile.
    pub profile: &'a Profile,
    /// The time.
    pub now: DateTime<Utc>,
}

impl Mint<'_> {
    fn sign(&self, typ: &str, claims: &impl Serialize) -> Result<String> {
        let mut header = Header::new(Algorithm::EdDSA);
        header.typ = Some(typ.to_owned());
        header.kid = Some(self.signer.kid.clone());
        jsonwebtoken::encode(&header, claims, &self.signer.key)
            .map_err(|e| Error::internal(format!("signing a token: {e}")))
    }

    /// The access token (RFC 9068). First-party clients get the person's
    /// email and name.
    pub fn access_token(&self) -> Result<String> {
        let now = self.now.timestamp();
        let scopes: Vec<&str> = self.grant.scope.split_whitespace().collect();
        let wants = |scope: &str| scopes.contains(&scope);
        self.sign(
            "at+jwt",
            &AccessClaims {
                iss: self.issuer,
                sub: self.grant.account_id.to_string(),
                aud: self.audience,
                azp: &self.grant.client_id,
                client_id: &self.grant.client_id,
                scope: &self.grant.scope,
                iat: now,
                nbf: now,
                exp: now + TOKEN_LIFETIME.num_seconds(),
                jti: random_token(),
                auth_time: self.grant.auth_time.timestamp(),
                sid: self.grant.session_id.map(|id| id.to_string()),
                email: wants("email")
                    .then_some(self.profile.primary_email.as_deref())
                    .flatten(),
                email_verified: wants("email") && self.profile.primary_email_verified,
                name: wants("profile")
                    .then_some(self.profile.display_name.as_deref())
                    .flatten(),
            },
        )
    }

    /// The ID token for a sign-in that sent `nonce`.
    pub fn id_token(&self, nonce: Option<&str>) -> Result<String> {
        let now = self.now.timestamp();
        let scopes: Vec<&str> = self.grant.scope.split_whitespace().collect();
        let wants = |scope: &str| scopes.contains(&scope);
        self.sign(
            "JWT",
            &IdClaims {
                iss: self.issuer,
                sub: self.grant.account_id.to_string(),
                aud: &self.grant.client_id,
                azp: &self.grant.client_id,
                iat: now,
                exp: now + TOKEN_LIFETIME.num_seconds(),
                auth_time: self.grant.auth_time.timestamp(),
                nonce,
                sid: self.grant.session_id.map(|id| id.to_string()),
                amr: ["fed"],
                email: wants("email")
                    .then_some(self.profile.primary_email.as_deref())
                    .flatten(),
                email_verified: wants("email") && self.profile.primary_email_verified,
                name: wants("profile")
                    .then_some(self.profile.display_name.as_deref())
                    .flatten(),
            },
        )
    }
}
