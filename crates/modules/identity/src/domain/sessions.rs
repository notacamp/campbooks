//! The issuer's browser sessions: who is signed in to the issuer in this
//! browser, so every client's authorization finds them without asking
//! again (single sign-on). The cookie holds a random token; the table keeps
//! its SHA-256.

use campbooks_kernel::{
    Result,
    crypto::{hash, random_token},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// A session unused this long ends.
pub const IDLE: TimeDelta = TimeDelta::days(14);
/// A session ends this long after sign-in, used or not.
pub const ABSOLUTE: TimeDelta = TimeDelta::days(30);
/// Record use at most this often.
const TOUCH_EVERY: TimeDelta = TimeDelta::minutes(1);
/// Longest stored `User-Agent`.
const MAX_USER_AGENT_CHARS: usize = 256;

/// A live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct Session {
    /// The session.
    pub id: Uuid,
    /// Who is signed in.
    pub account_id: Uuid,
    /// When they authenticated.
    pub auth_time: DateTime<Utc>,
    /// When it was last used.
    pub last_seen_at: DateTime<Utc>,
}

/// A started session and its cookie token.
#[derive(Debug)]
pub struct Started {
    /// The session.
    pub session: Session,
    /// The cookie value.
    pub token: String,
}

/// Start a session for `account_id`, signed in with provider identity
/// `identity_id`.
pub async fn start(
    conn: &mut PgConnection,
    account_id: Uuid,
    identity_id: Uuid,
    user_agent: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Started> {
    let token = random_token();
    let id = Uuid::now_v7();
    let user_agent = user_agent.map(|ua| ua.chars().take(MAX_USER_AGENT_CHARS).collect::<String>());
    sqlx::query(
        "INSERT INTO identity.sessions
             (id, token_hash, account_id, method, provider_identity_id, auth_time, user_agent,
              created_at, last_seen_at, idle_expires_at, expires_at)
         VALUES ($1, $2, $3, 'provider', $4, $5, $6, $5, $5, $7, $8)",
    )
    .bind(id)
    .bind(hash(&token))
    .bind(account_id)
    .bind(identity_id)
    .bind(now)
    .bind(user_agent)
    .bind(now + IDLE)
    .bind(now + ABSOLUTE)
    .execute(conn)
    .await?;
    Ok(Started {
        session: Session {
            id,
            account_id,
            auth_time: now,
            last_seen_at: now,
        },
        token,
    })
}

/// Start a session for `account_id`, signed in with passkey `passkey_id`;
/// `amr` is what its tokens report (RFC 8176: `hwk` or `swk`, and `user`).
pub async fn start_with_passkey(
    conn: &mut PgConnection,
    account_id: Uuid,
    passkey_id: Uuid,
    amr: &[&str],
    user_agent: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Started> {
    let token = random_token();
    let id = Uuid::now_v7();
    let user_agent = user_agent.map(|ua| ua.chars().take(MAX_USER_AGENT_CHARS).collect::<String>());
    sqlx::query(
        "INSERT INTO identity.sessions
             (id, token_hash, account_id, method, passkey_id, amr, auth_time, user_agent,
              created_at, last_seen_at, idle_expires_at, expires_at)
         VALUES ($1, $2, $3, 'passkey', $4, $5, $6, $7, $6, $6, $8, $9)",
    )
    .bind(id)
    .bind(hash(&token))
    .bind(account_id)
    .bind(passkey_id)
    .bind(amr)
    .bind(now)
    .bind(user_agent)
    .bind(now + IDLE)
    .bind(now + ABSOLUTE)
    .execute(conn)
    .await?;
    Ok(Started {
        session: Session {
            id,
            account_id,
            auth_time: now,
            last_seen_at: now,
        },
        token,
    })
}

/// The authentication methods session `id` reports (`amr`); `fed` (a
/// provider) when the session is gone.
pub async fn amr(db: &PgPool, id: Option<Uuid>) -> Result<Vec<String>> {
    let Some(id) = id else {
        return Ok(vec!["fed".to_owned()]);
    };
    let amr: Option<Vec<String>> =
        sqlx::query_scalar("SELECT amr FROM identity.sessions WHERE id = $1")
            .bind(id)
            .fetch_optional(db)
            .await?;
    Ok(amr
        .filter(|amr| !amr.is_empty())
        .unwrap_or_else(|| vec!["fed".to_owned()]))
}

/// The live session `token` names, recording its use.
pub async fn current(db: &PgPool, token: &str, now: DateTime<Utc>) -> Result<Option<Session>> {
    let session: Option<Session> = sqlx::query_as(
        "SELECT s.id, s.account_id, s.auth_time, s.last_seen_at
           FROM identity.sessions s
           JOIN identity.accounts a ON a.id = s.account_id AND a.disabled_at IS NULL
          WHERE s.token_hash = $1 AND s.ended_at IS NULL
            AND s.idle_expires_at > $2 AND s.expires_at > $2",
    )
    .bind(hash(token))
    .bind(now)
    .fetch_optional(db)
    .await?;
    if let Some(session) = &session
        && now - session.last_seen_at >= TOUCH_EVERY
    {
        sqlx::query(
            "UPDATE identity.sessions SET last_seen_at = $2, idle_expires_at = $3 WHERE id = $1",
        )
        .bind(session.id)
        .bind(now)
        .bind(now + IDLE)
        .execute(db)
        .await?;
    }
    Ok(session)
}

/// End session `id` of `account` (nothing when it is someone else's) and
/// revoke the refresh tokens issued in it; whether it was live and ended
/// now.
pub async fn end_for(
    conn: &mut PgConnection,
    id: Uuid,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<bool> {
    let owned: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM identity.sessions WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(account)
            .fetch_optional(&mut *conn)
            .await?;
    if owned.is_none() {
        return Ok(false);
    }
    end(conn, id, now).await
}

/// End session `id` and revoke the refresh tokens issued in it; whether it
/// was live and ended now.
pub async fn end(conn: &mut PgConnection, id: Uuid, now: DateTime<Utc>) -> Result<bool> {
    let ended = sqlx::query(
        "UPDATE identity.sessions SET ended_at = $2 WHERE id = $1 AND ended_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = 'signed_out'
          WHERE session_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(ended == 1)
}

/// End every live session of `account` and revoke every refresh token it
/// holds, whatever client it went to (signed out everywhere). How many
/// sessions and refresh families ended.
pub async fn end_all(
    conn: &mut PgConnection,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<(u64, u64)> {
    let sessions = sqlx::query(
        "UPDATE identity.sessions SET ended_at = $2 WHERE account_id = $1 AND ended_at IS NULL",
    )
    .bind(account)
    .bind(now)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    let families = sqlx::query(
        "UPDATE identity.refresh_families
            SET revoked_at = $2, revoke_reason = 'signed_out_everywhere'
          WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account)
    .bind(now)
    .execute(conn)
    .await?
    .rows_affected();
    Ok((sessions, families))
}

/// End every live session signed in with provider identity `identity_id`
/// and revoke the refresh tokens issued in them (the way in is being
/// removed: whoever signed in with it is signed out). How many ended.
pub async fn end_signed_in_with(
    conn: &mut PgConnection,
    identity_id: Uuid,
    now: DateTime<Utc>,
) -> Result<usize> {
    let ended: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE identity.sessions SET ended_at = $2
          WHERE provider_identity_id = $1 AND ended_at IS NULL
         RETURNING id",
    )
    .bind(identity_id)
    .bind(now)
    .fetch_all(&mut *conn)
    .await?;
    revoke_families_of(conn, &ended, now).await?;
    Ok(ended.len())
}

/// End every live session signed in with passkey `passkey_id` and revoke
/// the refresh tokens issued in them (the passkey is being removed). How
/// many ended.
pub async fn end_signed_in_with_passkey(
    conn: &mut PgConnection,
    passkey_id: Uuid,
    now: DateTime<Utc>,
) -> Result<usize> {
    let ended: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE identity.sessions SET ended_at = $2
          WHERE passkey_id = $1 AND ended_at IS NULL
         RETURNING id",
    )
    .bind(passkey_id)
    .bind(now)
    .fetch_all(&mut *conn)
    .await?;
    revoke_families_of(conn, &ended, now).await?;
    Ok(ended.len())
}

/// Revoke the refresh-token families issued in `sessions` (a way in was
/// removed).
async fn revoke_families_of(
    conn: &mut PgConnection,
    sessions: &[Uuid],
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = 'way_in_removed'
          WHERE session_id = ANY($1) AND revoked_at IS NULL",
    )
    .bind(sessions)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}
