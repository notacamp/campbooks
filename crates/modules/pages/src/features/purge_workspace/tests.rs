//! A deleted workspace's pages go.

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::{events::WorkspaceDeleting, testkit::Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn deleting_the_workspace_deletes_its_pages_and_everything_they_hold(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (bo, _) = kit.member("bo", other);
    let page = kit.add(&ana, workspace, json!({"title": "Plan"})).await;
    let uri = format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap());
    let doc = crate::testkit::docs::client(1);
    let update = crate::testkit::docs::paragraph(&doc, "text");
    kit.sync(
        &ana,
        workspace,
        page["id"].as_str().unwrap(),
        &doc,
        Some(&update),
    )
    .await;
    kit.upload(
        &ana,
        &format!("{uri}/files"),
        "text/plain",
        b"a file".to_vec(),
    )
    .await;
    kit.add(&bo, other, json!({"title": "Keep"})).await;

    let mut tx = kit.begin(workspace).await;
    kit.state
        .events
        .publish(
            &mut tx,
            &WorkspaceDeleting {
                workspace: workspace.to_string(),
                purge_after: Utc::now() - TimeDelta::hours(1),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;

    assert_eq!(kit.page_count(workspace).await, 0);
    for table in ["updates", "snapshots", "grants", "links", "files"] {
        let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM pages.{table} WHERE workspace_id = $1"
        )))
        .bind(workspace.as_uuid())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert_eq!(rows, 0, "{table}");
    }
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'pages.page' AND workspace_id = $1",
    )
    .bind(workspace.as_uuid().to_string())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 0);
    assert_eq!(kit.page_count(other).await, 1);
}
