use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    Deps,
    testkit::{tracked_world, world},
};

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

/// A row holding schedule `name`'s unique key, as the database owner writes
/// it: `state`, due at `run_at`, leased for `leased_secs`, of `workspace`.
async fn schedule_row(
    db: &PgPool,
    name: &str,
    state: &str,
    run_at: &str,
    leased_secs: Option<f64>,
    workspace: Option<&str>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, unique_key, state, run_at,
                                  locked_by, locked_until, started_at, max_attempts)
         VALUES ('maintenance', $1, 'null', $5, 'schedule:' || $1, $2, $3::timestamptz,
                 CASE WHEN $4::float8 IS NOT NULL THEN 'w-1' END,
                 now() + make_interval(secs => $4::float8),
                 CASE WHEN $4::float8 IS NOT NULL THEN now() END, 3)
         RETURNING id",
    )
    .bind(name)
    .bind(state)
    .bind(run_at)
    .bind(leased_secs)
    .bind(workspace)
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discarding_a_schedule_run_clears_only_what_blocks_its_ticks(db: PgPool) {
    let w = world(db.clone(), Deps::default()).await;
    let uri = |name: &str| format!("/admin/v1/system/schedules/{name}/run");
    // A forged pending row due never (campbooks-cloud#207).
    let stuck = schedule_row(&db, "kernel.prune_jobs", "pending", "infinity", None, None).await;
    let denied = w
        .call(Method::DELETE, &uri("kernel.prune_jobs"), "ops", None)
        .await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    assert!(state_of(&db, stuck).await.is_some());
    let gone = w
        .call(Method::DELETE, &uri("kernel.prune_jobs"), "fixer", None)
        .await;
    assert_eq!(gone.status, StatusCode::OK, "{}", gone.json);
    assert_eq!(gone.json["outcome"], "applied");
    assert!(state_of(&db, stuck).await.is_none());
    let again = w
        .call(Method::DELETE, &uri("kernel.prune_jobs"), "fixer", None)
        .await;
    assert_eq!(again.json["outcome"], "unchanged");
    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(
        recorded.last().unwrap().operation,
        "system.admin_discard_schedule_run"
    );

    // A run a live worker holds is kept; one leased forever is not.
    let live = schedule_row(
        &db,
        "kernel.prune_events",
        "running",
        "now",
        Some(30.0),
        None,
    )
    .await;
    let kept = w
        .call(Method::DELETE, &uri("kernel.prune_events"), "fixer", None)
        .await;
    assert_eq!(kept.json["outcome"], "unchanged");
    assert!(state_of(&db, live).await.is_some());
    sqlx::query("UPDATE kernel.jobs SET locked_until = 'infinity' WHERE id = $1")
        .bind(live)
        .execute(&db)
        .await
        .unwrap();
    let cleared = w
        .call(Method::DELETE, &uri("kernel.prune_events"), "fixer", None)
        .await;
    assert_eq!(cleared.json["outcome"], "applied");
    assert!(state_of(&db, live).await.is_none());

    // Not a schedule: nothing to name.
    let other = w
        .call(Method::DELETE, &uri("mail.sync"), "fixer", None)
        .await;
    assert_eq!(other.status, StatusCode::NOT_FOUND);
}

// --- tracked events (ADR 0020, campbooks-cloud#330) -------------------------

/// The `kernel.*` events named `name`, each version 1, instance-wide, from
/// the administrator `sub`'s request to `operation`: ids, codes and counts,
/// never a payload or an error.
async fn by_admin(
    db: &PgPool,
    name: &str,
    sub: &str,
    operation: &str,
) -> Vec<campbooks_kernel::testing::TrackedRow> {
    let events = campbooks_kernel::testing::tracked_events(db, name).await;
    for event in &events {
        assert_eq!(event.version, 1, "{name}");
        assert_eq!(event.workspace_id, None, "{name}");
        assert_eq!(
            event.envelope["actor"],
            json!({"type": "admin", "id": sub}),
            "{name}"
        );
        assert_eq!(event.envelope["operation"], operation, "{name}");
        assert_eq!(event.envelope["workspace_id"], serde_json::Value::Null);
        let body = event.body.to_string();
        assert!(
            !body.contains("secret") && !body.contains("boom") && !body.contains('@'),
            "{name}: {body}"
        );
    }
    events
}

