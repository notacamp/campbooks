//! The kernel's `Membership` port: may this caller work in this workspace?
//! Called by the `Workspace<S>` extractor on every workspace-owned request,
//! of every module.

use std::sync::Arc;

use campbooks_kernel::{
    auth::{Principal, PrincipalKind},
    db::BeginFor as _,
    tenancy::{Access, Membership, MembershipFuture, MembershipPort, Role, UserId, WorkspaceId},
};
use sqlx::PgPool;

/// The port implementation the server registers in `AppState`.
pub fn membership() -> MembershipPort {
    Arc::new(WorkspaceMembership)
}

/// Membership as this module records it.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceMembership;

impl Membership for WorkspaceMembership {
    fn resolve<'a>(
        &'a self,
        db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        Box::pin(async move {
            // Read in the workspace, so row-level security applies.
            let mut tx = db.begin_for(workspace).await?;
            let access = match principal.kind {
                PrincipalKind::User => sqlx::query!(
                    r#"
                    SELECT m.user_id AS "user_id: UserId", m.role
                      FROM workspaces.members m
                      JOIN workspaces.users u ON u.id = m.user_id
                      JOIN workspaces.workspaces w ON w.id = m.workspace_id
                     WHERE m.workspace_id = $1 AND u.idp_subject = $2 AND w.deleted_at IS NULL
                    "#,
                    workspace as WorkspaceId,
                    principal.subject,
                )
                .fetch_optional(&mut *tx)
                .await?
                .map(|row| {
                    Ok::<_, campbooks_kernel::Error>(Access {
                        workspace,
                        user: Some(row.user_id),
                        role: Some(row.role.parse::<Role>()?),
                    })
                })
                .transpose()?,
                // The extractor already matched the token's workspace claim.
                PrincipalKind::Client => sqlx::query_scalar!(
                    r#"SELECT id AS "id: WorkspaceId" FROM workspaces.workspaces
                        WHERE id = $1 AND deleted_at IS NULL"#,
                    workspace as WorkspaceId,
                )
                .fetch_optional(&mut *tx)
                .await?
                .map(|_| Access {
                    workspace,
                    user: None,
                    role: None,
                }),
            };
            tx.commit().await?;
            Ok(access)
        })
    }
}
