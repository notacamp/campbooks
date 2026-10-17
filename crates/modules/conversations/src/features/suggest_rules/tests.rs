//! Tests of rule suggestions ("teach once").

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    ConversationsDeps,
    model::InboxRole,
    testkit::{Kit, notifications},
};

const GITHUB: &str = "notifications@github.com";
const LIST: &str = "acme/web <web.acme.github.com>";

#[sqlx::test(migrations = "../../../migrations")]
async fn moving_one_notification_suggests_moving_every_lookalike(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &notifications("gh", 12, GITHUB, Some(LIST)),
    )
    .await;
    let first = kit.conversation_of("gh-0").await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{first}/stream"),
        Some(json!({"stream": "updates"})),
    )
    .await;

    let uri = format!(
        "/api/v1/w/{workspace}/conversations/{first}/rule-suggestions?action=move_to_stream&stream=updates"
    );
    let offered = kit.get(&sam, &uri).await;
    assert_eq!(offered.status, StatusCode::OK, "{:?}", offered.json);
    assert!(offered.json["none"].is_null(), "{:?}", offered.json);
    assert_eq!(offered.json["default"], 0);
    assert_eq!(offered.json["shared"], false);
    let all = offered.json["suggestions"].as_array().unwrap();
    let likenesses: Vec<&str> = all
        .iter()
        .map(|s| s["likeness"].as_str().unwrap())
        .collect();
    assert_eq!(likenesses, ["sender", "list", "sender_template"]);
    assert_eq!(all[0]["conditions"][0]["value"], GITHUB);
    assert_eq!(
        all[0]["preview"]["would_change"], 11,
        "the one moved already is not counted"
    );
    assert_eq!(all[0]["actions"][0]["stream"], "updates");

    // "Just this one": not offered again.
    let key = all[0]["key"].as_str().unwrap();
    let dismissed = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{first}/rule-suggestions/dismiss"),
            Some(json!({"key": key})),
        )
        .await;
    assert_eq!(dismissed.status, StatusCode::NO_CONTENT);
    let again = kit.get(&sam, &uri).await;
    assert_eq!(again.json["none"], "dismissed");
    assert!(again.json["suggestions"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn people_the_inbox_writes_to_are_never_hidden(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    // Maya wrote, and Sam replied (fixtures/en.json): archiving her mail
    // teaches nothing.
    let maya = kit.conversation_of("en-1").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{maya}/rule-suggestions?action=archive");
    let none = kit.get(&sam, &uri).await;
    assert_eq!(none.status, StatusCode::OK, "{:?}", none.json);
    assert_eq!(none.json["none"], "correspondent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn too_few_lookalikes_and_readers_get_nothing(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let (ana, ana_id) = kit.member("ana", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager), (ana_id, InboxRole::Reader)],
        )
        .await;
    kit.ingest_value(workspace, inbox, &notifications("gh", 2, GITHUB, None))
        .await;
    let first = kit.conversation_of("gh-0").await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{first}/read"),
        None,
    )
    .await;
    let uri =
        format!("/api/v1/w/{workspace}/conversations/{first}/rule-suggestions?action=mark_read");
    let few = kit.get(&sam, &uri).await;
    assert_eq!(few.json["none"], "too_few", "{:?}", few.json);
    assert_eq!(few.json["shared"], true);
    let reader = kit.get(&ana, &uri).await;
    assert_eq!(reader.status, StatusCode::FORBIDDEN);
    assert_eq!(reader.json["code"], "inbox_read_only");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn just_this_one_hides_every_likeness_when_there_is_no_default(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &notifications("gh", 12, GITHUB, Some(LIST)),
    )
    .await;
    // Sam once wrote to the sender: no default, every likeness offered.
    kit.ingest_value(
        workspace,
        inbox,
        &json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "to-gh", "thread_key": "thr-to-gh",
            "from": {"name": "Sam Rivers", "address": "sam@example.com"},
            "to": [{"name": "GitHub", "address": GITHUB}],
            "subject": "Question about notifications", "sent_at": "2026-10-02T09:00:00Z",
            "direction": "outbound", "text": "Hello.",
            "mailbox": {"in_inbox": false}, "read": true, "starred": false
        }}]}),
    )
    .await;
    // Labelling one hides nothing, so a correspondent still gets likenesses.
    let label: String = sqlx::query_scalar(
        "INSERT INTO conversations.labels (id, workspace_id, inbox_id, name)
         VALUES (gen_random_uuid(), $1, $2, 'GitHub') RETURNING id::text",
    )
    .bind(workspace.as_uuid())
    .bind(inbox.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let first = kit.conversation_of("gh-0").await;
    let uri = format!(
        "/api/v1/w/{workspace}/conversations/{first}/rule-suggestions?action=add_label&label={label}"
    );
    let offered = kit.get(&sam, &uri).await;
    assert!(offered.json["default"].is_null(), "{:?}", offered.json);
    let all = offered.json["suggestions"].as_array().unwrap();
    assert!(all.len() > 1, "{:?}", offered.json);
    // The member turned down the last one shown (the client sends them all).
    let key = all.last().unwrap()["key"].as_str().unwrap();
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{first}/rule-suggestions/dismiss"),
        Some(json!({"key": key})),
    )
    .await;
    let again = kit.get(&sam, &uri).await;
    assert_eq!(again.json["none"], "dismissed");
}

