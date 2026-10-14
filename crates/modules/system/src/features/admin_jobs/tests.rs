use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use crate::{Deps, testkit::world};

const SECRET: &str = "payload-secret-text";

/// A job row in `state`, whose payload carries [`SECRET`].
async fn job(db: &PgPool, kind: &str, queue: &str, state: &str, error: Option<&str>) {
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, state, max_attempts, attempts,
                                  last_error, finished_at, workspace_id,
                                  locked_by, locked_until)
         VALUES ($1, $2, $3, $4, 3, 3, $5,
                 CASE WHEN $4 IN ('failed', 'succeeded') THEN now() END, 'ws-1',
                 CASE WHEN $4 = 'running' THEN 'w' END,
                 CASE WHEN $4 = 'running' THEN now() + interval '1 minute' END)",
    )
    .bind(queue)
    .bind(kind)
    .bind(json!({ "note": SECRET }))
    .bind(state)
    .bind(error)
    .execute(db)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_jobs_readers_see_the_queue(db: PgPool) {
    let w = world(db, Deps::default()).await;
    for sub in ["member", "fixer"] {
        for uri in ["/admin/v1/system/jobs", "/admin/v1/system/jobs/failed"] {
            let response = w.call(Method::GET, uri, sub, None).await;
            assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub} {uri}");
        }
    }
    assert!(w.trail.0.lock().unwrap().is_empty());
    for sub in ["ada", "ops"] {
        let response = w
            .call(Method::GET, "/admin/v1/system/jobs", sub, None)
            .await;
        assert_eq!(response.status, StatusCode::OK, "{sub}");
    }
    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].operation, "system.admin_jobs");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_summary_counts_without_payloads(db: PgPool) {
    job(&db, "mail.sync", "sync", "pending", None).await;
    job(&db, "mail.sync", "sync", "running", None).await;
    job(&db, "mail.sync", "sync", "failed", Some("boom")).await;
    job(&db, "mail.sync", "sync", "succeeded", None).await;
    job(&db, "ai.read", "ai", "pending", None).await;
    let w = world(db, Deps::default()).await;
    let response = w
        .call(Method::GET, "/admin/v1/system/jobs", "ada", None)
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let kinds = response.json["kinds"].as_array().unwrap();
    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[0]["kind"], "ai.read");
    assert_eq!(kinds[0]["ready"], 1);
    assert_eq!(kinds[1]["queue"], "sync");
    assert_eq!(kinds[1]["running"], 1);
    assert_eq!(kinds[1]["failed"], 1);
    assert_eq!(kinds[1]["ready"], 1);
    assert!(response.json["oldest_ready_seconds"].as_i64().is_some());
    assert!(!response.json.to_string().contains(SECRET));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn failed_jobs_page_and_filter_and_never_show_payloads(db: PgPool) {
    for n in 0..3 {
        job(
            &db,
            "mail.sync",
            "sync",
            "failed",
            Some(&format!("boom {n}")),
        )
        .await;
    }
    job(&db, "ai.read", "ai", "failed", Some("quota")).await;
    job(&db, "ai.read", "ai", "pending", None).await;
    let w = world(db, Deps::default()).await;

    let all = w
        .call(
            Method::GET,
            "/admin/v1/system/jobs/failed?limit=3",
            "ops",
            None,
        )
        .await;
    assert_eq!(all.status, StatusCode::OK, "{}", all.json);
    let items = all.json["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["last_error"], "quota");
    assert_eq!(items[0]["workspace_id"], "ws-1");
    assert_eq!(items[0]["attempts"], 3);
    assert!(items[0].get("payload").is_none());
    let next = all.json["next"].as_str().unwrap().to_owned();
    let rest = w
        .call(
            Method::GET,
            &format!("/admin/v1/system/jobs/failed?limit=3&after={next}"),
            "ops",
            None,
        )
        .await;
    assert_eq!(rest.json["items"].as_array().unwrap().len(), 1);
    assert!(rest.json["next"].is_null());

    let filtered = w
        .call(
            Method::GET,
            "/admin/v1/system/jobs/failed?kind=ai.read",
            "ops",
            None,
        )
        .await;
    assert_eq!(filtered.json["items"].as_array().unwrap().len(), 1);
    let filtered = w
        .call(
            Method::GET,
            "/admin/v1/system/jobs/failed?queue=sync",
            "ops",
            None,
        )
        .await;
    assert_eq!(filtered.json["items"].as_array().unwrap().len(), 3);

    assert!(!all.json.to_string().contains(SECRET));
    assert!(!rest.json.to_string().contains(SECRET));

    let bad = w
        .call(
            Method::GET,
            "/admin/v1/system/jobs/failed?after=junk",
            "ops",
            None,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_query");
}
