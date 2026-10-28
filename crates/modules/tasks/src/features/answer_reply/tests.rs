//! Not yet, its undo, and the question coming back after a newer reply.

use axum::http::StatusCode;
use campbooks_kernel::{tenancy::WorkspaceId, testing};
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::{
    inbound::MemberReplied,
    testkit::{CONVERSATION, INBOX, Kit},
};

/// A task of Ana's from the email, replied to: its URI.
async fn asking(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    owner: campbooks_kernel::tenancy::UserId,
) -> String {
    let made = kit
        .add(
            token,
            workspace,
            json!({"title": "Send the lease",
                   "source": {"type": "conversations.conversation", "id": CONVERSATION}}),
        )
        .await;
    reply(kit, workspace, owner, 1).await;
    format!(
        "/api/v1/w/{workspace}/tasks/{}",
        made["id"].as_str().unwrap()
    )
}

/// The member replies, `hours` from now.
async fn reply(
    kit: &Kit,
    workspace: WorkspaceId,
    owner: campbooks_kernel::tenancy::UserId,
    hours: i64,
) {
    kit.publish(
        workspace,
        &MemberReplied {
            workspace: workspace.to_string(),
            inbox: INBOX.to_owned(),
            conversation: CONVERSATION.to_owned(),
            message: Uuid::now_v7(),
            sent_at: Utc::now() + TimeDelta::hours(hours),
            owner,
        },
    )
    .await;
}

async fn fetch(kit: &Kit, token: &str, uri: &str) -> Value {
    kit.get(token, uri).await.json["task"].clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_yet_stops_the_question_and_undo_asks_it_again(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let uri = asking(&kit, &ana, workspace, ana_id).await;
    let asked = fetch(&kit, &ana, &uri).await;
    assert_eq!(asked["asks_done"], true);

    let answered = kit
        .post(
            &ana,
            &format!("{uri}/reply_answered"),
            Some(json!({"revision": asked["revision"]})),
        )
        .await;
    assert_eq!(answered.status, StatusCode::OK, "{:?}", answered.json);
    assert_eq!(answered.json["asks_done"], false);
    assert_eq!(answered.json["state"], "open");
    // The reply is still on record, for the detail's "Your reply".
    assert!(answered.json["replied"].is_object());
    assert_eq!(
        testing::tracked_events(&kit.db, "tasks.task_reply_answered")
            .await
            .len(),
        1
    );

    // Answered already: nothing to answer, nothing to take twice.
    let again = kit
        .post(
            &ana,
            &format!("{uri}/reply_answered"),
            Some(json!({"revision": answered.json["revision"]})),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "no_reply_question");

    let undone = kit
        .post(
            &ana,
            &format!("{uri}/reply_unanswered"),
            Some(json!({"revision": answered.json["revision"]})),
        )
        .await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json["asks_done"], true);
    let whats: Vec<String> = kit.get(&ana, &uri).await.json["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["what"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        whats,
        ["created", "replied", "reply_answered", "reply_unanswered"]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_newer_reply_asks_again_after_not_yet(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let uri = asking(&kit, &ana, workspace, ana_id).await;
    let asked = fetch(&kit, &ana, &uri).await;
    // Not yet is stamped "now", which is before the next reply below.
    kit.post(
        &ana,
        &format!("{uri}/reply_answered"),
        Some(json!({"revision": asked["revision"]})),
    )
    .await;
    assert_eq!(fetch(&kit, &ana, &uri).await["asks_done"], false);

    reply(&kit, workspace, ana_id, 5).await;
    assert_eq!(fetch(&kit, &ana, &uri).await["asks_done"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn done_answers_it_and_a_task_that_asks_nothing_refuses_not_yet(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let uri = asking(&kit, &ana, workspace, ana_id).await;
    let asked = fetch(&kit, &ana, &uri).await;

    // A stale revision is refused like every write.
    let stale = kit
        .post(
            &ana,
            &format!("{uri}/reply_answered"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "task_changed");

    let done = kit
        .post(
            &ana,
            &format!("{uri}/done"),
            Some(json!({"revision": asked["revision"]})),
        )
        .await;
    assert_eq!(done.json["state"], "done");
    assert_eq!(done.json["asks_done"], false);
    let closed = kit
        .post(
            &ana,
            &format!("{uri}/reply_answered"),
            Some(json!({"revision": done.json["revision"]})),
        )
        .await;
    assert_eq!(closed.status, StatusCode::CONFLICT);
    assert_eq!(closed.json["code"], "task_closed");

    // A task nobody replied to asks nothing.
    let plain = kit.add(&ana, workspace, json!({"title": "Own task"})).await;
    let none = kit
        .post(
            &ana,
            &format!(
                "/api/v1/w/{workspace}/tasks/{}/reply_answered",
                plain["id"].as_str().unwrap()
            ),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(none.status, StatusCode::CONFLICT);
    assert_eq!(none.json["code"], "no_reply_question");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_task_that_is_not_yours_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    kit.let_see(INBOX, ana_id);
    let uri = asking(&kit, &ana, workspace, ana_id).await;
    let asked = fetch(&kit, &ana, &uri).await;
    let refused = kit
        .post(
            &joao,
            &format!("{uri}/reply_answered"),
            Some(json!({"revision": asked["revision"]})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
}
