//! Row-level security, checked in SQL: a transaction of the runtime role
//! sees only the rows of the workspace it sets, whatever its query says;
//! the scheduling index is readable by the dispatcher's maintenance
//! transaction (the kernel scope) only.

use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::TimeDelta;
use sqlx::PgPool;

use crate::testkit::{Kit, synthetic};

/// Rows of every workspace-owned table visible to `conn`, unfiltered.
async fn visible(conn: &mut sqlx::PgConnection) -> Vec<i64> {
    let row = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM connectors.connections) AS "c!",
                  (SELECT count(*) FROM connectors.credentials) AS "k!",
                  (SELECT count(*) FROM connectors.streams) AS "s!",
                  (SELECT count(*) FROM connectors.messages) AS "m!",
                  (SELECT count(*) FROM connectors.gmail_labels) AS "l!",
                  (SELECT count(*) FROM connectors.batches) AS "b!""#
    )
    .fetch_one(conn)
    .await
    .unwrap();
    vec![row.c, row.k, row.s, row.m, row.l, row.b]
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_transaction_sees_only_its_workspace(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.google.seed(
        &synthetic(1),
        &["INBOX"],
        kit.clock.now() - TimeDelta::hours(1),
    );
    let a = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, a, Role::Member);
    kit.connect(&ana, a).await;
    kit.run_jobs().await;
    let b = WorkspaceId::new();

    let app = testing::app_role_pool(&kit.db).await;
    let mut tx = app.begin_for(a).await.unwrap();
    assert!(visible(&mut tx).await.iter().all(|&n| n >= 1));
    tx.rollback().await.unwrap();

    let mut tx = app.begin_for(b).await.unwrap();
    assert_eq!(visible(&mut tx).await, [0; 6]);
    let due: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.sync_due"#)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(due, 0);
    let write = sqlx::query!("UPDATE connectors.connections SET health = 'failing'")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(write.rows_affected(), 0);
    tx.rollback().await.unwrap();

    // Without a workspace, nothing is visible, the scheduling index
    // included (campbooks-cloud#129).
    let mut tx = app.begin().await.unwrap();
    assert_eq!(visible(&mut tx).await, [0; 6]);
    let due: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.sync_due"#)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(due, 0);
    tx.rollback().await.unwrap();

    // In the kernel scope (the dispatcher's), only the scheduling index is
    // visible (ids and times).
    let mut tx = campbooks_kernel::db::begin_kernel(&app).await.unwrap();
    assert_eq!(visible(&mut tx).await, [0; 6]);
    let due: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.sync_due"#)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(due, 1);
    tx.rollback().await.unwrap();
}
