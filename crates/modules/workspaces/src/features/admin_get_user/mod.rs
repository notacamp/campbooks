//! `GET /admin/v1/workspaces/users/{user_id}` — Admin → People → one
//! person: who they are and which workspaces they belong to, with their role
//! in each.
//!
//! An admin operation (ADR 0015). Requires `users.read` (or `super`).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersRead},
    db, extract,
    tenancy::{Role, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::domain::model::Kind;

use super::admin_list_users::AdminUser;

/// One of a person's workspaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminUserWorkspace {
    /// The workspace.
    pub id: WorkspaceId,
    /// Its name.
    pub name: String,
    /// What it is for.
    pub kind: Kind,
    /// Their role in it.
    pub role: Role,
    /// When they joined.
    pub joined_at: DateTime<Utc>,
    /// When it was deleted (it is purged later); absent while it lives.
    pub deleted_at: Option<DateTime<Utc>>,
}

/// A person and their workspaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminUserDetail {
    /// Who they are.
    pub user: AdminUser,
    /// Their workspaces, in the order they joined.
    pub workspaces: Vec<AdminUserWorkspace>,
}

/// The person a path names.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct UserPath {
    /// The user.
    pub user_id: UserId,
}

/// See one person.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/users/{user_id}",
    operation_id = "workspaces.admin_get_user",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<UsersRead>()))),
    params(("user_id" = UserId, Path, description = "The user.")),
    responses(
        (status = OK, description = "The person and their workspaces.", body = AdminUserDetail),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such user, or not an administrator with \
            `users.read`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersRead>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<UserPath>,
) -> Result<Json<AdminUserDetail>> {
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let detail = execute(&mut tx, path.user_id).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

/// User `id` and their workspaces, read in an admin transaction.
pub async fn execute(conn: &mut PgConnection, id: UserId) -> Result<AdminUserDetail> {
    let user = sqlx::query!(
        r#"
        SELECT u.id AS "id: UserId", u.idp_subject, u.display_name, u.email, u.created_at
          FROM workspaces.users u
         WHERE u.id = $1
        "#,
        id as UserId,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    let workspaces = sqlx::query!(
        r#"
        SELECT w.id AS "id: WorkspaceId", w.name, w.kind, m.role, m.joined_at, w.deleted_at
          FROM workspaces.members m
          JOIN workspaces.workspaces w ON w.id = m.workspace_id
         WHERE m.user_id = $1
         ORDER BY m.joined_at, w.id
        "#,
        id as UserId,
    )
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|row| {
        Ok(AdminUserWorkspace {
            id: row.id,
            name: row.name,
            kind: row.kind.parse()?,
            role: row.role.parse()?,
            joined_at: row.joined_at,
            deleted_at: row.deleted_at,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let live = i64::try_from(
        workspaces
            .iter()
            .filter(|workspace| workspace.deleted_at.is_none())
            .count(),
    )
    .unwrap_or(i64::MAX);
    Ok(AdminUserDetail {
        user: AdminUser {
            id: user.id,
            account_id: user.idp_subject,
            display_name: user.display_name,
            email: user.email,
            created_at: user.created_at,
            workspace_count: live,
        },
        workspaces,
    })
}

#[cfg(test)]
mod tests;
