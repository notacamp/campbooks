//! Handing a task to a teammate, and taking it back.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn hand_off_moves_it_and_the_giver_can_take_it_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, joao_id) = kit.member("joao", workspace);
    let (eve, _) = kit.member("eve", workspace);
    let task = kit
        .add(&ana, workspace, json!({"title": "Book the venue"}))
        .await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );

    let handed = kit
        .post(
            &ana,
            &format!("{base}/hand_off"),
            Some(json!({"revision": 1, "to": joao_id})),
        )
        .await;
    assert_eq!(handed.status, StatusCode::OK, "{:?}", handed.json);
    assert_eq!(handed.json["assignee"], joao_id.to_string());
    assert_eq!(handed.json["handed_by"], ana_id.to_string());
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events[1]["change"], "handed_off");
    assert_eq!(events[1]["previous_assignee"], ana_id.to_string());

    // Ana still reads it but no longer changes it; a third member sees nothing.
    assert_eq!(kit.get(&ana, &base).await.status, StatusCode::OK);
    let nudge = kit
        .post(&ana, &format!("{base}/done"), Some(json!({"revision": 2})))
        .await;
    assert_eq!(nudge.status, StatusCode::FORBIDDEN);
    assert_eq!(nudge.json["code"], "task_not_yours");
    assert_eq!(kit.get(&eve, &base).await.status, StatusCode::NOT_FOUND);
    let stolen = kit
        .post(
            &eve,
            &format!("{base}/take_back"),
            Some(json!({"revision": 2})),
        )
        .await;
    assert_eq!(stolen.status, StatusCode::NOT_FOUND);
    // João cannot "take back" what he was given.
    let by_joao = kit
        .post(
            &joao,
            &format!("{base}/take_back"),
            Some(json!({"revision": 2})),
        )
        .await;
    assert_eq!(by_joao.status, StatusCode::FORBIDDEN);

    let back = kit
        .post(
            &ana,
            &format!("{base}/take_back"),
            Some(json!({"revision": 2})),
        )
        .await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(back.json["assignee"], ana_id.to_string());
    assert_eq!(back.json["handed_by"], serde_json::Value::Null);
    assert_eq!(kit.get(&joao, &base).await.status, StatusCode::NOT_FOUND);
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events[2]["change"], "taken_back");
    assert_eq!(events[2]["previous_assignee"], joao_id.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_member_of_the_workspace_can_be_handed_a_task(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let base = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let stranger = kit
        .post(
            &ana,
            &format!("{base}/hand_off"),
            Some(json!({"revision": 1, "to": UserId::new()})),
        )
        .await;
    assert_eq!(stranger.status, StatusCode::BAD_REQUEST);
    assert_eq!(stranger.json["code"], "not_a_member");
    let myself = kit
        .post(
            &ana,
            &format!("{base}/hand_off"),
            Some(json!({"revision": 1, "to": ana_id})),
        )
        .await;
    assert_eq!(myself.json["code"], "hand_off_yourself");
    let not_handed = kit
        .post(
            &ana,
            &format!("{base}/take_back"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(not_handed.status, StatusCode::CONFLICT);
    assert_eq!(not_handed.json["code"], "not_handed_off");
}
