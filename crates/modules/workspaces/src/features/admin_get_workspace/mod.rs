//! `GET /admin/v1/workspaces/workspaces/{workspace_id}` — Admin →
//! Workspaces → one workspace: its members with their roles, and how many
//! invitations wait for an answer (a count; never the addresses).
//!
//! An admin operation (ADR 0015). Requires `workspaces.read` (or `super`).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::WorkspacesRead},
    db, extract,
    tenancy::{UserId, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{domain::model::MemberView, features::list_members};

use super::admin_list_workspaces::AdminWorkspace;

/// A workspace and who is in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminWorkspaceDetail {
    /// The workspace.
    pub workspace: AdminWorkspace,
    /// Its members, in the order they joined.
    pub members: Vec<MemberView>,
    /// How many invitations are pending (not accepted, not expired).
    pub pending_invitations: i64,
}

/// The workspace a path names.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct WorkspacePath {
    /// The workspace.
    pub workspace_id: WorkspaceId,
}

/// See one workspace.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/workspaces/{workspace_id}",
    operation_id = "workspaces.admin_get_workspace",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<WorkspacesRead>()))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The workspace and its members.",
            body = AdminWorkspaceDetail),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or not an administrator with \
            `workspaces.read`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<WorkspacesRead>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<WorkspacePath>,
) -> Result<Json<AdminWorkspaceDetail>> {
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let detail = execute(&mut tx, path.workspace_id).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

/// Workspace `id`, its members and its pending invitations, read in an
/// admin transaction.
pub async fn execute(conn: &mut PgConnection, id: WorkspaceId) -> Result<AdminWorkspaceDetail> {
    let row = sqlx::query!(
        r#"
        SELECT w.id AS "id: WorkspaceId", w.name, w.kind, w.created_at, w.deleted_at,
               (SELECT count(*) FROM workspaces.invitations i
                 WHERE i.workspace_id = w.id AND i.accepted_at IS NULL
                   AND i.expires_at > now()) AS "pending_invitations!"
          FROM workspaces.workspaces w
         WHERE w.id = $1
        "#,
        id as WorkspaceId,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    let members = list_members::load(conn, id, None, None).await?;
    let owner_ids: Vec<UserId> = members
        .iter()
        .filter(|member| member.role == campbooks_kernel::tenancy::Role::Owner)
        .map(|member| member.user_id)
        .collect();
    Ok(AdminWorkspaceDetail {
        workspace: AdminWorkspace {
            id: row.id,
            name: row.name,
            kind: row.kind.parse()?,
            created_at: row.created_at,
            deleted_at: row.deleted_at,
            member_count: i64::try_from(members.len()).unwrap_or(i64::MAX),
            owner_ids,
        },
        members,
        pending_invitations: row.pending_invitations,
    })
}

#[cfg(test)]
mod tests;
