//! Reading a task and its history.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn the_history_lists_what_changed_and_by_whom(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, joao_id) = kit.member("joao", workspace);
    let task = kit
        .add(&ana, workspace, json!({"title": "Book the car inspection"}))
        .await;
    let id = task["id"].as_str().unwrap();
    let base = format!("/api/v1/w/{workspace}/tasks/{id}");
    let planned = kit
        .post(
            &ana,
            &format!("{base}/plan"),
            Some(json!({"revision": 1, "plan": "date", "plan_on": "2026-10-29"})),
        )
        .await;
    assert_eq!(planned.status, StatusCode::OK, "{:?}", planned.json);
    let handed = kit
        .post(
            &ana,
            &format!("{base}/hand_off"),
            Some(json!({"revision": 2, "to": joao_id})),
        )
        .await;
    assert_eq!(handed.status, StatusCode::OK, "{:?}", handed.json);

    // The assignee and the member who handed it off both read it.
    let joao = kit.token_with("joao", "tasks:read tasks:write");
    for token in [&ana, &joao] {
        let detail = kit.get(token, &base).await;
        assert_eq!(detail.status, StatusCode::OK, "{:?}", detail.json);
        let kinds: Vec<&str> = detail.json["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["what"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["created", "planned", "handed_off"]);
        assert_eq!(detail.json["history"][0]["actor"], "member");
        assert_eq!(
            detail.json["history"][0]["actor_member"],
            ana_id.to_string()
        );
        assert_eq!(detail.json["task"]["handed_by"], ana_id.to_string());
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_task_that_is_not_yours_is_not_there(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Private"})).await;
    let uri = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    assert_eq!(kit.get(&joao, &uri).await.status, StatusCode::NOT_FOUND);
    // A client acting for nobody does not see it either.
    let client = kit.client(workspace, "tasks:read");
    assert_eq!(kit.get(&client, &uri).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::OK);
    // Not an id at all.
    let nothing = kit
        .get(&ana, &format!("/api/v1/w/{workspace}/tasks/not-an-id"))
        .await;
    assert_eq!(nothing.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn overdue_follows_the_callers_day_on_a_read_and_on_a_write(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    // The day before the day it already is at UTC+14:00: late there, not at
    // UTC-14:00, whatever time the test runs.
    let due = (chrono::Utc::now() + chrono::TimeDelta::hours(14)).date_naive()
        - chrono::TimeDelta::days(1);
    let task = kit
        .add(&ana, workspace, json!({"title": "Pay", "due_on": due}))
        .await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let early = kit.get(&ana, &format!("{base}?utc_offset=-840")).await;
    assert_eq!(early.json["task"]["overdue"], false);
    let late = kit.get(&ana, &format!("{base}?utc_offset=840")).await;
    assert_eq!(late.json["task"]["overdue"], true);
    let refused = kit.get(&ana, &format!("{base}?utc_offset=900")).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "invalid_offset");

    // A write answers in the same day.
    let planned = kit
        .post(
            &ana,
            &format!("{base}/plan?utc_offset=840"),
            Some(json!({"revision": 1, "plan": "none"})),
        )
        .await;
    assert_eq!(planned.status, StatusCode::OK, "{:?}", planned.json);
    assert_eq!(planned.json["overdue"], true);
    let planned = kit
        .post(
            &ana,
            &format!("{base}/plan?utc_offset=-840"),
            Some(json!({"revision": 2, "plan": "none"})),
        )
        .await;
    assert_eq!(planned.json["overdue"], false);
}
