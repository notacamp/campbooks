//! Round trips to the providers people sign in with, and what is kept
//! between the start of one and its callback, or between the callback and
//! the person confirming a new account.
//!
//! A round trip is single use, expires after [`FLOW_LIFETIME`], and is bound
//! to the browser that started it by a cookie whose hash it stores. Its
//! PKCE verifier is sealed. A pending sign-up is bound the same way and
//! keeps the provider's tokens sealed until the account is created, or
//! until `identity.prune` deletes it.

pub mod google;

use campbooks_kernel::{
    Result,
    crypto::{Cipher, hash},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::domain::{accounts::ProviderIdentity, held_grants::ProviderTokens};

/// How long a round trip may take at the provider.
pub const FLOW_LIFETIME: TimeDelta = TimeDelta::minutes(10);
/// How long a pending new account waits for the person to confirm it.
pub const SIGN_UP_LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// The providers people sign in with. Slice S1a has Google; Microsoft and
/// Zoho arrive with S2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Google (OIDC).
    Google,
}

impl Provider {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
        }
    }

    /// The provider named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "google" => Some(Self::Google),
            _ => None,
        }
    }
}

/// A started round trip.
#[derive(Debug)]
pub struct Flow {
    /// The provider.
    pub provider: Provider,
    /// The `/oauth2/authorize` request to complete afterwards.
    pub request_id: Option<String>,
    /// Same-origin path to land on when there is no request to complete.
    pub return_to: String,
    /// The OIDC `nonce` sent.
    pub nonce: String,
    /// The PKCE verifier.
    pub code_verifier: String,
    /// Whether this round trip already asked for consent again.
    pub consent_retry: bool,
}

fn verifier_context(state_hash: &[u8]) -> Vec<u8> {
    let mut context = b"identity.upstream_flows:code_verifier:".to_vec();
    context.extend_from_slice(state_hash);
    context
}

/// Store `flow`, identified by `state` and bound to the browser holding
/// `browser_token`.
pub async fn store_flow(
    db: &PgPool,
    cipher: &Cipher,
    state: &str,
    browser_token: &str,
    flow: &Flow,
    now: DateTime<Utc>,
) -> Result<()> {
    let state_hash = hash(state);
    sqlx::query(
        "INSERT INTO identity.upstream_flows
             (state_hash, browser_hash, provider, purpose, request_id, return_to, nonce,
              code_verifier, consent_retry, created_at, expires_at)
         VALUES ($1, $2, $3, 'sign_in', $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&state_hash)
    .bind(hash(browser_token))
    .bind(flow.provider.as_str())
    .bind(&flow.request_id)
    .bind(&flow.return_to)
    .bind(&flow.nonce)
    .bind(cipher.seal(&flow.code_verifier, &verifier_context(&state_hash)))
    .bind(flow.consent_retry)
    .bind(now)
    .bind(now + FLOW_LIFETIME)
    .execute(db)
    .await?;
    Ok(())
}

/// Take (delete and return) the round trip `state` names, if the browser
/// holding `browser_token` started it for `provider` and it has not expired.
pub async fn take_flow(
    db: &PgPool,
    cipher: &Cipher,
    state: &str,
    browser_token: Option<&str>,
    provider: Provider,
    now: DateTime<Utc>,
) -> Result<Option<Flow>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        browser_hash: Vec<u8>,
        provider: String,
        request_id: Option<String>,
        return_to: String,
        nonce: String,
        code_verifier: Vec<u8>,
        consent_retry: bool,
        expires_at: DateTime<Utc>,
    }
    let state_hash = hash(state);
    // Deleted whether or not it matches: a state is tried once.
    let row: Option<Row> = sqlx::query_as(
        "DELETE FROM identity.upstream_flows WHERE state_hash = $1
         RETURNING browser_hash, provider, request_id, return_to, nonce, code_verifier,
                   consent_retry, expires_at",
    )
    .bind(&state_hash)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let same_browser = browser_token.is_some_and(|token| {
        aws_lc_rs::constant_time::verify_slices_are_equal(&hash(token), &row.browser_hash).is_ok()
    });
    if !same_browser || row.provider != provider.as_str() || row.expires_at <= now {
        return Ok(None);
    }
    let Ok(code_verifier) = cipher.open(&row.code_verifier, &verifier_context(&state_hash)) else {
        return Ok(None);
    };
    Ok(Some(Flow {
        provider,
        request_id: row.request_id,
        return_to: row.return_to,
        nonce: row.nonce,
        code_verifier,
        consent_retry: row.consent_retry,
    }))
}

