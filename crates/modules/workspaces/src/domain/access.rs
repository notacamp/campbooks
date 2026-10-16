//! Who may do what inside a workspace.

use campbooks_kernel::{
    AppState, Error, RecordRef, Result,
    authz::{self, Action},
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use sqlx::PgConnection;

use crate::{
    MESSAGES,
    events::{MEMBER_RECORD, WORKSPACE_RECORD},
};

/// Ask the access catalogue (ADR 0017) whether `access` may take `action`
/// on `resource`: 404 when they may not see it, 403 when they may see it
/// but not do that.
pub async fn authorize(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    resource: &RecordRef,
) -> Result<()> {
    authz::require(state, conn, access, action, resource, || Error::Forbidden).await
}

/// [`authorize`] for resources of a type before one is named (lists,
/// creating one).
pub fn authorize_type(
    state: &AppState,
    access: &Access,
    action: Action,
    resource_type: &str,
) -> Result<()> {
    authz::require_type(state, access, action, resource_type, || Error::Forbidden)
}

/// The workspace of `access`, as a resource.
pub fn workspace_resource(access: &Access) -> RecordRef {
    RecordRef::new(WORKSPACE_RECORD, &access.workspace)
}

/// Member `user`, as a resource.
pub fn member_resource(user: UserId) -> RecordRef {
    RecordRef::new(MEMBER_RECORD, &user)
}

/// The user acting through `access`, with their role. Client tokens act
/// through their scopes only and cannot take role-gated actions: 403.
pub fn acting_user(access: &Access) -> Result<(UserId, Role)> {
    match (access.user, access.role) {
        (Some(user), Some(role)) => Ok((user, role)),
        _ => Err(Error::Forbidden),
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
