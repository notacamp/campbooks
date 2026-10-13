//! What `campbooks-server admin grant|revoke|list` does, on a connection of
//! the **migration login**: the runtime role cannot write profiles, so these
//! fail on any connection a request could hold. Every change is recorded in
//! `admin.actions` with `via = 'cli'` and the database login as the actor.
//!
//! Nothing else writes `admin.profiles`; no HTTP endpoint calls these.

use campbooks_kernel::{Result, admin::Permission};
use chrono::{DateTime, Utc};
use sqlx::{Connection as _, PgConnection};
use uuid::Uuid;

/// One administrator's profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The identity account id (a token's `sub`).
    pub account_id: String,
    /// Permission wire names, in order.
    pub permissions: Vec<String>,
    /// Why, as the operator wrote it.
    pub note: Option<String>,
    /// When the profile was first granted.
    pub granted_at: DateTime<Utc>,
    /// When it last changed.
    pub updated_at: DateTime<Utc>,
}

/// Wire names of `permissions`, sorted and without repeats.
fn names(permissions: &[Permission]) -> Vec<String> {
    let mut names: Vec<Permission> = permissions.to_vec();
    names.sort_unstable();
    names.dedup();
    names.into_iter().map(|p| p.as_str().to_owned()).collect()
}

/// Give `account` the `permissions` (added to any it already holds), and
/// `note` when given. Returns the profile as it now stands.
pub async fn grant(
    conn: &mut PgConnection,
    account: &str,
    permissions: &[Permission],
    note: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Profile> {
    let added = names(permissions);
    let mut tx = conn.begin().await?;
    let profile = sqlx::query_as!(
        Profile,
        r#"INSERT INTO admin.profiles AS p (account_id, permissions, note, granted_at, updated_at)
           VALUES ($1, $2, $3, $4, $4)
           ON CONFLICT (account_id) DO UPDATE
              SET permissions = ARRAY(SELECT DISTINCT unnest(p.permissions || excluded.permissions)
                                       ORDER BY 1),
                  note = coalesce(excluded.note, p.note),
                  updated_at = excluded.updated_at
           RETURNING account_id, permissions AS "permissions!", note, granted_at, updated_at"#,
        account,
        &added,
        note,
        now,
    )
    .fetch_one(&mut *tx)
    .await?;
    record(
        &mut tx,
        "admin.grant",
        serde_json::json!({ "account": account, "permissions": added }),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(profile)
}

/// What [`revoke`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revoked {
    /// The account had no profile; nothing changed.
    NotAnAdmin,
    /// The profile is gone: the account is no longer an administrator.
    Removed,
    /// These permissions remain.
    Remaining(Profile),
}

/// Take `permissions` away from `account`, or its whole profile when
/// `permissions` is empty or nothing would be left.
pub async fn revoke(
    conn: &mut PgConnection,
    account: &str,
    permissions: &[Permission],
    now: DateTime<Utc>,
) -> Result<Revoked> {
    let removed = names(permissions);
    let mut tx = conn.begin().await?;
    let left: Option<Vec<String>> = sqlx::query_scalar!(
        r#"SELECT ARRAY(SELECT unnest(permissions) EXCEPT SELECT unnest($2::text[]) ORDER BY 1)
                  AS "left!"
             FROM admin.profiles WHERE account_id = $1 FOR UPDATE"#,
        account,
        &removed,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let outcome = match left {
        None => return Ok(Revoked::NotAnAdmin),
        Some(left) if removed.is_empty() || left.is_empty() => {
            sqlx::query!("DELETE FROM admin.profiles WHERE account_id = $1", account)
                .execute(&mut *tx)
                .await?;
            Revoked::Removed
        }
        Some(left) => Revoked::Remaining(
            sqlx::query_as!(
                Profile,
                r#"UPDATE admin.profiles SET permissions = $2, updated_at = $3
                    WHERE account_id = $1
                RETURNING account_id, permissions AS "permissions!", note, granted_at, updated_at"#,
                account,
                &left,
                now,
            )
            .fetch_one(&mut *tx)
            .await?,
        ),
    };
    record(
        &mut tx,
        "admin.revoke",
        serde_json::json!({ "account": account, "permissions": removed }),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}

/// Every administrator, oldest grant first.
pub async fn list(conn: &mut PgConnection) -> Result<Vec<Profile>> {
    Ok(sqlx::query_as!(
        Profile,
        r#"SELECT account_id, permissions AS "permissions!", note, granted_at, updated_at
             FROM admin.profiles ORDER BY granted_at, account_id"#
    )
    .fetch_all(conn)
    .await?)
}

/// One entry of the audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// When.
    pub at: DateTime<Utc>,
    /// The administrator's account id (`api`), or the database login (`cli`).
    pub actor: String,
    /// `api` or `cli`.
    pub via: String,
    /// The `operationId` (`api`) or `admin.grant` / `admin.revoke` (`cli`).
    pub operation: String,
    /// Path parameters (`api`) or the change (`cli`).
    pub target: serde_json::Value,
    /// The response status (`api` only).
    pub status: Option<i16>,
}

/// The newest `limit` entries of the audit trail, newest first.
pub async fn log(conn: &mut PgConnection, limit: i64) -> Result<Vec<Action>> {
    Ok(sqlx::query_as!(
        Action,
        "SELECT at, actor, via, operation, target, status
           FROM admin.actions ORDER BY at DESC, id DESC LIMIT $1",
        limit.max(0),
    )
    .fetch_all(conn)
    .await?)
}

/// Append a CLI change to the trail, by the database login.
async fn record(
    conn: &mut PgConnection,
    operation: &str,
    target: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO admin.actions (id, at, actor, via, operation, target)
         VALUES ($1, $2, session_user, 'cli', $3, $4)",
        Uuid::now_v7(),
        now,
        operation,
        target,
    )
    .execute(conn)
    .await?;
    Ok(())
}