/// A provider identity no account uses yet, waiting for "Create account".
#[derive(Debug)]
pub struct PendingSignUp {
    /// The row.
    pub id: Uuid,
    /// Who the provider says they are.
    pub identity: ProviderIdentity,
    /// The provider's tokens, when it granted any worth keeping.
    pub tokens: Option<ProviderTokens>,
    /// The authorize request to complete afterwards.
    pub request_id: Option<String>,
    /// Where to land otherwise.
    pub return_to: String,
}

/// Store a pending sign-up bound to the browser holding `browser_token`.
pub async fn store_sign_up(
    db: &PgPool,
    cipher: &Cipher,
    browser_token: &str,
    pending: &PendingSignUp,
    now: DateTime<Utc>,
) -> Result<()> {
    let sealed = pending
        .tokens
        .as_ref()
        .map(|tokens| tokens.seal(cipher, "pending_sign_ups", pending.id))
        .transpose()?;
    sqlx::query(
        "INSERT INTO identity.pending_sign_ups
             (id, token_hash, provider, issuer, subject, email, email_verified, display_name,
              grant_tokens, request_id, return_to, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(pending.id)
    .bind(hash(browser_token))
    .bind(&pending.identity.provider)
    .bind(&pending.identity.issuer)
    .bind(&pending.identity.subject)
    .bind(&pending.identity.email)
    .bind(pending.identity.email_verified)
    .bind(&pending.identity.name)
    .bind(sealed)
    .bind(&pending.request_id)
    .bind(&pending.return_to)
    .bind(now)
    .bind(now + SIGN_UP_LIFETIME)
    .execute(db)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct SignUpRow {
    id: Uuid,
    provider: String,
    issuer: String,
    subject: String,
    email: Option<String>,
    email_verified: bool,
    display_name: Option<String>,
    grant_tokens: Option<Vec<u8>>,
    request_id: Option<String>,
    return_to: String,
}

impl SignUpRow {
    fn into_pending(self, cipher: &Cipher) -> Result<PendingSignUp> {
        let tokens = self
            .grant_tokens
            .as_deref()
            .map(|sealed| ProviderTokens::open(cipher, sealed, "pending_sign_ups", self.id))
            .transpose()?;
        Ok(PendingSignUp {
            id: self.id,
            identity: ProviderIdentity {
                provider: self.provider,
                issuer: self.issuer,
                subject: self.subject,
                email: self.email,
                email_verified: self.email_verified,
                name: self.display_name,
            },
            tokens,
            request_id: self.request_id,
            return_to: self.return_to,
        })
    }
}

/// What the new-account screen shows for the browser holding
/// `browser_token`: the provider and the address, never a token.
pub async fn peek_sign_up(
    db: &PgPool,
    browser_token: &str,
    now: DateTime<Utc>,
) -> Result<Option<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query_as(
        "SELECT provider, email, display_name FROM identity.pending_sign_ups
          WHERE token_hash = $1 AND expires_at > $2",
    )
    .bind(hash(browser_token))
    .bind(now)
    .fetch_optional(db)
    .await?)
}

/// Take (delete and return) the pending sign-up of the browser holding
/// `browser_token`.
pub async fn take_sign_up(
    conn: &mut PgConnection,
    cipher: &Cipher,
    browser_token: &str,
    now: DateTime<Utc>,
) -> Result<Option<PendingSignUp>> {
    let row: Option<SignUpRow> = sqlx::query_as(
        "DELETE FROM identity.pending_sign_ups WHERE token_hash = $1 AND expires_at > $2
         RETURNING id, provider, issuer, subject, email, email_verified, display_name,
                   grant_tokens, request_id, return_to",
    )
    .bind(hash(browser_token))
    .bind(now)
    .fetch_optional(conn)
    .await?;
    row.map(|row| row.into_pending(cipher)).transpose()
}
