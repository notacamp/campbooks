//! What happened to a conversation, over HTTP.

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    model::InboxRole,
    testkit::{Kit, sorting_deps},
};

fn kinds(history: &Value) -> Vec<&str> {
    history["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bill_reads_as_received_scouts_read_then_pay(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = campbooks_kernel::tenancy::WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(user, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    let id = kit.conversation_of("needs-3").await;
    let response = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/history"),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let history = &response.json;
    assert_eq!(history["conversation_id"], id.as_str());
    assert_eq!(history["truncated"], false);
    let kinds = kinds(history);
    assert_eq!(kinds[0], "received", "{kinds:?}");
    let events = history["events"].as_array().unwrap();
    assert_eq!(
        events[0]["from"]["address"], "billing@acme-hosting.example",
        "{events:?}"
    );
    let at = |kind: &str, question: Option<&str>| {
        events
            .iter()
            .position(|e| e["kind"] == kind && question.is_none_or(|q| e["question"] == q))
            .unwrap_or_else(|| panic!("{kind}: {events:?}"))
    };
    let money = &events[at("scout_read", Some("money"))];
    assert_eq!(money["value"], "owed");
    assert_eq!(money["source"], "model");
    assert_eq!(money["confidence"], 97);
    assert!(
        kinds.iter().filter(|k| **k == "scout_read").count() >= 2,
        "{kinds:?}"
    );
    let need = &events[at("need_opened", None)];
    assert!(
        at("need_opened", None) > at("scout_read", Some("money")),
        "{kinds:?}"
    );
    assert_eq!(need["reason"], "pay");
    assert_eq!(need["basis"], "model");
    assert_eq!(need["open"], true);
    // No message text.
    let text = history.to_string();
    for words in ["Your invoice INV-2041 for", "Hello,"] {
        assert!(!text.contains(words), "{text}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn corrections_say_whether_the_caller_made_them(db: PgPool) {
    let kit = Kit::with_deps(db, sorting_deps()).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Sender).await;
    let id = kit.conversation_of("en-4").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    let moved = kit
        .post(
            &sam,
            &format!("{base}/stream"),
            Some(json!({"stream": "primary"})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.json);
    let moved = kit
        .post(
            &ana,
            &format!("{base}/stream"),
            Some(json!({"stream": "newsletters"})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.json);

    let corrected = |history: &Value| -> Vec<(String, String, String)> {
        history["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "corrected" && e["question"] == "stream")
            .map(|e| {
                (
                    e["from"].as_str().unwrap().to_owned(),
                    e["to"].as_str().unwrap().to_owned(),
                    e["by"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let sams = kit.get(&sam, &format!("{base}/history")).await;
    assert_eq!(sams.status, StatusCode::OK, "{}", sams.json);
    assert_eq!(
        corrected(&sams.json),
        [
            ("updates".into(), "primary".into(), "you".into()),
            ("primary".into(), "newsletters".into(), "member".into()),
        ]
    );
    let anas = kit.get(&ana, &format!("{base}/history")).await;
    assert_eq!(
        corrected(&anas.json),
        [
            ("updates".into(), "primary".into(), "member".into()),
            ("primary".into(), "newsletters".into(), "you".into()),
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rule_applied_then_reverted(db: PgPool) {
    use crate::features::rules::tests::{kit, move_github, sams_notifications};

    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.json);
    kit.run_jobs().await;

    let id = kit.conversation_of("gh-1").await;
    let history = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/history"),
        )
        .await;
    assert_eq!(history.status, StatusCode::OK, "{}", history.json);
    let events = history.json["events"].as_array().unwrap();
    let ruled: Vec<&Value> = events
        .iter()
        .filter(|e| e["kind"].as_str().unwrap().starts_with("rule_"))
        .collect();
    assert_eq!(ruled.len(), 2, "{events:?}");
    assert_eq!(ruled[0]["kind"], "rule_applied");
    assert_eq!(ruled[1]["kind"], "rule_reverted");
    for event in ruled {
        assert_eq!(event["rule"]["id"], rule.as_str());
        assert_eq!(
            event["rule"]["conditions"],
            json!([{"field": "sender_address", "value": crate::features::rules::tests::GITHUB}])
        );
        assert_eq!(
            event["rule"]["actions"],
            json!([{"action": "move_to_stream", "stream": "updates"}])
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_conversation_the_caller_cannot_read_is_not_found(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let (ana, _) = kit.member("ana", workspace);
    let id = kit.conversation_of("en-1").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{id}/history");
    assert_eq!(kit.get(&sam, &uri).await.status, StatusCode::OK);
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
    let client = kit.client(workspace, "conversations:read");
    assert_eq!(kit.get(&client, &uri).await.status, StatusCode::NOT_FOUND);
    let unknown = format!(
        "/api/v1/w/{workspace}/conversations/{}/history",
        crate::model::ConversationId::new()
    );
    assert_eq!(kit.get(&sam, &unknown).await.status, StatusCode::NOT_FOUND);
}
