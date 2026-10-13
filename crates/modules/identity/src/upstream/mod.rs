//! Round trips to the providers people sign in with (or link, or connect
//! a mailbox with), and what is kept
//! between the start of one and its callback, or between the callback and
//! the person confirming a new account.
//!
//! A round trip is single use, expires after [`FLOW_LIFETIME`], and is bound
//! to the browser that started it by a cookie whose hash it stores. Its
//! PKCE verifier is sealed. A pending sign-up is bound the same way and
//! keeps the provider's tokens sealed until the account is created, or
//! until `identity.prune` deletes it.

pub mod client;

use campbooks_kernel::{
    Result,
    crypto::{Cipher, hash, random_token},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::domain::{accounts::ProviderIdentity, held_grants::ProviderTokens};

/// How long a round trip may take at the provider.
pub const FLOW_LIFETIME: TimeDelta = TimeDelta::minutes(10);
/// How long a pending new account waits for the person to confirm it.
pub const SIGN_UP_LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// The providers people sign in with, in the order the sign-in screen shows
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    /// Google (OIDC).
    Google,
    /// Microsoft: personal, work and school accounts (OIDC, multi-tenant).
    Microsoft,
    /// Zoho, in every data centre (OIDC).
    Zoho,
}

impl Provider {
    /// Every provider, in display order.
    pub const ALL: [Self; 3] = [Self::Google, Self::Microsoft, Self::Zoho];

    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Microsoft => "microsoft",
            Self::Zoho => "zoho",
        }
    }

    /// The provider named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == name)
    }
}

/// Why a round trip goes to the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Sign in, or sign up.
    SignIn,
    /// Add a way in to the signed-in account.
    Link,
    /// Connect a mailbox (and add it as a way in).
    Connect,
    /// Re-authorise a connected mailbox.
    Reconnect,
}

impl Purpose {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignIn => "sign_in",
            Self::Link => "link",
            Self::Connect => "connect",
            Self::Reconnect => "reconnect",
        }
    }

    /// The purpose named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::SignIn, Self::Link, Self::Connect, Self::Reconnect]
            .into_iter()
            .find(|p| p.as_str() == name)
    }

    /// Whether the round trip also brings a mailbox grant.
    pub const fn wants_mailbox(self) -> bool {
        matches!(self, Self::Connect | Self::Reconnect)
    }
}

/// The `redirect_uri` sent to `provider` (in the authorization request and
/// again in the code exchange, where the provider requires the two to match):
/// `issuer` (this instance's origin) plus `configured`, the provider's
/// `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH`, or the route Campbooks serves,
/// `/auth/callback/<provider>`, when it is unset.
///
/// A deployment whose OAuth app registered another callback path puts a
/// reverse-proxy rewrite from that path to `/auth/callback/<provider>` in
/// front of Campbooks and sets the setting to the registered path.
pub fn redirect_uri(
    provider: Provider,
    issuer: &str,
    configured: Option<&str>,
) -> Result<String, String> {
    let name = provider.as_str();
    let setting = format!("CAMPBOOKS_{}__REDIRECT_PATH", name.to_ascii_uppercase());
    let path = match configured.map(str::trim).filter(|p| !p.is_empty()) {
        None => format!("/auth/callback/{name}"),
        Some(path) => {
            check_redirect_path(path).map_err(|why| format!("{setting}={path}: {why}"))?;
            path.to_owned()
        }
    };
    Ok(format!("{}{path}", issuer.trim_end_matches('/')))
}

/// A redirect path is an absolute path: `/` first, no scheme, host, query,
/// fragment or dot segments.
fn check_redirect_path(path: &str) -> Result<(), &'static str> {
    const WHY: &str = "must be an absolute path such as /oauth/gmail/callback (no scheme, host, query or fragment)";
    if !path.starts_with('/') || path.starts_with("//") {
        return Err(WHY);
    }
    let base = url::Url::parse("https://campbooks.invalid").map_err(|_| WHY)?;
    let joined = base.join(path).map_err(|_| WHY)?;
    if joined.path() != path || joined.query().is_some() || joined.fragment().is_some() {
        return Err(WHY);
    }
    Ok(())
}

/// A started round trip.
#[derive(Debug)]
pub struct Flow {
    /// The provider.
    pub provider: Provider,
    /// Why.
    pub purpose: Purpose,
    /// The signed-in account a link, connect or reconnect is for.
    pub account_id: Option<Uuid>,
    /// The workspace a connect or reconnect is for.
    pub workspace_id: Option<Uuid>,
    /// The connection a reconnect re-authorises.
    pub connection_id: Option<String>,
    /// For a reconnect, the provider account (`sub`) that must come back.
    pub expected_subject: Option<String>,
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

impl Flow {
    /// A sign-in round trip for `provider`.
    pub fn sign_in(provider: Provider, request_id: Option<String>, return_to: String) -> Self {
        Self {
            provider,
            purpose: Purpose::SignIn,
            account_id: None,
            workspace_id: None,
            connection_id: None,
            expected_subject: None,
            request_id,
            return_to,
            nonce: random_token(),
            code_verifier: random_token(),
            consent_retry: false,
        }
    }
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
             (state_hash, browser_hash, provider, purpose, account_id, target_workspace_id,
              connection_id, expected_subject, request_id, return_to, nonce, code_verifier,
              consent_retry, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(&state_hash)
    .bind(hash(browser_token))
    .bind(flow.provider.as_str())
    .bind(flow.purpose.as_str())
    .bind(flow.account_id)
    .bind(flow.workspace_id)
    .bind(&flow.connection_id)
    .bind(&flow.expected_subject)
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
        purpose: String,
        account_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        connection_id: Option<String>,
        expected_subject: Option<String>,
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
         RETURNING browser_hash, provider, purpose, account_id,
                   target_workspace_id AS workspace_id, connection_id, expected_subject,
                   request_id, return_to, nonce, code_verifier, consent_retry, expires_at",
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
    let Some(purpose) = Purpose::parse(&row.purpose) else {
        return Ok(None);
    };
    Ok(Some(Flow {
        provider,
        purpose,
        account_id: row.account_id,
        workspace_id: row.workspace_id,
        connection_id: row.connection_id,
        expected_subject: row.expected_subject,
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
