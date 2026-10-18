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
/// id, or the verified address on their profile (case ignored). As the API
/// lets them in: a member who is suspended, or of a workspace being deleted,
/// is no one. More than one when an address is on several profiles; none
/// when nobody in the workspace matches. Read in `conn`, a transaction
/// working in `workspace`.
pub async fn members_named(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    who: &str,
) -> Result<Vec<(UserId, Role)>> {
    let who = who.trim();
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT m.user_id, m.role FROM workspaces.members m
           JOIN workspaces.users u ON u.id = m.user_id
           JOIN workspaces.workspaces w ON w.id = m.workspace_id AND w.deleted_at IS NULL
          WHERE m.workspace_id = $1 AND NOT u.suspended
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

    /// Rivera Studio (Sam its owner, Eve unverified, Ana suspended) and
    /// another workspace Leo belongs to.
    async fn seed(db: &PgPool, ids: [Uuid; 6]) {
        let [workspace, other, sam, leo, eve, ana] = ids;
        sqlx::query(
            "INSERT INTO workspaces.users
                    (id, idp_subject, email, email_verified, suspended, suspension_at) VALUES
                 ($1, 'sam', 'Sam@Example.com', true, false, NULL),
                 ($2, 'leo', 'leo@example.com', true, false, NULL),
                 ($3, 'eve', 'eve@example.com', false, false, NULL),
                 ($4, 'ana', 'ana@example.com', true, true, now())",
        )
        .bind(sam)
        .bind(leo)
        .bind(eve)
        .bind(ana)
        .execute(db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workspaces.workspaces (id, name, kind, accent)
             VALUES ($1, 'Rivera Studio', 'business', 'business'),
                    ($2, 'Elsewhere', 'business', 'business')",
        )
        .bind(workspace)
        .bind(other)
        .execute(db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO workspaces.members (workspace_id, user_id, role)
             VALUES ($1, $2, 'owner'), ($1, $3, 'member'), ($1, $4, 'member'),
                    ($5, $6, 'member')",
        )
        .bind(workspace)
        .bind(sam)
        .bind(eve)
        .bind(ana)
        .bind(other)
        .bind(leo)
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn finds_a_member_by_id_or_verified_address(db: PgPool) {
        let (workspace, other, sam, leo, eve, ana) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        seed(&db, [workspace, other, sam, leo, eve, ana]).await;
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
        // A member elsewhere only, an unverified address, a suspended member,
        // nobody: no one.
        for who in [
            "leo@example.com",
            "eve@example.com",
            "ana@example.com",
            "nobody@example.com",
        ] {
            assert!(
                members_named(&mut conn, ws, who).await.unwrap().is_empty(),
                "{who}"
            );
        }
        // A workspace being deleted has no one to export for.
        sqlx::query(
            "UPDATE workspaces.workspaces SET deleted_at = now(), purge_after = now()
              WHERE id = $1",
        )
        .bind(workspace)
        .execute(&db)
        .await
        .unwrap();
        assert!(
            members_named(&mut conn, ws, "sam@example.com")
                .await
                .unwrap()
                .is_empty()
        );
    }
}
