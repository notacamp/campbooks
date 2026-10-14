//! `GET /admin/v1/identity/accounts/{account_id}` — Admin → People → one
//! person's account: their primary address, how they get in, how many
//! passkeys and live sessions they have, when they last signed in, and
//! whether the account is suspended.
//!
//! An admin operation (ADR 0015). Requires `users.read` (or `super`).
//! `account_id` is the `sub` of the person's tokens (the workspaces
//! module's `account_id`). Never a token, a session or a passkey's key.
//! While another identity provider serves the instance there are no
//! accounts here, and every id answers 404.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersRead},
    db, extract,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::accounts;

/// A way in, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminWayIn {
    /// `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The address the provider gave when it was added.
    pub email: Option<String>,
    /// `sign_up`, `link` or `connect`.
    pub linked_via: String,
    /// When it was added.
    pub linked_at: DateTime<Utc>,
    /// When it was last used to sign in.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// The account's sign-ins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminSessions {
    /// Browsers signed in to the issuer right now.
    pub active: i64,
    /// When the person last signed in.
    pub last_sign_in_at: Option<DateTime<Utc>>,
}

/// An identity account, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminAccount {
    /// The account (the `sub` of its tokens).
    pub id: Uuid,
    /// Its display name.
    pub display_name: Option<String>,
    /// The address it was created with.
    pub primary_email: Option<String>,
    /// Whether a provider verified that address.
    pub primary_email_verified: bool,
    /// When it was created.
    pub created_at: DateTime<Utc>,
    /// When an administrator suspended it; absent unless suspended.
    pub suspended_at: Option<DateTime<Utc>>,
    /// Its provider ways in, oldest first.
    pub ways_in: Vec<AdminWayIn>,
    /// How many passkeys it has.
    pub passkeys: i64,
    /// Its sign-ins.
    pub sessions: AdminSessions,
}

/// The account a path names (by field, so the route resolves under
/// `/bff/admin/{*path}`).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AccountPath {
    /// The account.
    pub account_id: Uuid,
}

/// See one account.
#[utoipa::path(
    get,
    path = "/admin/v1/identity/accounts/{account_id}",
    operation_id = "identity.admin_get_account",
    tag = "identity",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<UsersRead>()))),
    params(("account_id" = Uuid, Path, description = "The account (a token `sub`).")),
    responses(
        (status = OK, description = "The account.", body = AdminAccount),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such account (or another identity provider \
            serves the instance), or not an administrator with `users.read`.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersRead>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<AccountPath>,
) -> Result<Json<AdminAccount>> {
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let account = load(&mut tx, path.account_id, Utc::now())
        .await?
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(Json(account))
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: Uuid,
    display_name: Option<String>,
    primary_email: Option<String>,
    primary_email_verified: bool,
    created_at: DateTime<Utc>,
    disabled_at: Option<DateTime<Utc>>,
    passkeys: i64,
    active_sessions: i64,
    last_sign_in_at: Option<DateTime<Utc>>,
}

/// Account `id` as an administrator sees it at `now`, if it exists.
pub async fn load(
    conn: &mut PgConnection,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<AdminAccount>> {
    let row: Option<AccountRow> = sqlx::query_as(
        "SELECT a.id, a.display_name, a.primary_email, a.primary_email_verified, a.created_at,
                a.disabled_at,
                (SELECT count(*) FROM identity.passkeys p WHERE p.account_id = a.id) AS passkeys,
                (SELECT count(*) FROM identity.sessions s
                  WHERE s.account_id = a.id AND s.ended_at IS NULL
                    AND s.idle_expires_at > $2 AND s.expires_at > $2) AS active_sessions,
                (SELECT max(s.created_at) FROM identity.sessions s
                  WHERE s.account_id = a.id) AS last_sign_in_at
           FROM identity.accounts a
          WHERE a.id = $1",
    )
    .bind(id)
    .bind(now)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let ways_in = accounts::ways_in(&mut *conn, id)
        .await?
        .into_iter()
        .map(|way| AdminWayIn {
            provider: way.provider,
            email: way.email,
            linked_via: way.linked_via,
            linked_at: way.linked_at,
            last_used_at: way.last_used_at,
        })
        .collect();
    Ok(Some(AdminAccount {
        id: row.id,
        display_name: row.display_name,
        primary_email: row.primary_email,
        primary_email_verified: row.primary_email_verified,
        created_at: row.created_at,
        suspended_at: row.disabled_at,
        ways_in,
        passkeys: row.passkeys,
        sessions: AdminSessions {
            active: row.active_sessions,
            last_sign_in_at: row.last_sign_in_at,
        },
    }))
}

#[cfg(test)]
mod tests;
