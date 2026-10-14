//! Tests of rule runs: counting then applying, and runs whose rule or inbox
//! goes away.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    features::rules::tests::{GITHUB, kit, move_github, sams_notifications, streams},
    model::InboxRole,
    testkit::notifications,
};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_counts_in_the_background_then_applies(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 30).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    // The request counted nothing: the run does.
    assert_eq!(created.json["rule"]["run"]["counted"], false);
    assert_eq!(created.json["rule"]["run"]["total"], 0);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["run"]["counted"], true);
    assert_eq!(got.json["run"]["total"], 30);
    assert_eq!(got.json["run"]["done"], 30);
    assert_eq!(got.json["run"]["state"], "done");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_stops_when_its_author_loses_the_inbox(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, _, _) = sams_notifications(&kit, 5).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let created = kit
        .post(
            &leo,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    // Leo leaves before the run's first batch.
    sqlx::query("DELETE FROM conversations.inbox_members WHERE user_id = $1")
        .bind(leo_id.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    kit.run_jobs().await;
    assert!(streams(&kit).await.iter().all(|(_, s, _)| s == "primary"));
    let state: String = sqlx::query_scalar("SELECT state FROM conversations.rule_runs")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(state, "cancelled");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_whose_inbox_goes_away_ends_quietly(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 5).await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
        Some(move_github()),
    )
    .await;
    sqlx::query("DELETE FROM conversations.inboxes WHERE id = $1")
        .bind(inbox.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    // No job fails (run_jobs asserts it).
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_under_review_is_never_hidden_on_arrival(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    // gh-0's conversation has an open Review need (a question to the member).
    sqlx::query(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question, state)
         SELECT conversation_id, workspace_id, id, 'review', 'check', 'model', 'needs_reply', 'open'
           FROM conversations.messages WHERE provider_message_id = 'gh-0'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let archive = json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "archive"}],
        "apply_to_existing": false,
    });
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
        Some(archive),
    )
    .await;
    // A follow-up arrives in the same conversation.
    let mut follow_up = notifications("gh-0-reply", 1, GITHUB, None);
    follow_up["changes"][0]["message"]["thread_key"] = json!("thr-gh-0");
    kit.ingest_value(workspace, inbox, &follow_up).await;
    let in_inbox: bool = sqlx::query_scalar(
        "SELECT in_inbox FROM conversations.messages WHERE provider_message_id = 'gh-0-reply-0'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(in_inbox);
}
