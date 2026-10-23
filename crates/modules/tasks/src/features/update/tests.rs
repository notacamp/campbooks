//! Changing a task's title, verb and due date.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn an_edit_bumps_the_revision_and_undoes_with_the_old_values(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Renew the permit", "due_on": "2026-10-30"}),
        )
        .await;
    let uri = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );

    let edited = kit
        .patch(
            &ana,
            &uri,
            json!({"revision": 1, "title": "Renew the parking permit", "verb": "pay", "due_on": null}),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.json);
    assert_eq!(edited.json["title"], "Renew the parking permit");
    assert_eq!(edited.json["verb"], "pay");
    assert_eq!(edited.json["due_on"], serde_json::Value::Null);
    assert_eq!(edited.json["revision"], 2);

    let undone = kit
        .patch(
            &ana,
            &uri,
            json!({"revision": 2, "title": "Renew the permit", "verb": "do", "due_on": "2026-10-30"}),
        )
        .await;
    assert_eq!(undone.json["title"], "Renew the permit");
    assert_eq!(undone.json["due_on"], "2026-10-30");
    assert_eq!(undone.json["revision"], 3);
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events.len(), 3);
    assert_eq!(events[1]["change"], "edited");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_revision_conflicts_and_nothing_changes(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let uri = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let body =
        |revision: i64, title: &str| json!({"revision": revision, "title": title, "verb": "do"});
    assert_eq!(
        kit.patch(&ana, &uri, body(1, "Call Ana")).await.status,
        StatusCode::OK
    );
    let stale = kit.patch(&ana, &uri, body(1, "Call João")).await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "task_changed");
    let blank = kit.patch(&ana, &uri, body(2, "  ")).await;
    assert_eq!(blank.json["code"], "invalid_title");
    let now = kit.get(&ana, &uri).await;
    assert_eq!(now.json["task"]["title"], "Call Ana");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_assignee_changes_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let uri = format!(
        "/api/v1/w/{workspace}/tasks/{}",
        task["id"].as_str().unwrap()
    );
    let body = json!({"revision": 1, "title": "Hijacked", "verb": "do"});
    assert_eq!(
        kit.patch(&joao, &uri, body).await.status,
        StatusCode::NOT_FOUND
    );
    let reader = kit.token_with("ana", "tasks:read");
    let body = json!({"revision": 1, "title": "Read only", "verb": "do"});
    assert_eq!(
        kit.patch(&reader, &uri, body).await.status,
        StatusCode::FORBIDDEN
    );
}
