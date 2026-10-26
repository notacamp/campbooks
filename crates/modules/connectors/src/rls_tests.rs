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

    // In the maintenance scope (the dispatcher's), only the scheduling index
    // is visible (ids and times).
    let mut tx = campbooks_kernel::db::begin_maintenance(&app).await.unwrap();
    assert_eq!(visible(&mut tx).await, [0; 6]);
    let due: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.sync_due"#)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(due, 1);
    tx.rollback().await.unwrap();

    // The kernel scope reaches the kernel's tables only, not the index
    // (campbooks-cloud#196).
    let mut tx = campbooks_kernel::db::begin_kernel(&app).await.unwrap();
    assert_eq!(visible(&mut tx).await, [0; 6]);
    let due: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.sync_due"#)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(due, 0);
    tx.rollback().await.unwrap();
}

/// Microsoft subscriptions (campbooks-cloud#316): a workspace sees its own;
/// the maintenance scope (the notification callback's) reads them across
/// workspaces and changes none; nothing set, none.
#[sqlx::test(migrations = "../../../migrations")]
async fn microsoft_subscriptions_are_isolated_and_only_read_across_workspaces(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    let a = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, a, Role::Member);
    kit.connect_microsoft(&ana, a).await;
    kit.run_jobs().await;
    let b = WorkspaceId::new();

    let app = testing::app_role_pool(&kit.db).await;
    let mut tx = app.begin_for(a).await.unwrap();
    assert_eq!(subscriptions(&mut tx).await, 1);
    tx.rollback().await.unwrap();

    let mut tx = app.begin_for(b).await.unwrap();
    assert_eq!(subscriptions(&mut tx).await, 0);
    let changed = sqlx::query("UPDATE connectors.microsoft_subscriptions SET retry_at = now()")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(changed.rows_affected(), 0);
    tx.rollback().await.unwrap();

    let mut tx = app.begin().await.unwrap();
    assert_eq!(subscriptions(&mut tx).await, 0);
    tx.rollback().await.unwrap();

    let mut tx = campbooks_kernel::db::begin_maintenance(&app).await.unwrap();
    assert_eq!(subscriptions(&mut tx).await, 1);
    let changed = sqlx::query("UPDATE connectors.microsoft_subscriptions SET retry_at = now()")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        changed.rows_affected(),
        0,
        "the maintenance scope only reads"
    );
    let deleted = sqlx::query("DELETE FROM connectors.microsoft_subscriptions")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(deleted.rows_affected(), 0);
    tx.rollback().await.unwrap();

    let mut tx = campbooks_kernel::db::begin_kernel(&app).await.unwrap();
    assert_eq!(subscriptions(&mut tx).await, 0);
    tx.rollback().await.unwrap();

    // A row cannot pair one workspace's connection with another's stream.
    let stream: uuid::Uuid =
        sqlx::query_scalar("SELECT stream_id FROM connectors.microsoft_subscriptions")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    let forged = sqlx::query(
        "INSERT INTO connectors.microsoft_subscriptions
             (connection_id, workspace_id, stream_id, retry_at, updated_at)
         VALUES ($1, $2, $3, now(), now())",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(b)
    .bind(stream)
    .execute(&kit.db)
    .await;
    assert!(forged.is_err());
}

/// The Microsoft subscriptions `conn` sees.
async fn subscriptions(conn: &mut sqlx::PgConnection) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.microsoft_subscriptions")
        .fetch_one(conn)
        .await
        .unwrap()
}

/// The Notion rows `conn` sees: links, recorded changes and open consents.
async fn counts(conn: &mut sqlx::PgConnection) -> [i64; 3] {
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM connectors.notion_links),
                (SELECT count(*) FROM connectors.notion_changes),
                (SELECT count(*) FROM connectors.notion_authorizations)",
    )
    .fetch_one(conn)
    .await
    .unwrap();
    [row.0, row.1, row.2]
}

/// Notion's links, recorded changes and consents (campbooks-cloud#376): a
/// workspace sees its own; the maintenance scope (the webhook's) reads the
/// links across workspaces, ids only, and nothing else; nothing set, none.
#[sqlx::test(migrations = "../../../migrations")]
async fn notion_rows_are_isolated_and_only_the_links_are_read_across_workspaces(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let a = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, a, Role::Member);
    let connection = kit.connect_notion(&marta, a).await;
    let notion_page = kit.notion.add_page("Roadmap", None);
    let block = kit.notion.paragraph(notion_page, "Plan");
    let page = uuid::Uuid::now_v7();
    kit.link_notion(a, &connection, page, notion_page).await;
    let mine = uuid::Uuid::now_v7();
    kit.ask_notion_changes(
        a,
        &connection,
        (page, notion_page),
        vec![crate::testkit_notion::update_item(mine, block, None)],
        &crate::testkit_notion::paragraphs(&[(0, mine, "Plan more")]),
        true,
    )
    .await;
    // A consent still open.
    let ticket = kit.notion_start(&marta, a, serde_json::json!({})).await;
    assert_eq!(ticket.status.as_u16(), 201);
    let b = WorkspaceId::new();

    let app = testing::app_role_pool(&kit.db).await;
    let mut tx = app.begin_for(a).await.unwrap();
    assert_eq!(counts(&mut tx).await, [1, 1, 1]);
    tx.rollback().await.unwrap();

    let mut tx = app.begin_for(b).await.unwrap();
    assert_eq!(counts(&mut tx).await, [0, 0, 0]);
    for table in ["notion_links", "notion_changes", "notion_authorizations"] {
        let changed = sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE connectors.{table} SET workspace_id = workspace_id"
        )))
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(changed.rows_affected(), 0, "{table}");
        let deleted = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM connectors.{table}"
        )))
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(deleted.rows_affected(), 0, "{table}");
    }
    tx.rollback().await.unwrap();

    let mut tx = app.begin().await.unwrap();
    assert_eq!(counts(&mut tx).await, [0, 0, 0]);
    tx.rollback().await.unwrap();

    // The maintenance scope reads the links and nothing else, and changes none.
    let mut tx = campbooks_kernel::db::begin_maintenance(&app).await.unwrap();
    let [links, changes, consents] = counts(&mut tx).await;
    assert_eq!((links, changes, consents), (1, 0, 0));
    let changed = sqlx::query("UPDATE connectors.notion_links SET page_id = gen_random_uuid()")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(changed.rows_affected(), 0);
    let deleted = sqlx::query("DELETE FROM connectors.notion_links")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(deleted.rows_affected(), 0);
    tx.rollback().await.unwrap();

    let mut tx = campbooks_kernel::db::begin_kernel(&app).await.unwrap();
    assert_eq!(counts(&mut tx).await, [0, 0, 0]);
    tx.rollback().await.unwrap();

    // A link cannot pair one workspace's connection with another's.
    let stream: uuid::Uuid = sqlx::query_scalar("SELECT stream_id FROM connectors.notion_links")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let forged = sqlx::query(
        "INSERT INTO connectors.notion_links
             (stream_id, workspace_id, connection_id, notion_page_id, page_id)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(stream)
    .bind(b)
    .bind(uuid::Uuid::now_v7())
    .bind(uuid::Uuid::now_v7())
    .bind(uuid::Uuid::now_v7())
    .execute(&kit.db)
    .await;
    assert!(forged.is_err());
}
