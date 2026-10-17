//! The BFF's tables in schema `bff`: sessions and started sign-ins. Token
//! columns hold ciphertexts (see `crypto`); this module never sees plaintext
//! tokens.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

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
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    /// The cookie carried the current token (not the rotated-away one).
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
pub struct LockedTokens {
    pub access_token: Vec<u8>,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: Option<Vec<u8>>,
}

/// A started sign-in.
pub struct LoginTransaction {
    pub state_hash: Vec<u8>,
    pub browser_hash: Vec<u8>,
    pub nonce: String,
    pub code_verifier: Vec<u8>,
    pub return_to: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// The scopes it asked the identity provider for (space-separated).
    pub scopes: Option<String>,
}

pub async fn insert_session(db: &PgPool, s: &NewSession<'_>) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO bff.sessions
            (id, token_hash, rotated_at, subject, idp_session_id, name, email,
             access_token, access_expires_at, refresh_token, id_token, user_agent,
             created_at, last_seen_at, idle_expires_at, expires_at, scopes)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $3, $3, $13, $14, $15)
        "#,
        s.id,
        s.token_hash,
        s.now,
        s.subject,
        s.idp_session_id,
        s.name,
        s.email,
        s.access_token,
        s.access_expires_at,
        s.refresh_token,
        s.id_token,
        s.user_agent,
        s.idle_expires_at,
        s.expires_at,
        s.scopes,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The live session whose current token, or whose previous token, hashes
