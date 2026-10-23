//! Done, not needed and reopened.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn done_then_undo_leaves_it_open_with_its_plan(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit
        .add(&ana, workspace, json!({"title": "Send the signed lease"}))
        .await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let until = Utc::now() + TimeDelta::days(3);
    kit.post(
        &ana,
        &format!("{base}/plan"),
        Some(json!({"revision": 1, "plan": "later", "later_until": until})),
    )
    .await;

    let done = kit
        .post(&ana, &format!("{base}/done"), Some(json!({"revision": 2})))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert_eq!(done.json["state"], "done");
    assert!(done.json["closed_at"].is_string());
    // Closed: it cannot be closed again, nor marked not needed.
    let again = kit
        .post(
            &ana,
            &format!("{base}/not_needed"),
            Some(json!({"revision": 3})),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "task_closed");

    let reopened = kit
        .post(
            &ana,
            &format!("{base}/reopen"),
            Some(json!({"revision": 3})),
        )
        .await;
    assert_eq!(reopened.json["state"], "open");
    assert_eq!(reopened.json["closed_at"], serde_json::Value::Null);
    assert_eq!(reopened.json["plan"], "later");
    let open_again = kit
        .post(
            &ana,
            &format!("{base}/reopen"),
            Some(json!({"revision": 4})),
        )
        .await;
    assert_eq!(open_again.json["code"], "task_open");

    let detail = kit.get(&ana, &base).await;
    let kinds: Vec<&str> = detail.json["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["what"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["created", "planned", "done", "reopened"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_tabs_the_older_view_conflicts(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let planned = kit
        .post(
            &ana,
            &format!("{base}/plan"),
            Some(json!({"revision": 1, "plan": "date", "plan_on": "2026-10-29"})),
        )
        .await;
    assert_eq!(planned.status, StatusCode::OK);
    let stale = kit
        .post(&ana, &format!("{base}/done"), Some(json!({"revision": 1})))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "task_changed");
    assert_eq!(kit.get(&ana, &base).await.json["task"]["state"], "open");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_needed_closes_it_and_a_read_only_token_cannot(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let reader = kit.token_with("ana", "tasks:read");
    let refused = kit
        .post(
            &reader,
            &format!("{base}/done"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    let gone = kit
        .post(
            &ana,
            &format!("{base}/not_needed"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(gone.json["state"], "not_needed");
    let open = kit.get(&ana, &format!("/api/v1/w/{workspace}/tasks")).await;
    assert_eq!(open.json["tasks"].as_array().unwrap().len(), 0);
}
