//! "Scout noticed": suggestions from what an inbox does with a sender's
//! mail, answered per member, accepted through a confirmed smart rule.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::Kit,
};

const BUILDS: &str = "builds@ci.example.com";

/// `n` automatic notifications from `from`, one conversation each, a day
/// apart up to yesterday, unread and in the inbox.
fn daily(prefix: &str, from: &str, n: usize) -> Value {
    let now = Utc::now();
    let changes: Vec<Value> = (0..n)
        .map(|i| {
            let days = i64::try_from(n - i).unwrap();
            json!({
                "op": "upsert",
                "message": {
                    "provider_message_id": format!("{prefix}-{i}"),
                    "thread_key": format!("thr-{prefix}-{i}"),
                    "from": {"name": "CI", "address": from},
                    "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                    "subject": format!("Build {i} passed"),
                    "sent_at": (now - TimeDelta::days(days)).to_rfc3339(),
                    "direction": "inbound",
                    "headers": {"auto_submitted": "auto-generated"},
                    "text": "All green.",
                    "mailbox": {"in_inbox": true},
                    "read": false, "starred": false
                }
            })
        })
        .collect();
    json!({"changes": changes})
}

async fn sams_inbox(kit: &Kit) -> (WorkspaceId, InboxId, String) {
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    (workspace, inbox, sam)
}

