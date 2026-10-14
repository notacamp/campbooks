use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{Deps, testkit::world};

async fn job(db: &PgPool, kind: &str, state: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO kernel.jobs (queue, kind, payload, state, max_attempts, attempts, finished_at)
         VALUES ('sync', $1, '{}', $2, 3, 3, CASE WHEN $2 = 'failed' THEN now() END)
         RETURNING id",
    )
    .bind(kind)
    .bind(state)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn state_of(db: &PgPool, id: Uuid) -> Option<(String, i32)> {
    sqlx::query_as("SELECT state, attempts FROM kernel.jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn retrying_needs_the_retry_permission(db: PgPool) {
    let id = job(&db, "mail.sync", "failed").await;
    let w = world(db.clone(), Deps::default()).await;
    let uri = format!("/admin/v1/system/jobs/{id}/retry");
    for sub in ["member", "ops"] {
        let response = w.call(Method::POST, &uri, sub, None).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub}");
    }
    assert_eq!(state_of(&db, id).await.unwrap().0, "failed");
    assert!(w.trail.0.lock().unwrap().is_empty());

    let response = w.call(Method::POST, &uri, "fixer", None).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json["outcome"], "applied");
    assert_eq!(state_of(&db, id).await, Some(("pending".into(), 0)));
    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].operation, "system.admin_retry_job");
    assert_eq!(recorded[0].target, json!({ "id": id.to_string() }));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn retrying_twice_or_a_live_job_changes_nothing(db: PgPool) {
    let dead = job(&db, "mail.sync", "failed").await;
    let running = job(&db, "mail.sync", "pending").await;
    let w = world(db.clone(), Deps::default()).await;
    let retry = |id: Uuid| format!("/admin/v1/system/jobs/{id}/retry");
    let first = w.call(Method::POST, &retry(dead), "ada", None).await;
    let again = w.call(Method::POST, &retry(dead), "ada", None).await;
    assert_eq!(first.json["outcome"], "applied");
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json["outcome"], "unchanged");
    let live = w.call(Method::POST, &retry(running), "ada", None).await;
    assert_eq!(live.json["outcome"], "unchanged");
    assert_eq!(state_of(&db, running).await, Some(("pending".into(), 3)));

    let missing = w
        .call(Method::POST, &retry(Uuid::now_v7()), "ada", None)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let junk = w
        .call(
            Method::POST,
            "/admin/v1/system/jobs/not-an-id/retry",
            "ada",
            None,
        )
        .await;
    assert_eq!(junk.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discarding_deletes_only_dead_jobs(db: PgPool) {
    let dead = job(&db, "mail.sync", "failed").await;
    let pending = job(&db, "mail.sync", "pending").await;
    let w = world(db.clone(), Deps::default()).await;
    let uri = |id: Uuid| format!("/admin/v1/system/jobs/{id}");
    assert_eq!(
        w.call(Method::DELETE, &uri(dead), "ops", None).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(state_of(&db, dead).await.is_some());

    let gone = w.call(Method::DELETE, &uri(dead), "fixer", None).await;
    assert_eq!(gone.status, StatusCode::OK, "{}", gone.json);
    assert_eq!(gone.json["outcome"], "applied");
    assert!(state_of(&db, dead).await.is_none());
    let again = w.call(Method::DELETE, &uri(dead), "fixer", None).await;
    assert_eq!(again.json["outcome"], "unchanged");
    let kept = w.call(Method::DELETE, &uri(pending), "fixer", None).await;
    assert_eq!(kept.json["outcome"], "unchanged");
    assert!(state_of(&db, pending).await.is_some());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn retrying_a_kind_requeues_its_dead_jobs(db: PgPool) {
    let a = job(&db, "mail.sync", "failed").await;
    let b = job(&db, "mail.sync", "failed").await;
    let other = job(&db, "ai.read", "failed").await;
    let w = world(db.clone(), Deps::default()).await;
    let uri = "/admin/v1/system/jobs/retry";
    let denied = w
        .call(
            Method::POST,
            uri,
            "ops",
            Some(json!({ "kind": "mail.sync" })),
        )
        .await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);

    let done = w
        .call(
            Method::POST,
            uri,
            "fixer",
            Some(json!({ "kind": "mail.sync" })),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.json);
    assert_eq!(done.json, json!({ "requeued": 2, "more": false }));
    assert_eq!(state_of(&db, a).await.unwrap().0, "pending");
    assert_eq!(state_of(&db, b).await.unwrap().0, "pending");
    assert_eq!(state_of(&db, other).await.unwrap().0, "failed");
    let again = w
        .call(
            Method::POST,
            uri,
            "fixer",
            Some(json!({ "kind": "mail.sync" })),
        )
        .await;
    assert_eq!(again.json["requeued"], 0);

    let bad = w
        .call(
            Method::POST,
            uri,
            "fixer",
            Some(json!({ "kind": "x", "extra": 1 })),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}
