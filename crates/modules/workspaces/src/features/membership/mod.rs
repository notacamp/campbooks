//! The kernel's `Membership` port: may this caller work in this workspace?
//! Called by the `Workspace<S>` extractor on every workspace-owned request,
//! of every module.

use std::sync::Arc;

use campbooks_kernel::{
    auth::{Principal, PrincipalKind},
    db::BeginFor as _,
    tenancy::{
        Access, Membership, MembershipFuture, MembershipPort, Role, UserId, WorkspaceId,
        WorkspaceSummary, WorkspacesFuture,
    },
};
use sqlx::PgPool;

use crate::domain::db::begin_as_user;

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

    fn workspaces_of<'a>(&'a self, db: &'a PgPool, subject: &'a str) -> WorkspacesFuture<'a> {
        Box::pin(async move {
            let Some(user) = sqlx::query_scalar!(
                r#"SELECT id AS "id: UserId" FROM workspaces.users WHERE idp_subject = $1"#,
                subject
            )
            .fetch_optional(db)
            .await?
            else {
                return Ok(Vec::new());
            };
            // Across workspaces, as that user: row-level security applies.
            let mut tx = begin_as_user(db, user).await?;
            let rows = sqlx::query!(
                r#"
                SELECT w.id AS "id: WorkspaceId", w.name, m.role
                  FROM workspaces.members m
                  JOIN workspaces.workspaces w ON w.id = m.workspace_id
                 WHERE m.user_id = $1 AND w.deleted_at IS NULL
                 ORDER BY w.created_at, w.id
                "#,
                user as UserId,
            )
            .fetch_all(&mut *tx)
            .await?;
            tx.commit().await?;
            rows.into_iter()
                .map(|row| {
                    Ok(WorkspaceSummary {
                        id: row.id,
                        name: row.name,
                        role: row.role.parse::<Role>()?,
                    })
                })
                .collect()
        })
    }

    fn access_records(&self) -> &'static [&'static str] {
        &[
            crate::events::MEMBER_RECORD,
            crate::events::WORKSPACE_RECORD,
        ]
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::{Membership as _, Role};
    use sqlx::PgPool;

    use super::WorkspaceMembership;
    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn workspaces_of_lists_a_persons_live_workspaces_only(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let bea = kit.user("bea");
        let mine = kit.create_workspace(&ana, "Mine").await;
        let gone = kit.create_workspace(&ana, "Gone").await;
        kit.create_workspace(&bea, "Hers").await;
        kit.delete(&ana, &format!("/api/v1/w/{gone}")).await;

        let listed = WorkspaceMembership
            .workspaces_of(&kit.state.db, "ana")
            .await
            .unwrap();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0].id, mine);
        assert_eq!(listed[0].name, "Mine");
        assert_eq!(listed[0].role, Role::Owner);
        let nobody = WorkspaceMembership
            .workspaces_of(&kit.state.db, "nobody")
            .await
            .unwrap();
        assert!(nobody.is_empty());
    }
}
