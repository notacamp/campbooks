//! The trash is kept 30 days.

use campbooks_kernel::{jobs, jobs::EnqueueOptions, tenancy::WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use super::DispatchPurgeTrash;
use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_deleted_over_thirty_days_ago_go_for_good_and_newer_ones_stay(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (bo, _) = kit.member("bo", other);
    let uri = Kit::pages(workspace);
    let old = kit.add(&ana, workspace, json!({"title": "Old"})).await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Old child", "parent": old["id"]}),
    )
    .await;
    let recent = kit.add(&ana, workspace, json!({"title": "Recent"})).await;
    let kept = kit.add(&bo, other, json!({"title": "Bo's"})).await;
    for page in [&old, &recent] {
        let trashed = kit
            .post(
                &ana,
                &format!("{uri}/{}/trash", page["id"].as_str().unwrap()),
                None,
            )
            .await;
        assert_eq!(trashed.status, axum::http::StatusCode::OK);
    }
    kit.post(
        &bo,
        &format!(
            "{}/{}/trash",
            Kit::pages(other),
            kept["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    // "Old" and what went with it were deleted 31 days ago, "Recent" 29.
    sqlx::query(
        "UPDATE pages.pages SET trashed_at = now() - interval '31 days'
          WHERE title IN ('Old', 'Old child')",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE pages.pages SET trashed_at = now() - interval '29 days' WHERE title = 'Recent'",
    )
    .execute(&kit.db)
    .await
    .unwrap();

    // The daily tick, then the jobs it causes.
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchPurgeTrash, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;

    let titles: Vec<String> = sqlx::query_scalar("SELECT title FROM pages.pages ORDER BY title")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert_eq!(titles, ["Bo's", "Recent"]);
    let purged = kit
        .events("pages.page_changed")
        .await
        .into_iter()
        .filter(|event| event["kinds"] == json!(["purge"]))
        .count();
    assert_eq!(purged, 2);
}
