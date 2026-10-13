//! Row-level security, checked in SQL: a transaction of the runtime role
//! sees only the rows of the workspace it sets, whatever its query says.

use campbooks_kernel::{
    db::{self, BeginFor as _},
    tenancy::WorkspaceId,
    testing,
};
use sqlx::PgPool;

use crate::{domain::db::set_user, testkit::Kit};

/// Rows of each workspace-owned table visible to `tx`, with no filter:
/// (workspaces, members, invitations, security events).
async fn visible(conn: &mut sqlx::PgConnection) -> (Vec<WorkspaceId>, i64, i64, i64) {
    let workspaces = sqlx::query_scalar!(
        r#"SELECT id AS "id: WorkspaceId" FROM workspaces.workspaces ORDER BY id"#
    )
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    let counts = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM workspaces.members) AS "m!",
                  (SELECT count(*) FROM workspaces.invitations) AS "i!",
                  (SELECT count(*) FROM workspaces.security_events) AS "e!""#
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    (workspaces, counts.m, counts.i, counts.e)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_transaction_sees_only_its_workspace(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    let a = kit.create_workspace(&ana, "A").await;
    kit.invite(&ana, a, "x@example.com", "member").await;
    let b = kit.create_workspace(&kit.user("bea"), "B").await;

    let app = testing::app_role_pool(&kit.db).await;
    let mut tx = app.begin_for(b).await.unwrap();
    let (workspaces, members, invitations, events) = visible(&mut tx).await;
    assert_eq!(workspaces, [b]);
    assert_eq!((members, invitations, events), (1, 0, 1));
    // Filtering on A explicitly finds nothing either.
    let a_members = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM workspaces.members WHERE workspace_id = $1"#,
        a as WorkspaceId
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(a_members, 0);
    // Nor can it write into A.
    let write = sqlx::query!(
        "UPDATE workspaces.workspaces SET name = 'taken' WHERE id = $1",
        a as WorkspaceId
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(write.rows_affected(), 0);
    let insert = sqlx::query!(
        "INSERT INTO workspaces.members (workspace_id, user_id, role)
         SELECT $1, id, 'owner' FROM workspaces.users WHERE idp_subject = 'bea'",
        a as WorkspaceId
    )
    .execute(&mut *tx)
    .await;
    assert!(
        insert.is_err(),
        "a row for another workspace violates the policy"
    );
    tx.rollback().await.unwrap();

    // The owner (a superuser) sees everything, which is why the server never
    // queries as it.
    let mut all = kit.db.begin().await.unwrap();
    db::set_workspace(&mut all, &b.to_string()).await.unwrap();
    assert_eq!(visible(&mut all).await.0.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_workspace_nothing_is_visible(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.create_workspace(&kit.user("ana"), "A").await;
    let app = testing::app_role_pool(&kit.db).await;

    // Never set, and set then ended (the setting then reads as '').
    let mut conn = app.acquire().await.unwrap();
    assert_eq!(visible(&mut conn).await, (vec![], 0, 0, 0));
    let mut tx = app.begin_for(WorkspaceId::new()).await.unwrap();
    assert_eq!(visible(&mut tx).await, (vec![], 0, 0, 0));
    tx.commit().await.unwrap();
    assert_eq!(visible(&mut conn).await, (vec![], 0, 0, 0));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_user_transaction_sees_only_their_own_memberships(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    let a = kit.create_workspace(&ana, "A").await;
    kit.create_workspace(&kit.user("bea"), "B").await;
    let ana_id = kit.user_id("ana").await;

    let app = testing::app_role_pool(&kit.db).await;
    let mut tx = app.begin().await.unwrap();
    set_user(&mut tx, ana_id).await.unwrap();
    let (workspaces, members, invitations, events) = visible(&mut tx).await;
    assert_eq!(workspaces, [a]);
    // Her membership and her own first sign-in; no workspace's invitations
    // or log.
    assert_eq!((members, invitations, events), (1, 0, 1));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_runtime_role_does_not_bypass_row_security(db: PgPool) {
    assert!(
        db::bypasses_row_security(&db).await.unwrap(),
        "the test owner is a superuser"
    );
    let app = testing::app_role_pool(&db).await;
    assert!(!db::bypasses_row_security(&app).await.unwrap());
}
