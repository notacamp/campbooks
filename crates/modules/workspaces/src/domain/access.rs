//! Who may do what inside a workspace.

use campbooks_kernel::{
    Error, Result,
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use sqlx::PgConnection;

use crate::MESSAGES;

/// The user acting through `access`, with their role. Client tokens act
/// through their scopes only and cannot take role-gated actions: 403.
pub fn acting_user(access: &Access) -> Result<(UserId, Role)> {
    match (access.user, access.role) {
        (Some(user), Some(role)) => Ok((user, role)),
        _ => Err(Error::Forbidden),
    }
}

/// [`acting_user`], who must be an owner or an admin.
pub fn acting_manager(access: &Access) -> Result<(UserId, Role)> {
    let (user, role) = acting_user(access)?;
    if role.can_manage() {
        Ok((user, role))
    } else {
        Err(Error::Forbidden)
    }
}

/// Lock the live workspace's row until the transaction ends, so concurrent
/// membership changes (and the last-owner rule) are serialized.
pub async fn lock_workspace(conn: &mut PgConnection, workspace: WorkspaceId) -> Result<()> {
    sqlx::query_scalar!(
        r#"SELECT id AS "id: WorkspaceId" FROM workspaces.workspaces
            WHERE id = $1 AND deleted_at IS NULL
              FOR UPDATE"#,
        workspace as WorkspaceId
    )
    .fetch_optional(conn)
    .await?
    .map(|_| ())
    .ok_or(Error::NotFound)
}

/// The role of `user` in `workspace`, if they are a member.
pub async fn role_of(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
) -> Result<Option<Role>> {
    sqlx::query_scalar!(
        "SELECT role FROM workspaces.members WHERE workspace_id = $1 AND user_id = $2",
        workspace as WorkspaceId,
        user as UserId,
    )
    .fetch_optional(conn)
    .await?
    .map(|role| role.parse())
    .transpose()
}

/// Refuse (409 `last_owner`) when `user`, currently `role`, is the
/// workspace's only owner. Call with the workspace locked.
pub async fn keep_an_owner(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    role: Role,
) -> Result<()> {
    if role != Role::Owner {
        return Ok(());
    }
    let owners = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM workspaces.members
            WHERE workspace_id = $1 AND role = 'owner'"#,
        workspace as WorkspaceId
    )
    .fetch_one(conn)
    .await?;
    if owners <= 1 {
        return Err(Error::Conflict(MESSAGES.reason("last_owner")));
    }
    Ok(())
}
