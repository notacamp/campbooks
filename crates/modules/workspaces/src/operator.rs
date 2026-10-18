//! What the server's operator commands read about a workspace's members
//! (campbooks-cloud#219): `campbooks-server labelling export --member` bounds
//! an export by one member's visibility, so it first finds who that is.

use campbooks_kernel::{
    Result,
    tenancy::{Role, UserId, WorkspaceId},
};
use sqlx::PgConnection;
use uuid::Uuid;

/// The members of `workspace` that `who` names, with their role: a member's
/// id, or the verified address on their profile (case ignored). More than one when
/// an address is on several profiles; none when nobody in the workspace
/// matches. Read in `conn`, a transaction working in `workspace`.
pub async fn members_named(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    who: &str,
) -> Result<Vec<(UserId, Role)>> {
    let who = who.trim();
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT m.user_id, m.role FROM workspaces.members m
           JOIN workspaces.users u ON u.id = m.user_id
          WHERE m.workspace_id = $1
            AND (u.id::text = lower($2) OR (u.email_verified AND lower(u.email) = lower($2)))
          ORDER BY m.user_id",
    )
    .bind(workspace.as_uuid())
    .bind(who)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|(user, role)| Ok((UserId::from_uuid(user), role.parse()?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn finds_a_member_by_id_or_verified_address(db: PgPool) {
        let (workspace, sam, leo, eve) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        sqlx::query(
            "INSERT INTO workspaces.users (id, idp_subject, email, email_verified) VALUES
                 ($1, 'sam', 'Sam@Example.com', true),
                 ($2, 'leo', 'leo@example.com', true),
                 ($3, 'eve', 'eve@example.com', false)",
        )
        .bind(sam)
        .bind(leo)
        .bind(eve)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workspaces.workspaces (id, name, kind, accent)
             VALUES ($1, 'Rivera Studio', 'business', 'business')",
        )
        .bind(workspace)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workspaces.members (workspace_id, user_id, role)
             VALUES ($1, $2, 'owner'), ($1, $3, 'member')",
        )
        .bind(workspace)
        .bind(sam)
        .bind(eve)
        .execute(&db)
        .await
        .unwrap();
        let mut conn = db.acquire().await.unwrap();
        // As the CLI reads it: working in the workspace.
        sqlx::query("SELECT set_config('campbooks.workspace_id', $1, false)")
            .bind(workspace.to_string())
            .execute(&mut *conn)
            .await
            .unwrap();
        let ws = WorkspaceId::from_uuid(workspace);
        let sam_owner = vec![(UserId::from_uuid(sam), Role::Owner)];
        assert_eq!(
            members_named(&mut conn, ws, " sam@example.com ")
                .await
                .unwrap(),
            sam_owner
        );
        assert_eq!(
            members_named(&mut conn, ws, &sam.to_string())
                .await
                .unwrap(),
            sam_owner
        );
        // Not a member here, an unverified address, nobody: no one.
        for who in ["leo@example.com", "eve@example.com", "nobody@example.com"] {
            assert!(
                members_named(&mut conn, ws, who).await.unwrap().is_empty(),
                "{who}"
            );
        }
    }
}
