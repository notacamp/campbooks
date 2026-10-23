//! Making a task.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::{CONVERSATION, INBOX, Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn an_own_task_is_open_do_and_the_creators_and_its_event_has_no_title(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let made = kit
        .post(
            &ana,
            &format!("/api/v1/w/{workspace}/tasks"),
            Some(json!({"title": "  Call the accountant about Q3 VAT ", "due_on": "2026-10-30"})),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    let task = &made.json;
    assert_eq!(task["title"], "Call the accountant about Q3 VAT");
    assert_eq!(task["state"], "open");
    assert_eq!(task["verb"], "do");
    assert_eq!(task["origin"], "own");
    assert_eq!(task["plan"], "none");
    assert_eq!(task["due_on"], "2026-10-30");
    assert_eq!(task["assignee"], ana_id.to_string());
    assert_eq!(task["created_by"], ana_id.to_string());
    assert_eq!(task["revision"], 1);
    assert_eq!(task["source"], serde_json::Value::Null);

    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["change"], "created");
    assert_eq!(events[0]["revision"], 1);
    assert!(!events[0].to_string().contains("accountant"));
    assert!(events[0].get("title").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_blank_title_is_refused_and_nothing_is_stored(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let uri = format!("/api/v1/w/{workspace}/tasks");
    for title in ["   ", "\u{200b}\u{202e}", &"x".repeat(201)] {
        let refused = kit.post(&ana, &uri, Some(json!({"title": title}))).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{title}");
        assert_eq!(refused.json["code"], "invalid_title");
    }
    let no_day = kit
        .post(
            &ana,
            &uri,
            Some(json!({"title": "Call", "due_at": "2026-10-30T15:00:00Z"})),
        )
        .await;
    assert_eq!(no_day.json["code"], "due_time_needs_day");
    assert_eq!(kit.task_count(workspace).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_source_needs_read_access_to_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    kit.let_see(INBOX, ana_id);
    let uri = format!("/api/v1/w/{workspace}/tasks");
    let source = json!({"type": "conversations.conversation", "id": CONVERSATION});

    // João has no role in the inbox: it is not there for him.
    let unseen = kit
        .post(
            &joao,
            &uri,
            Some(json!({"title": "Send the lease", "source": source})),
        )
        .await;
    assert_eq!(unseen.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.task_count(workspace).await, 0);

    let seen = kit
        .post(
            &ana,
            &uri,
            Some(json!({
                "title": "Send the signed lease to Ana",
                "verb": "pay",
                "source": source,
                "source_sender": " Ana.Ribeiro@Example.com ",
            })),
        )
        .await;
    assert_eq!(seen.status, StatusCode::CREATED, "{:?}", seen.json);
    assert_eq!(seen.json["source"], source);
    assert_eq!(seen.json["verb"], "pay");
    let (container, sender): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT source_container, source_sender FROM tasks.tasks WHERE workspace_id = $1",
    )
    .bind(workspace.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(container.as_deref(), Some(INBOX));
    assert_eq!(sender.as_deref(), Some("ana.ribeiro@example.com"));
    // The sender's address is not in the event.
    for event in kit.events("tasks.task_changed").await {
        assert!(!event.to_string().contains("example.com"));
    }

    // Records that are not records are not there either.
    let bogus = kit
        .post(
            &ana,
            &uri,
            Some(json!({"title": "x", "source": {"type": "nope", "id": "1"}})),
        )
        .await;
    assert_eq!(bogus.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn writing_needs_the_write_scope_and_a_member(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, _) = kit.member("ana", workspace);
    let uri = format!("/api/v1/w/{workspace}/tasks");
    let body = Some(json!({"title": "Call"}));
    assert_eq!(
        kit.post("", &uri, body.clone()).await.status,
        StatusCode::UNAUTHORIZED
    );
    let reader = kit.token_with("ana", "tasks:read");
    assert_eq!(
        kit.post(&reader, &uri, body.clone()).await.status,
        StatusCode::FORBIDDEN
    );
    // A client acting for nobody has no tasks of its own.
    let client = kit.client(workspace, "tasks:read tasks:write");
    assert_eq!(
        kit.post(&client, &uri, body).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(kit.task_count(workspace).await, 0);
}