/// The notices `token` sees, after a pass ran.
async fn noticed(kit: &Kit, token: &str, workspace: WorkspaceId) -> Vec<Value> {
    let uri = format!("/api/v1/w/{workspace}/rule-notices");
    let first = kit.get(token, &uri).await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
    kit.run_jobs().await;
    kit.get(token, &uri).await.json["notices"]
        .as_array()
        .unwrap()
        .clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_never_opened_is_noticed_and_offered_as_a_rule(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;

    let notices = noticed(&kit, &sam, workspace).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
    let notice = &notices[0];
    assert_eq!(notice["kind"], "keep_off_home");
    assert_eq!(notice["sender"], BUILDS);
    assert_eq!(notice["sender_name"], "CI");
    assert_eq!(notice["inbox_id"], json!(inbox));
    assert_eq!(notice["evidence"]["received"], 12);
    assert_eq!(notice["evidence"]["opened"], 0);
    assert_eq!(notice["evidence"]["cadence"], "daily");
    assert_eq!(
        notice["conditions"],
        json!([{"field": "sender_address", "value": BUILDS}])
    );
    assert_eq!(
        notice["actions"][1],
        json!({"action": "surface", "value": "never"})
    );

    // Saying yes, after the member confirmed it: a smart rule from the
    // suggestion, active at once, and the suggestion is gone.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": notice["conditions"],
                "actions": notice["actions"],
                "origin": "suggestion",
                "notice_id": notice["id"],
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["state"], "active");
    assert_eq!(created.json["rule"]["origin"], "suggestion");
    assert_eq!(created.json["acted_without_asking"], false);
    assert!(noticed(&kit, &sam, workspace).await.is_empty());

    // Undo: the rule goes, its mail comes back, and so does the offer.
    let rule = created.json["rule"]["id"].as_str().unwrap();
    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    kit.run_jobs().await;
    let again = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rule-notices"))
        .await;
    assert_eq!(again.json["notices"].as_array().unwrap().len(), 1);

    // Undo of the Undo: the rule is back, and the suggestion with it is
    // accepted again, never a second rule.
    let restored = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}/restore"),
            Some(json!({"apply_to_existing": true})),
        )
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json);
    let after = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rule-notices"))
        .await;
    assert!(after.json["notices"].as_array().unwrap().is_empty());
    let twice = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": notice["conditions"],
                "actions": notice["actions"],
                "origin": "suggestion",
                "notice_id": notice["id"],
            })),
        )
        .await;
    assert_eq!(twice.status, StatusCode::BAD_REQUEST, "{:?}", twice.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_pass_before_the_first_sync_looks_again_once_mail_arrives(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    // Home's first load, before any mail: nothing, and the pass counts only
    // for ten minutes, not a day.
    assert!(noticed(&kit, &sam, workspace).await.is_empty());
    let due_in: f64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM scanned_at + interval '24 hours' - now())::float8
           FROM conversations.rule_notice_scans WHERE inbox_id = $1",
    )
    .bind(inbox)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!((0.0..=600.0).contains(&due_in), "{due_in}");
    // The first sync lands; ten minutes later, Home looks again.
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = scanned_at - interval '11 minutes'
          WHERE inbox_id = $1",
    )
    .bind(inbox)
    .execute(&kit.db)
    .await
    .unwrap();
    let notices = noticed(&kit, &sam, workspace).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_now_and_dont_suggest_this_are_each_members_own(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Sender).await;
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;
    let notices = noticed(&kit, &sam, workspace).await;
    let id = notices[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(notices[0]["shared"], true);
    let answer = format!("/api/v1/w/{workspace}/rule-notices/{id}/answer");
    let list = format!("/api/v1/w/{workspace}/rule-notices");
    let count = |v: &Value| v["notices"].as_array().unwrap().len();

    let not_now = kit
        .post(&sam, &answer, Some(json!({"answer": "not_now"})))
        .await;
    assert_eq!(not_now.status, StatusCode::NO_CONTENT);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 0);
    // Ana did not answer: she still sees it.
    assert_eq!(count(&kit.get(&ana, &list).await.json), 1);

    // Undo brings it back.
    let undone = kit.delete(&sam, &answer).await;
    assert_eq!(undone.status, StatusCode::NO_CONTENT);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 1);

    // For good: a later pass keeps it, but never shows it to Sam again.
    kit.post(&sam, &answer, Some(json!({"answer": "never"})))
        .await;
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = now() - interval '2 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    assert!(noticed(&kit, &sam, workspace).await.is_empty());
    assert_eq!(count(&kit.get(&ana, &list).await.json), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nobody_else_sees_or_answers_an_inboxs_notices(db: PgPool) {
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
    let other_inbox = kit
        .inbox(
            workspace,
            "sam.studio@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;
    let notices = noticed(&kit, &sam, workspace).await;
    let id = notices[0]["id"].as_str().unwrap().to_owned();

    // Leo is in the workspace but not the inbox; Rita only reads it.
    let (leo, _) = kit.member("leo", workspace);
    let (rita, rita_id) = kit.member("rita", workspace);
    kit.add_to_inbox(inbox, rita_id, InboxRole::Reader).await;
    // Another workspace's member asks with the suggestion's id.
    let elsewhere = WorkspaceId::new();
    let (eve, _) = kit.member("eve", elsewhere);
    for token in [&leo, &rita] {
        assert!(noticed(&kit, token, workspace).await.is_empty());
    }
    let answer = |ws: WorkspaceId| format!("/api/v1/w/{ws}/rule-notices/{id}/answer");
    let body = Some(json!({"answer": "never"}));
    assert_eq!(
        kit.post(&leo, &answer(workspace), body.clone())
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        kit.post(&rita, &answer(workspace), body.clone())
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        kit.post(&eve, &answer(elsewhere), body).await.status,
        StatusCode::NOT_FOUND
    );
    // A suggestion of one inbox cannot be accepted in another.
    let crossed = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{other_inbox}/rules"),
            Some(json!({
                "conditions": notices[0]["conditions"],
                "actions": notices[0]["actions"],
                "origin": "suggestion",
                "notice_id": id,
            })),
        )
        .await;
    assert_eq!(
        crossed.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        crossed.json
    );
    assert_eq!(crossed.json["code"], "invalid_rule");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_that_matters_is_not_noticed(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    // Too few to notice.
    kit.ingest_value(workspace, inbox, &daily("few", "alerts@few.example", 4))
        .await;
    // Opened every time.
    kit.ingest_value(workspace, inbox, &daily("read", "news@read.example", 12))
        .await;
    sqlx::query(
        "UPDATE conversations.messages SET read = true WHERE sender_address = 'news@read.example'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE conversations.conversations SET unread_count = 0
          WHERE id IN (SELECT conversation_id FROM conversations.messages
                        WHERE sender_address = 'news@read.example')",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    assert!(noticed(&kit, &sam, workspace).await.is_empty());
}
