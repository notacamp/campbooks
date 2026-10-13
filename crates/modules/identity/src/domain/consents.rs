//! A person's yes to a client: which scopes, in which workspace.
//!
//! At most one live consent exists per (account, client); a new one replaces
//! it. A **remembered** consent lets a later authorization skip the screen
//! when it asks for no more than was allowed and the client's redirect is
//! an exact `https` URI. Revoking a consent revokes every refresh token the
//! client holds for that person.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// A live consent.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Consent {
    /// The consent.
    pub id: Uuid,
    /// The client.
    pub client_id: String,
    /// The workspace its tokens are bound to.
    pub bound_workspace_id: Option<Uuid>,
    /// What was allowed.
    pub scopes: Vec<String>,
    /// Whether later authorizations may skip the screen.
    pub remembered: bool,
    /// The redirect URI it was given for (a remembered consent covers this
    /// one only).
    pub redirect_uri: String,
    /// Until when it is remembered.
    pub remember_until: Option<DateTime<Utc>>,
    /// When.
    pub granted_at: DateTime<Utc>,
    /// When a token was last minted under it.
    pub last_used_at: Option<DateTime<Utc>>,
}

const COLUMNS: &str = "id, client_id, bound_workspace_id, scopes, remembered, redirect_uri, \
     remember_until, granted_at, last_used_at";

/// How long a remembered consent lets authorizations skip the screen.
pub const REMEMBER_FOR: chrono::TimeDelta = chrono::TimeDelta::days(90);

/// What a person allowed.
#[derive(Debug)]
pub struct NewConsent<'a> {
    /// Who.
    pub account_id: Uuid,
    /// Which client.
    pub client_id: &'a str,
    /// In which workspace (`None`: Campbooks' own apps, all workspaces).
    pub workspace: Option<Uuid>,
    /// The workspaces module's id of that member.
    pub member: Option<Uuid>,
    /// What.
    pub scopes: &'a [String],
    /// Whether to skip the screen next time (for [`REMEMBER_FOR`]).
    pub remembered: bool,
    /// The redirect URI it is given for.
    pub redirect_uri: &'a str,
}

/// The live consent of `account` for `client`.
pub async fn live(db: &PgPool, account: Uuid, client: &str) -> Result<Option<Consent>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM identity.consents
          WHERE account_id = $1 AND client_id = $2 AND revoked_at IS NULL"
    )))
    .bind(account)
    .bind(client)
    .fetch_optional(db)
    .await?)
}

/// Record `consent`, replacing the live one of the same person and client.
/// The client's refresh tokens for that person that were bound to another
/// workspace, or granted more scopes than this consent allows, stop working
/// (those within it, such as the same app on another device, keep going).
/// Its id.
pub async fn grant(
    conn: &mut PgConnection,
    consent: &NewConsent<'_>,
    now: DateTime<Utc>,
) -> Result<Uuid> {
    sqlx::query(
        "UPDATE identity.consents SET revoked_at = $3
          WHERE account_id = $1 AND client_id = $2 AND revoked_at IS NULL",
    )
    .bind(consent.account_id)
    .bind(consent.client_id)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $5, revoke_reason = 'consent_replaced'
          WHERE account_id = $1 AND client_id = $2 AND revoked_at IS NULL
            AND (bound_workspace_id IS DISTINCT FROM $3
                 OR NOT (string_to_array(scope, ' ') <@ $4::text[]))",
    )
    .bind(consent.account_id)
    .bind(consent.client_id)
    .bind(consent.workspace)
    .bind(consent.scopes)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.consents
             (id, account_id, client_id, bound_workspace_id, bound_member_id, scopes, remembered,
              redirect_uri, remember_until, granted_at, last_used_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)",
    )
    .bind(id)
    .bind(consent.account_id)
    .bind(consent.client_id)
    .bind(consent.workspace)
    .bind(consent.member)
    .bind(consent.scopes)
    .bind(consent.remembered)
    .bind(consent.redirect_uri)
    .bind(consent.remembered.then_some(now + REMEMBER_FOR))
    .bind(now)
    .execute(conn)
    .await?;
    Ok(id)
}

/// Whether `account` still has a live consent for `client` bound to
/// `workspace` and covering every scope of `scope`: what a code or a refresh
/// token of a client that needs consent still stands on.
pub async fn covers(
    db: &PgPool,
    account: Uuid,
    client: &str,
    workspace: Option<Uuid>,
    scope: &str,
) -> Result<bool> {
    Ok(live(db, account, client).await?.is_some_and(|consent| {
        consent.bound_workspace_id == workspace
            && scope
                .split_whitespace()
                .all(|s| consent.scopes.iter().any(|c| c == s))
    }))
}

/// Record that a token was minted under `consent`.
pub async fn touch(db: &PgPool, consent: Uuid, now: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        "UPDATE identity.consents SET last_used_at = $2
          WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $2 - interval '1 minute')",
    )
    .bind(consent)
    .bind(now)
    .execute(db)
    .await?;
    Ok(())
}

/// Every live consent of `account`, newest first.
pub async fn of_account(db: &PgPool, account: Uuid) -> Result<Vec<Consent>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM identity.consents
          WHERE account_id = $1 AND revoked_at IS NULL ORDER BY granted_at DESC, id"
    )))
    .bind(account)
    .fetch_all(db)
    .await?)
}

/// What a revocation ended: the client and the workspace it was bound to.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Revoked {
    /// The account.
    pub account_id: Uuid,
    /// The client.
    pub client_id: String,
    /// The workspace.
    pub bound_workspace_id: Option<Uuid>,
}

/// Revoke consent `id` of `account` and the client's refresh tokens for
/// that person; `None` when there is no such live consent.
pub async fn revoke(
    conn: &mut PgConnection,
    id: Uuid,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Revoked>> {
    let revoked: Option<Revoked> = sqlx::query_as(
        "UPDATE identity.consents SET revoked_at = $3
          WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL
          RETURNING account_id, client_id, bound_workspace_id",
    )
    .bind(id)
    .bind(account)
    .bind(now)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(revoked) = &revoked {
        revoke_families(
            conn,
            revoked.account_id,
            &revoked.client_id,
            "consent_revoked",
            now,
        )
        .await?;
    }
    Ok(revoked)
}

/// Revoke every live consent bound to `workspace` (all of them, or only
/// those `member` granted), and the refresh tokens each client holds for
/// each person. What was revoked.
pub async fn revoke_in_workspace(
    conn: &mut PgConnection,
    workspace: Uuid,
    member: Option<Uuid>,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Revoked>> {
    let revoked: Vec<Revoked> = sqlx::query_as(
        "UPDATE identity.consents SET revoked_at = $1
          WHERE revoked_at IS NULL AND bound_workspace_id = $2
            AND ($3::uuid IS NULL OR bound_member_id = $3)
          RETURNING account_id, client_id, bound_workspace_id",
    )
    .bind(now)
    .bind(workspace)
    .bind(member)
    .fetch_all(&mut *conn)
    .await?;
    for consent in &revoked {
        revoke_families(conn, consent.account_id, &consent.client_id, reason, now).await?;
    }
    Ok(revoked)
}

/// Revoke every refresh family `client` holds for `account`.
pub async fn revoke_families(
    conn: &mut PgConnection,
    account: Uuid,
    client: &str,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $3, revoke_reason = $4
          WHERE account_id = $1 AND client_id = $2 AND revoked_at IS NULL",
    )
    .bind(account)
    .bind(client)
    .bind(now)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(())
}
