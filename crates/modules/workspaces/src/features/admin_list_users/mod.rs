//! `GET /admin/v1/workspaces/users` — Admin → People: everyone who has
//! signed in, newest first, searchable by name or address prefix.
//!
//! An admin operation (ADR 0015): served by the admin API only, never in the
//! operation catalogue. Requires `users.read` (or `super`). Metadata only:
//! who, since when, and in how many workspaces.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersRead},
    db, extract,
    tenancy::UserId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::admin_page::{AdminPage, Paging};

/// A person, as an administrator sees them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminUser {
    /// The user.
    pub id: UserId,
    /// Their identity account (the `sub` of their tokens), for the identity
    /// module's admin view of the account.
    pub account_id: String,
    /// Their name, as their identity provider gives it.
    pub display_name: Option<String>,
    /// Their email address, as their identity provider gives it.
    pub email: Option<String>,
    /// When they first signed in.
    pub created_at: DateTime<Utc>,
    /// How many workspaces they belong to (not counting deleted ones).
    pub workspace_count: i64,
}

/// One page of people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminUserPage {
    /// Newest first.
    pub users: Vec<AdminUser>,
    /// The `after` of the next page; absent on the last one.
    pub next: Option<String>,
}

/// List people.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/users",
    operation_id = "workspaces.admin_list_users",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<UsersRead>()))),
    params(AdminPage),
    responses(
        (status = OK, description = "A page of people.", body = AdminUserPage),
        (status = BAD_REQUEST, description = "`invalid_query`, `invalid_limit`, `invalid_cursor` or \
            `query_too_long`.", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `users.read` (the same \
            answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersRead>,
    State(state): State<AppState>,
    extract::Query(page): extract::Query<AdminPage>,
) -> Result<Json<AdminUserPage>> {
    let paging = page.paging()?;
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let page = execute(&mut tx, &paging).await?;
    tx.commit().await?;
    Ok(Json(page))
}

/// One page of people, read in an admin transaction.
pub async fn execute(conn: &mut sqlx::PgConnection, paging: &Paging) -> Result<AdminUserPage> {
    let rows = sqlx::query!(
        r#"
        SELECT u.id AS "id: UserId", u.idp_subject, u.display_name, u.email, u.created_at,
               (SELECT count(*) FROM workspaces.members m
                  JOIN workspaces.workspaces w ON w.id = m.workspace_id
                 WHERE m.user_id = u.id AND w.deleted_at IS NULL) AS "workspace_count!"
          FROM workspaces.users u
         WHERE ($1::text IS NULL OR u.display_name ILIKE $1 OR u.email ILIKE $1)
           AND ($2::timestamptz IS NULL OR (u.created_at, u.id) < ($2, $3))
         ORDER BY u.created_at DESC, u.id DESC
         LIMIT $4
        "#,
        paging.prefix,
        paging.after_at(),
        paging.after_id(),
        paging.fetch(),
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|row| AdminUser {
        id: row.id,
        account_id: row.idp_subject,
        display_name: row.display_name,
        email: row.email,
        created_at: row.created_at,
        workspace_count: row.workspace_count,
    })
    .collect();
    let (users, next) = paging.finish(rows, |user: &AdminUser| {
        (user.created_at, *user.id.as_uuid())
    });
    Ok(AdminUserPage { users, next })
}

#[cfg(test)]
mod tests;
