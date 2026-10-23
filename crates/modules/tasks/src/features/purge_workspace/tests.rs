//! A deleted workspace's tasks go.

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::{events::WorkspaceDeleting, testkit::Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn deleting_the_workspace_deletes_its_tasks_and_history(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (bo, _) = kit.member("bo", other);
    kit.add(&ana, workspace, json!({"title": "Call"})).await;
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

    assert_eq!(kit.task_count(workspace).await, 0);
    let history: i64 =
        sqlx::query_scalar("SELECT count(*) FROM tasks.history WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(history, 0);
    assert_eq!(kit.task_count(other).await, 1);
}