/// to `hash`. [`Session::current`] says which; the caller decides whether a
/// previous token is still within its grace (or a replay).
pub async fn find_session(
    db: &PgPool,
    hash: &[u8],
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Session>> {
    sqlx::query_as!(
        Session,
        r#"
        SELECT id, token_hash = $1 AS "current!", rotated_at, token_delivered, subject, name, email,
               access_token, access_expires_at, last_seen_at, expires_at, scopes
          FROM bff.sessions
         WHERE (token_hash = $1 OR previous_token_hash = $1)
           AND idle_expires_at > $2
           AND expires_at > $2
        "#,
        hash,
        now,
    )
    .fetch_optional(db)
    .await
}

/// Record use: push the idle deadline to `idle_expires_at`.
pub async fn touch(
    db: &PgPool,
    id: Uuid,
    now: DateTime<Utc>,
    idle_expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE bff.sessions SET last_seen_at = $2, idle_expires_at = $3 WHERE id = $1",
        id,
        now,
        idle_expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Replace the session's token `old_hash` by `new_hash`, keeping the old one
/// for the grace period and retiring the one before it (kept until the
/// session ends, to recognise a copy). `false` when another request rotated
/// it first.
pub async fn rotate(
    db: &PgPool,
    id: Uuid,
    old_hash: &[u8],
    new_hash: &[u8],
    now: DateTime<Utc>,
) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let Some(previous) = sqlx::query_scalar!(
        "SELECT previous_token_hash FROM bff.sessions WHERE id = $1 AND token_hash = $2 FOR UPDATE",
        id,
        old_hash,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };
    // The token before the previous one leaves the grace for good: kept, so
    // a copy of it presented later is recognised.
    if let Some(previous) = previous {
        sqlx::query!(
            "INSERT INTO bff.retired_tokens (token_hash, session_id, retired_at)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            previous,
            id,
            now,
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        r#"
        UPDATE bff.sessions
           SET previous_token_hash = token_hash, token_hash = $3, rotated_at = $4,
               token_delivered = false
         WHERE id = $1 AND token_hash = $2
        "#,
        id,
        old_hash,
        new_hash,
        now,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Record that the session's current token `hash` reached the browser.
pub async fn delivered(db: &PgPool, id: Uuid, hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE bff.sessions SET token_delivered = true WHERE id = $1 AND token_hash = $2",
        id,
        hash,
    )
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
    id: Uuid,
    previous_hash: &[u8],
    new_hash: &[u8],
    now: DateTime<Utc>,
    grace_since: DateTime<Utc>,
) -> sqlx::Result<Reissue> {
    let mut tx = db.begin().await?;
    let Some(row) = sqlx::query!(
        "SELECT token_hash, token_delivered, rotated_at FROM bff.sessions
          WHERE id = $1 AND previous_token_hash = $2
          FOR UPDATE",
        id,
        previous_hash,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(Reissue::Gone);
    };
    if row.token_delivered {
        return Ok(Reissue::Delivered);
    }
    if row.rotated_at > grace_since {
        return Ok(Reissue::WithinGrace);
    }
    sqlx::query!(
        "INSERT INTO bff.retired_tokens (token_hash, session_id, retired_at)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        row.token_hash,
        id,
        now,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE bff.sessions SET token_hash = $2, rotated_at = $3 WHERE id = $1",
        id,
        new_hash,
        now,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Reissue::Reissued)
}

/// The live session a retired token (one rotated away before the previous
/// one) belonged to.
pub async fn retired_session(
    db: &PgPool,
    hash: &[u8],
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        r#"
        SELECT s.id
          FROM bff.retired_tokens r
          JOIN bff.sessions s ON s.id = r.session_id
         WHERE r.token_hash = $1 AND s.idle_expires_at > $2 AND s.expires_at > $2
        "#,
        hash,
        now,
    )
    .fetch_optional(db)
    .await
}

/// Begin the transaction a token refresh locks its session row in
/// ([`lock_tokens`]). Sessions are instance-wide (no workspace), so it sets
/// none; it is opened here, the one BFF file allowed to open transactions on
/// the pool (`architecture.toml`).
pub async fn begin_refresh(db: &PgPool) -> sqlx::Result<Transaction<'static, Postgres>> {
    db.begin().await
}

/// [`delete_session`] on its own, outside any transaction.
pub async fn end_session(
    db: &PgPool,
    id: Uuid,
) -> sqlx::Result<Option<(Option<Vec<u8>>, Vec<u8>)>> {
    delete_session(&mut *db.acquire().await?, id).await
}

/// The session's tokens, locked until `tx` ends; `None` if it is gone.
pub async fn lock_tokens(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> sqlx::Result<Option<LockedTokens>> {
    sqlx::query_as!(
        LockedTokens,
        r#"
        SELECT access_token, access_expires_at, refresh_token
          FROM bff.sessions WHERE id = $1 FOR UPDATE
        "#,
        id,
    )
    .fetch_optional(&mut **tx)
    .await
}

/// Store refreshed tokens.
pub async fn update_tokens(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    access_token: &[u8],
    access_expires_at: DateTime<Utc>,
    refresh_token: Option<&[u8]>,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE bff.sessions
           SET access_token = $2, access_expires_at = $3, refresh_token = $4
         WHERE id = $1
        "#,
        id,
        access_token,
        access_expires_at,
        refresh_token,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Delete one session; its sealed refresh token (if any) for revoking and
/// its sealed ID token for ending the identity provider's session.
pub async fn delete_session(
    db: &mut PgConnection,
    id: Uuid,
) -> sqlx::Result<Option<(Option<Vec<u8>>, Vec<u8>)>> {
    let row = sqlx::query!(
        "DELETE FROM bff.sessions WHERE id = $1 RETURNING refresh_token, id_token",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|row| (row.refresh_token, row.id_token)))
}

/// Delete the session whose current token hashes to `hash`, if any.
pub async fn delete_by_token(db: &PgPool, hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM bff.sessions WHERE token_hash = $1", hash)
        .execute(db)
        .await?;
    Ok(())
}

/// Back-channel logout: delete the sessions of IdP session `sid` (of user
/// `subject` when both are given), or every session of `subject`.
pub async fn delete_for_logout(
    db: &PgPool,
    subject: Option<&str>,
    sid: Option<&str>,
) -> sqlx::Result<u64> {
    let deleted = sqlx::query!(
        r#"
        DELETE FROM bff.sessions
         WHERE ($2::text IS NOT NULL AND idp_session_id = $2
                AND ($1::text IS NULL OR subject = $1))
            OR ($2::text IS NULL AND $1::text IS NOT NULL AND subject = $1)
        "#,
        subject,
        sid,
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(deleted)
}

pub async fn insert_login(db: &PgPool, login: &LoginTransaction) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO bff.login_transactions
            (state_hash, browser_hash, nonce, code_verifier, return_to, created_at, expires_at,
             scopes)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        login.state_hash,
        login.browser_hash,
        login.nonce,
        login.code_verifier,
        login.return_to,
        login.created_at,
        login.expires_at,
        login.scopes,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Redeem the sign-in whose state hashes to `state_hash`: it is deleted
/// whatever happens next, so a state works once.
pub async fn take_login(db: &PgPool, state_hash: &[u8]) -> sqlx::Result<Option<LoginTransaction>> {
    sqlx::query_as!(
        LoginTransaction,
        r#"
        DELETE FROM bff.login_transactions WHERE state_hash = $1
        RETURNING state_hash, browser_hash, nonce, code_verifier, return_to, created_at, expires_at,
                  scopes
        "#,
        state_hash,
    )
    .fetch_optional(db)
    .await
}

/// Delete ended sessions and expired sign-ins; how many of each.
pub async fn prune(db: &PgPool, now: DateTime<Utc>) -> sqlx::Result<(u64, u64)> {
    let sessions = sqlx::query!(
        "DELETE FROM bff.sessions WHERE least(idle_expires_at, expires_at) <= $1",
        now
    )
    .execute(db)
    .await?
    .rows_affected();
    let logins = sqlx::query!(
        "DELETE FROM bff.login_transactions WHERE expires_at <= $1",
        now
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok((sessions, logins))
}
