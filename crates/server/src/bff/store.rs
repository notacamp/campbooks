//! The BFF's tables in schema `bff`: sessions, their retired tokens and
//! started sign-ins. Token columns hold ciphertexts (see `crypto`); this
//! module never sees plaintext tokens.
//!
//! The runtime role holds no privilege on those tables (campbooks-cloud#212):
//! every statement here calls one of the `bff.*` functions, which run as
//! `campbooks_definer`, a role that cannot log in and holds only what they
//! need (campbooks-cloud#214). Each acts on the one session a cookie's token
//! hash names (`presented`), the one sign-in a state hash names, or one account's
//! sessions; only the two prunes reach across accounts, deleting ended rows
//! a bounded batch at a time.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Rows a prune deletes per call (the functions accept at most 10 000).
const PRUNE_BATCH: i32 = 5_000;

/// A session to create.
pub struct NewSession<'a> {
    pub id: Uuid,
    pub token_hash: &'a [u8],
    pub subject: &'a str,
    pub idp_session_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub email: Option<&'a str>,
    pub access_token: &'a [u8],
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: Option<&'a [u8]>,
    pub id_token: &'a [u8],
    pub user_agent: Option<&'a str>,
    pub now: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// The scopes its sign-in asked for (space-separated), to tell later
    /// whether the BFF has started asking for more; `None` for a sign-in
    /// started before they were recorded.
    pub scopes: Option<&'a str>,
}

/// A live session, found by its cookie.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    /// The SHA-256 of the token the request's cookie carried: what every
    /// later call about this session names it by.
    #[sqlx(skip)]
    pub presented: Vec<u8>,
    /// The cookie carried the current token (not the rotated-away one).
    #[sqlx(rename = "is_current")]
    pub current: bool,
    pub rotated_at: DateTime<Utc>,
    /// The current token has been presented at least once since it was
    /// issued.
    pub token_delivered: bool,
    pub subject: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub access_token: Vec<u8>,
    pub access_expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// The scopes it asked for at sign-in; `None` for a session from before
    /// they were recorded.
    pub scopes: Option<String>,
}

/// A session's tokens, read under a row lock for refreshing.
#[derive(sqlx::FromRow)]
pub struct LockedTokens {
    pub access_token: Vec<u8>,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: Option<Vec<u8>>,
    pub id_token: Option<Vec<u8>>,
}

/// An ended session: its id and its sealed refresh token (if any) for
/// revoking, and its sealed ID token for ending the identity provider's
/// session.
#[derive(sqlx::FromRow)]
pub struct Ended {
    pub id: Uuid,
    pub refresh_token: Option<Vec<u8>>,
    pub id_token: Vec<u8>,
}

/// A started sign-in.
#[derive(sqlx::FromRow)]
pub struct LoginTransaction {
    pub state_hash: Vec<u8>,
    pub browser_hash: Vec<u8>,
    /// The nonce sent to the IdP, sealed like the verifier.
    pub nonce: Vec<u8>,
    pub code_verifier: Vec<u8>,
    pub return_to: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// The scopes it asked the identity provider for (space-separated).
    pub scopes: Option<String>,
}

/// Store a new session, in the transaction that tracks its start.
pub async fn insert_session(db: &mut PgConnection, s: &NewSession<'_>) -> sqlx::Result<()> {
    sqlx::query(
        "SELECT bff.create_session($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
         $15)",
    )
    .bind(s.id)
    .bind(s.token_hash)
    .bind(s.subject)
    .bind(s.idp_session_id)
    .bind(s.name)
    .bind(s.email)
    .bind(s.access_token)
    .bind(s.access_expires_at)
    .bind(s.refresh_token)
    .bind(s.id_token)
    .bind(s.user_agent)
    .bind(s.now)
    .bind(s.idle_expires_at)
    .bind(s.expires_at)
    .bind(s.scopes)
    .execute(db)
    .await?;
    Ok(())
}