/// A dead job of `kind` in `workspace`.
async fn dead_in(db: &PgPool, kind: &str, workspace: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, state, max_attempts,
                                  attempts, finished_at, last_error)
         VALUES ('sync', $1, '{\"secret\": 1}', $2, 'failed', 3, 3, now(), 'boom at marta@example.com')
         RETURNING id",
    )
    .bind(kind)
    .bind(workspace)
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn retrying_and_discarding_a_job_are_tracked_once(db: PgPool) {
    let workspace = Uuid::now_v7().to_string();
    let dead = dead_in(&db, "mail.sync", &workspace).await;
    let doomed = job(&db, "mail.sync", "failed").await;
    let w = tracked_world(db.clone(), Deps::default()).await;

    let retry = format!("/admin/v1/system/jobs/{dead}/retry");
    for _ in 0..2 {
        let response = w.call(Method::POST, &retry, "fixer", None).await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    }
    let retried = by_admin(&db, "kernel.job_retried", "fixer", "system.admin_retry_job").await;
    assert_eq!(retried.len(), 1, "retrying again changes nothing");
    let job = &retried[0].body["job"];
    assert_eq!(job["id"], dead.to_string());
    assert_eq!(job["kind"], "mail.sync");
    assert_eq!(job["queue"], "sync");
    assert_eq!(job["workspace_id"], workspace);
    assert_eq!(job["state"], "pending");
    assert_eq!(job["attempts"], 0);
    assert_eq!(job["max_attempts"], 3);

    let uri = format!("/admin/v1/system/jobs/{doomed}");
    for _ in 0..2 {
        let response = w.call(Method::DELETE, &uri, "fixer", None).await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    }
    // A job that is not dead is kept, and nothing is tracked.
    let kept = w
        .call(
            Method::DELETE,
            &format!("/admin/v1/system/jobs/{dead}"),
            "fixer",
            None,
        )
        .await;
    assert_eq!(kept.json["outcome"], "unchanged");
    let discarded = by_admin(
        &db,
        "kernel.job_discarded",
        "fixer",
        "system.admin_discard_job",
    )
    .await;
    assert_eq!(discarded.len(), 1);
    let job = &discarded[0].body["job"];
    assert_eq!(job["id"], doomed.to_string());
    assert_eq!(job["state"], "failed");
    assert_eq!(job["attempts"], 3);
    assert!(job.get("workspace_id").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn retrying_a_kind_is_one_summary(db: PgPool) {
    for _ in 0..3 {
        job(&db, "mail.sync", "failed").await;
    }
    let w = tracked_world(db.clone(), Deps::default()).await;
    let uri = "/admin/v1/system/jobs/retry";
    for _ in 0..2 {
        let response = w
            .call(
                Method::POST,
                uri,
                "ada",
                Some(json!({ "kind": "mail.sync", "queue": "sync" })),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    }
    let none = w
        .call(
            Method::POST,
            uri,
            "ada",
            Some(json!({ "kind": "nothing.here" })),
        )
        .await;
    assert_eq!(none.json["requeued"], 0);
    let events = by_admin(&db, "kernel.jobs_retried", "ada", "system.admin_retry_jobs").await;
    assert_eq!(events.len(), 1, "none requeued tracks nothing");
    assert_eq!(
        events[0].body,
        json!({ "kind": "mail.sync", "queue": "sync", "requeued": 3, "more": false })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discarding_a_schedule_run_is_tracked(db: PgPool) {
    let w = tracked_world(db.clone(), Deps::default()).await;
    let stuck = schedule_row(&db, "kernel.prune_jobs", "pending", "infinity", None, None).await;
    let uri = "/admin/v1/system/schedules/kernel.prune_jobs/run";
    for _ in 0..2 {
        let response = w.call(Method::DELETE, uri, "fixer", None).await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    }
    let events = by_admin(
        &db,
        "kernel.schedule_run_discarded",
        "fixer",
        "system.admin_discard_schedule_run",
    )
    .await;
    assert_eq!(events.len(), 1, "nothing to discard tracks nothing");
    let job = &events[0].body["job"];
    assert_eq!(job["id"], stuck.to_string());
    assert_eq!(job["kind"], "kernel.prune_jobs");
    assert_eq!(job["queue"], "maintenance");
    assert_eq!(job["state"], "pending");
}