/// Mail from `from` with the receiving provider's `results`, one
/// conversation each (`<prefix>-0` …).
fn mail_with_results(prefix: &str, from: &str, results: &str, n: usize) -> serde_json::Value {
    let base = chrono::DateTime::parse_from_rfc3339("2026-10-01T08:00:00Z").unwrap();
    let changes: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let minutes = i64::try_from(i).unwrap();
            json!({
                "op": "upsert",
                "message": {
                    "provider_message_id": format!("{prefix}-{i}"),
                    "thread_key": format!("thr-{prefix}-{i}"),
                    "from": {"name": "PayPal", "address": from},
                    "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                    "subject": format!("Notice {i}"),
                    "sent_at": (base + chrono::TimeDelta::minutes(minutes)).to_rfc3339(),
                    "direction": "inbound",
                    "headers": {"authentication_results": results},
                    "text": "Hello.",
                    "mailbox": {"in_inbox": true},
                    "read": false, "starred": false
                }
            })
        })
        .collect();
    json!({"changes": changes})
}

const FAILED: &str = "mx.google.com; spf=fail smtp.mailfrom=paypal.com; dmarc=fail (p=REJECT) header.from=paypal.com";
const PASSED: &str =
    "mx.google.com; dkim=pass header.i=@paypal.com; dmarc=pass (p=REJECT) header.from=paypal.com";

#[sqlx::test(migrations = "../../../migrations")]
async fn reporting_a_fake_offers_to_report_mail_pretending_to_be_that_sender(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with_results("fake", "service@paypal.com", FAILED, 2),
    )
    .await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with_results("real", "service@paypal.com", PASSED, 1),
    )
    .await;
    let fake = kit.conversation_of("fake-0").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{fake}");
    // Nothing to offer before it is reported.
    let early = kit
        .get(&sam, &format!("{base}/rule-suggestions?action=report"))
        .await;
    assert_eq!(early.json["none"], "not_teachable", "{:?}", early.json);

    kit.post(&sam, &format!("{base}/spam"), None).await;
    let offered = kit
        .get(&sam, &format!("{base}/rule-suggestions?action=report"))
        .await;
    assert_eq!(offered.status, StatusCode::OK, "{:?}", offered.json);
    assert_eq!(offered.json["default"], 0, "{:?}", offered.json);
    let first = &offered.json["suggestions"][0];
    assert_eq!(first["likeness"], "impersonation");
    assert_eq!(
        first["conditions"],
        json!([
            {"field": "sender_domain", "value": "paypal.com"},
            {"field": "sender_checks_fail"}
        ])
    );
    assert_eq!(first["actions"], json!([{"action": "spam"}]));
    // The other fake; never the real PayPal mail, never the whole domain.
    assert_eq!(first["preview"]["would_change"], 1);
    let likenesses: Vec<&str> = offered.json["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["likeness"].as_str().unwrap())
        .collect();
    assert_eq!(likenesses, ["impersonation", "sender"]);

    // Saved, new fakes are reported as they arrive; real mail is not.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": first["conditions"],
                "actions": first["actions"],
                "from_conversation_id": fake,
                "apply_to_existing": false,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with_results("later-fake", "service@paypal.com", FAILED, 1),
    )
    .await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with_results("later-real", "service@paypal.com", PASSED, 1),
    )
    .await;
    let spam: Vec<(String, bool)> = sqlx::query_as(
        "SELECT provider_message_id, in_spam FROM conversations.messages
          WHERE provider_message_id LIKE 'later-%' ORDER BY provider_message_id",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(
        spam,
        [
            ("later-fake-0".to_owned(), true),
            ("later-real-0".to_owned(), false)
        ]
    );
    let requested = kit.events("conversations.changes_requested").await;
    assert!(
        requested
            .iter()
            .flat_map(|e| e["items"].as_array().unwrap().clone())
            .any(|i| i["provider_message_id"] == "later-fake-0" && i["change"]["kind"] == "spam")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn already_paid_offers_to_mark_the_senders_bills_paid(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with_results("bill", "faturas@edp.example", PASSED, 1),
    )
    .await;
    let bill = kit.conversation_of("bill-0").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{bill}/rule-suggestions?action=paid");
    assert_eq!(kit.get(&sam, &uri).await.json["none"], "not_teachable");
    // The member said Already paid on its Pay need.
    sqlx::query(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question,
              state, cleared_because, verdict)
         SELECT c.id, c.workspace_id, m.id, 'pay', 'check', 'model', 'money',
                'cleared', 'done', 'already_paid'
           FROM conversations.conversations c
           JOIN conversations.messages m ON m.conversation_id = c.id
          WHERE c.id = $1::uuid",
    )
    .bind(&bill)
    .execute(&kit.db)
    .await
    .unwrap();
    let offered = kit.get(&sam, &uri).await;
    assert_eq!(offered.status, StatusCode::OK, "{:?}", offered.json);
    assert_eq!(offered.json["default"], 0, "{:?}", offered.json);
    let first = &offered.json["suggestions"][0];
    assert_eq!(first["likeness"], "sender");
    assert_eq!(first["actions"], json!([{"action": "mark_paid"}]));
}