/// The live session whose current token, or whose previous token, hashes
/// to `hash`. [`Session::current`] says which; the caller decides whether a
/// previous token is still within its grace (or a replay).
pub async fn find_session(db: &PgPool, hash: &[u8]) -> sqlx::Result<Option<Session>> {
    let session: Option<Session> = sqlx::query_as(
        "SELECT id, is_current, rotated_at, token_delivered, subject, name, email, access_token,
                access_expires_at, last_seen_at, expires_at, scopes
           FROM bff.find_session($1)",
    )
    .bind(hash)
    .fetch_optional(db)
    .await?;
    Ok(session.map(|session| Session {
        presented: hash.to_vec(),
        ..session
    }))
}

/// Record use of the session `hash` names: push its idle deadline to
/// `idle_expires_at` (never past its absolute one).
pub async fn touch(
    db: &PgPool,
    hash: &[u8],
    now: DateTime<Utc>,
    idle_expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query("SELECT bff.touch_session($1, $2, $3)")
        .bind(hash)
        .bind(now)
        .bind(idle_expires_at)
        .execute(db)
        .await?;
    Ok(())
}

/// Replace the session's current token `old_hash` by `new_hash`, keeping
/// the old one for the grace period and retiring the one before it (kept
/// until the session ends, to recognise a copy). `false` when another
/// request rotated it first.
pub async fn rotate(
    db: &PgPool,
    old_hash: &[u8],
    new_hash: &[u8],
    now: DateTime<Utc>,
) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT bff.rotate_session($1, $2, $3)")
        .bind(old_hash)
        .bind(new_hash)
        .bind(now)
        .fetch_one(db)
        .await
}

/// Record that the session's current token `hash` reached the browser.
pub async fn delivered(db: &PgPool, hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query("SELECT bff.mark_session_delivered($1)")
        .bind(hash)
        .execute(db)
        .await?;
    Ok(())
}

/// What [`reissue`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reissue {
    /// The new token replaced the undelivered one.
    Reissued,
    /// Another request presenting the same previous token reissued first:
    /// this one is within the new grace.
    WithinGrace,
    /// The current token was presented meanwhile: the previous one coming
    /// back now is a copy.
    Delivered,
    /// The previous token is no longer the session's previous one (a
    /// rotation moved on, or the session ended).
    Gone,
}

/// The browser still presents the previous token `previous_hash` because
/// the current one never reached it: issue `new_hash` in its place, once the
/// previous token is past its grace (rotated at or before `grace_since`).
/// The undelivered token is retired (presented later, it is a copy) and the
/// previous one starts a new grace (`rotated_at` = `now`), so concurrent
/// requests with it reissue only once.
pub async fn reissue(
    db: &PgPool,
    previous_hash: &[u8],
    new_hash: &[u8],
    now: DateTime<Utc>,
    grace_since: DateTime<Utc>,
) -> sqlx::Result<Reissue> {
    let outcome: String = sqlx::query_scalar("SELECT bff.reissue_session($1, $2, $3, $4)")
        .bind(previous_hash)
        .bind(new_hash)
        .bind(now)
        .bind(grace_since)
        .fetch_one(db)
        .await?;
    Ok(match outcome.as_str() {
        "reissued" => Reissue::Reissued,
        "within_grace" => Reissue::WithinGrace,
        "delivered" => Reissue::Delivered,
        _ => Reissue::Gone,
    })
}

/// Whether `hash` is a retired token (one rotated away before the previous
/// one) of a live session.
pub async fn is_retired(db: &PgPool, hash: &[u8]) -> sqlx::Result<bool> {
    let session: Option<Uuid> = sqlx::query_scalar("SELECT bff.retired_session($1)")
        .bind(hash)
        .fetch_one(db)
        .await?;
    Ok(session.is_some())
}

/// Begin a transaction on the BFF's tables: the one a token refresh locks
/// its session row in ([`lock_tokens`]), or the one a session starts or ends
/// in, with its tracked event (ADR 0020). Sessions are instance-wide (no
/// workspace), so it sets none; it is opened here, the one BFF file allowed
/// to open transactions on the pool (`architecture.toml`).
pub async fn begin(db: &PgPool) -> sqlx::Result<Transaction<'static, Postgres>> {
    db.begin().await
}

/// The tokens of the session `hash` names, locked until `tx` ends; `None`
/// if it is gone.
pub async fn lock_tokens(
    tx: &mut Transaction<'_, Postgres>,
    hash: &[u8],
) -> sqlx::Result<Option<LockedTokens>> {
    sqlx::query_as(
        "SELECT access_token, access_expires_at, refresh_token, id_token
           FROM bff.lock_session_tokens($1)",
    )
    .bind(hash)
    .fetch_optional(&mut **tx)
    .await
}

/// Sealed tokens a refresh stores.
pub struct Refreshed<'a> {
    /// The new access token.
    pub access_token: &'a [u8],
    /// When it expires.
    pub access_expires_at: DateTime<Utc>,
    /// The refresh token to keep.
    pub refresh_token: Option<&'a [u8]>,
    /// The ID token to keep; `None` keeps the stored one.
    pub id_token: Option<&'a [u8]>,
}

/// Store refreshed tokens.
pub async fn update_tokens(
    tx: &mut Transaction<'_, Postgres>,
    hash: &[u8],
    tokens: &Refreshed<'_>,
) -> sqlx::Result<()> {
    sqlx::query("SELECT bff.store_session_tokens($1, $2, $3, $4, $5)")
        .bind(hash)
        .bind(tokens.access_token)
        .bind(tokens.access_expires_at)
        .bind(tokens.refresh_token)
        .bind(tokens.id_token)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Delete the session that holds or held the token `hash` (current,
/// previous or retired), with its retired tokens.
pub async fn delete_session(db: &mut PgConnection, hash: &[u8]) -> sqlx::Result<Option<Ended>> {
    sqlx::query_as("SELECT id, refresh_token, id_token FROM bff.end_session($1)")
        .bind(hash)
        .fetch_optional(db)
        .await
}

/// Back-channel logout: delete the sessions of IdP session `sid` (of user
/// `subject` when both are given), or every session of `subject`; how many.
pub async fn delete_for_logout(
    db: &mut PgConnection,
    subject: Option<&str>,
    sid: Option<&str>,
) -> sqlx::Result<u64> {
    let deleted: i64 = sqlx::query_scalar("SELECT bff.end_account_sessions($1, $2)")
        .bind(subject)
        .bind(sid)
        .fetch_one(db)
        .await?;
    Ok(deleted.unsigned_abs())
}

pub async fn insert_login(db: &PgPool, login: &LoginTransaction) -> sqlx::Result<()> {
    sqlx::query("SELECT bff.start_login($1, $2, $3, $4, $5, $6, $7, $8)")
        .bind(&login.state_hash)
        .bind(&login.browser_hash)
        .bind(&login.nonce)
        .bind(&login.code_verifier)
        .bind(&login.return_to)
        .bind(login.created_at)
        .bind(login.expires_at)
        .bind(&login.scopes)
        .execute(db)
        .await?;
    Ok(())
}

/// Redeem the sign-in whose state hashes to `state_hash`: it is deleted
/// whatever happens next, so a state works once.
pub async fn take_login(db: &PgPool, state_hash: &[u8]) -> sqlx::Result<Option<LoginTransaction>> {
    sqlx::query_as(
        "SELECT state_hash, browser_hash, nonce, code_verifier, return_to, created_at, expires_at,
                scopes
           FROM bff.take_login($1)",
    )
    .bind(state_hash)
    .fetch_optional(db)
    .await
}

/// Delete ended sessions and expired sign-ins, a batch per call; how many
/// of each. "Ended" is the database's clock: no caller can prune a live
/// session.
pub async fn prune(db: &PgPool) -> sqlx::Result<(u64, u64)> {
    let sessions = drain(db, "SELECT bff.prune_sessions($1)").await?;
    let logins = drain(db, "SELECT bff.prune_logins($1)").await?;
    Ok((sessions, logins))
}

/// Call a prune function until a batch comes back short; the total.
async fn drain(db: &PgPool, sql: &'static str) -> sqlx::Result<u64> {
    let mut total = 0_u64;
    loop {
        let deleted: i64 = sqlx::query_scalar(sql)
            .bind(PRUNE_BATCH)
            .fetch_one(db)
            .await?;
        let deleted = deleted.unsigned_abs();
        total += deleted;
        if deleted < u64::from(PRUNE_BATCH.unsigned_abs()) {
            return Ok(total);
        }
    }
}
