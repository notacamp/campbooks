//! Tests of the rule operations, their runs, rules on arrival and exceptions.

use std::sync::Arc;

use axum::http::StatusCode;
use campbooks_kernel::{
    preferences::{MemberPreferences, PreferenceFuture},
    tenancy::{UserId, WorkspaceId},
};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::{Kit, notifications},
};

const GITHUB: &str = "notifications@github.com";

/// A workspace with Sam's inbox holding `n` GitHub notifications and one
/// message from a person; nothing sorted (the shipped policy).
async fn sams_notifications(kit: &Kit, n: usize) -> (WorkspaceId, InboxId, String, UserId) {
    let workspace = WorkspaceId::new();
    let (token, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest_value(workspace, inbox, &notifications("gh", n, GITHUB, None))
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "marta-1", "thread_key": "thr-marta",
            "from": {"name": "Marta Ferreira", "address": "marta@example.com"},
            "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
            "subject": "Thursday's walkthrough", "sent_at": "2026-10-02T09:00:00Z",
            "direction": "inbound", "text": "See you Thursday.",
            "mailbox": {"in_inbox": true}, "read": false, "starred": false
        }}]}),
    )
    .await;
    (workspace, inbox, token, sam)
}

async fn kit(db: PgPool) -> Kit {
    Kit::with_deps(db, ConversationsDeps::offline()).await
}

fn move_github() -> Value {
    json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "move_to_stream", "stream": "updates"}],
    })
}

async fn streams(kit: &Kit) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT m.provider_message_id, c.stream, c.stream_source
           FROM conversations.conversations c
           JOIN conversations.messages m ON m.conversation_id = c.id
          ORDER BY m.provider_message_id",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn teaching_once_moves_every_lookalike_in_batches(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 250).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");

    let preview = kit
        .post(&sam, &format!("{rules}/preview"), Some(move_github()))
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{:?}", preview.json);
    assert_eq!(preview.json["matching"], 250);
    assert_eq!(preview.json["would_change"], 250);
    assert_eq!(preview.json["samples"].as_array().unwrap().len(), 5);
    assert_eq!(
        preview.json["samples"][0]["subject"], "[acme/web] Pull request #4249 merged",
        "newest first"
    );

    let created = kit.post(&sam, &rules, Some(move_github())).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["state"], "active");
    assert_eq!(created.json["rule"]["origin"], "action");
    assert_eq!(created.json["rule"]["run"]["total"], 250);
    assert_eq!(created.json["acted_without_asking"], false);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;

    let placed = streams(&kit).await;
    let moved: Vec<_> = placed
        .iter()
        .filter(|(id, ..)| id.starts_with("gh-"))
        .collect();
    assert_eq!(moved.len(), 250);
    assert!(
        moved
            .iter()
            .all(|(_, s, src)| s == "updates" && src == "taught")
    );
    let marta = placed.iter().find(|(id, ..)| id == "marta-1").unwrap();
    assert_eq!((marta.1.as_str(), marta.2.as_str()), ("primary", "default"));

    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["applied_count"], 250);
    assert_eq!(got.json["run"]["state"], "done");
    assert_eq!(got.json["run"]["done"], 250);
    assert_eq!(got.json["run"]["changed"], 250);
    // Three batches of at most 100, each announced.
    let progress: Vec<i64> = kit
        .events("conversations.rule_changed")
        .await
        .iter()
        .filter_map(|e| e["run"]["done"].as_i64())
        .collect();
    assert_eq!(progress, vec![0, 100, 200, 250]);
    // The provider is asked to move each one.
    let requested = kit.events("conversations.changes_requested").await;
    assert_eq!(requested.len(), 250);
    assert!(
        requested
            .iter()
            .all(|e| e["items"][0]["change"]["kind"] == "move_to_stream")
    );

    // A summary names the rule.
    let id = kit.conversation_of("gh-7").await;
    let one = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/conversations/{id}"))
        .await;
    assert_eq!(one.json["conversation"]["rule"]["id"], rule.as_str());
    assert_eq!(one.json["conversation"]["rule"]["applied_count"], 250);
    assert!(one.json["conversation"]["suggestion"].is_null());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn new_mail_is_handled_on_arrival_and_runs_first(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let body = json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [
            {"action": "move_to_stream", "stream": "updates"},
            {"action": "mark_read"},
            {"action": "needs_reply", "value": false}
        ],
        "apply_to_existing": false,
    });
    let created = kit.post(&sam, &rules, Some(body)).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    kit.run_jobs().await;
    assert!(
        streams(&kit)
            .await
            .iter()
            .filter(|(id, ..)| id.starts_with("gh-"))
            .all(|(_, s, _)| s == "primary")
    );

    kit.ingest_value(workspace, inbox, &notifications("late", 1, GITHUB, None))
        .await;
    let placed = streams(&kit).await;
    let late = placed.iter().find(|(id, ..)| id == "late-0").unwrap();
    assert_eq!((late.1.as_str(), late.2.as_str()), ("updates", "taught"));
    let (read, decision): (bool, String) = sqlx::query_as(
        "SELECT m.read, d.source FROM conversations.messages m
           JOIN conversations.decisions d ON d.message_id = m.id AND d.question = 'stream'
                                        AND d.source = 'taught'
          WHERE m.provider_message_id = 'late-0'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(read);
    assert_eq!(decision, "taught");
    let requested: Vec<String> = kit
        .events("conversations.changes_requested")
        .await
        .iter()
        .map(|e| e["items"][0]["change"]["kind"].as_str().unwrap().to_owned())
        .collect();
    assert!(requested.contains(&"move_to_stream".to_owned()));
    assert!(requested.contains(&"mark_read".to_owned()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn suspicious_mail_and_hand_placed_mail_are_left_alone(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 5).await;
    // gh-0 looks suspicious; Sam put gh-1 in Receipts by hand.
    sqlx::query(
        "UPDATE conversations.decisions d SET value = 'true'::jsonb
           FROM conversations.messages m
          WHERE d.message_id = m.id AND m.provider_message_id = 'gh-0'
            AND d.question = 'suspicious'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let hand = kit.conversation_of("gh-1").await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{hand}/stream"),
        Some(json!({"stream": "receipts"})),
    )
    .await;

    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let preview = kit
        .post(&sam, &format!("{rules}/preview"), Some(move_github()))
        .await;
    assert_eq!(preview.json["skipped_suspicious"], 1);
    assert_eq!(preview.json["skipped_member"], 1);
    assert_eq!(preview.json["would_change"], 3);
    kit.post(&sam, &rules, Some(move_github())).await;
    kit.run_jobs().await;
    let placed = streams(&kit).await;
    let at = |id: &str| placed.iter().find(|(m, ..)| m == id).unwrap().1.clone();
    assert_eq!(at("gh-0"), "primary");
    assert_eq!(at("gh-1"), "receipts");
    assert_eq!(at("gh-2"), "updates");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn undo_all_puts_back_what_no_one_changed_since(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 120).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    // Sam moves one to Newsletters by hand afterwards: it stays there.
    let mine = kit.conversation_of("gh-3").await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{mine}/stream"),
        Some(json!({"stream": "newsletters"})),
    )
    .await;

    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    assert_eq!(removed.json["state"], "removed");
    assert_eq!(removed.json["run"]["kind"], "revert");
    kit.run_jobs().await;
    let placed = streams(&kit).await;
    for (id, stream, source) in placed.iter().filter(|(id, ..)| id.starts_with("gh-")) {
        if id == "gh-3" {
            assert_eq!(
                (stream.as_str(), source.as_str()),
                ("newsletters", "member")
            );
        } else {
            assert_eq!(
                (stream.as_str(), source.as_str()),
                ("primary", "default"),
                "{id}"
            );
        }
    }
    let gone = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    // New mail is no longer moved.
    kit.ingest_value(workspace, inbox, &notifications("after", 1, GITHUB, None))
        .await;
    let after = streams(&kit).await;
    assert_eq!(
        after.iter().find(|(id, ..)| id == "after-0").unwrap().1,
        "primary"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn moving_one_back_makes_it_an_exception(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 4).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    for (n, id) in ["gh-1", "gh-2"].into_iter().enumerate() {
        let conversation = kit.conversation_of(id).await;
        let back = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/conversations/{conversation}/stream"),
                Some(json!({"stream": "primary"})),
            )
            .await;
        assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
        assert_eq!(back.json["rule_exception"]["rule_id"], rule.as_str());
        assert_eq!(back.json["rule_exception"]["recent_exceptions"], n + 1);
        assert!(back.json["rule"].is_null());
    }
    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["recent_exceptions"], 2);
    let placed = streams(&kit).await;
    assert_eq!(
        placed.iter().find(|(id, ..)| id == "gh-1").unwrap().1,
        "primary"
    );
    assert_eq!(
        placed.iter().find(|(id, ..)| id == "gh-3").unwrap().1,
        "updates"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn who_may_teach_and_change_rules(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    let (ana, ana_id) = kit.member("ana", workspace);
    let (outsider, _) = kit.member("ola", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");

    let read_only = kit.post(&ana, &rules, Some(move_github())).await;
    assert_eq!(read_only.status, StatusCode::FORBIDDEN);
    assert_eq!(read_only.json["code"], "inbox_read_only");
    let hidden = kit.post(&outsider, &rules, Some(move_github())).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);

    let created = kit.post(&leo, &rules, Some(move_github())).await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.json["rule"]["shared"], true);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");

    // Ana reads it, cannot change it; nobody outside the inbox sees it.
    let seen = kit.get(&ana, &one).await;
    assert_eq!(seen.status, StatusCode::OK);
    assert_eq!(seen.json["may_change"], false);
    assert_eq!(kit.get(&outsider, &one).await.status, StatusCode::NOT_FOUND);
    let listed = kit
        .get(&outsider, &format!("/api/v1/w/{workspace}/rules"))
        .await;
    assert_eq!(listed.json["rules"].as_array().unwrap().len(), 0);

    // Another member who is not its author cannot remove it; a manager can.
    let (other, other_id) = kit.member("ines", workspace);
    kit.add_to_inbox(inbox, other_id, InboxRole::Sender).await;
    let refused = kit.delete(&other, &one).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "rule_not_yours");
    assert_eq!(kit.delete(&sam, &one).await.status, StatusCode::OK);

    // A client token reads rules but cannot make one, even with write access.
    let client = kit.client(workspace, "conversations:read conversations:write");
    assert_eq!(
        kit.get(&client, &format!("/api/v1/w/{workspace}/rules"))
            .await
            .status,
        StatusCode::OK
    );
    let refused = kit.post(&client, &rules, Some(move_github())).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "rule_needs_member");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rule_pauses_when_its_author_loses_access(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let created = kit
        .post(
            &leo,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    sqlx::query("DELETE FROM conversations.inbox_members WHERE user_id = $1")
        .bind(leo_id.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    assert_eq!(kit.get(&sam, &one).await.json["state"], "paused");
    kit.ingest_value(workspace, inbox, &notifications("later", 1, GITHUB, None))
        .await;
    let placed = streams(&kit).await;
    assert_eq!(
        placed.iter().find(|(id, ..)| id == "later-0").unwrap().1,
        "primary"
    );
    // Sam, a manager, adopts it: it acts again with Sam's rights.
    let adopted = kit.patch(&sam, &one, json!({"adopt": true})).await;
    assert_eq!(adopted.json["state"], "active");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn invalid_rules_are_refused(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    for body in [
        json!({"conditions": [{"field": "sender_domain", "value": "gmail.com"}],
               "actions": [{"action": "archive"}]}),
        json!({"conditions": [{"field": "stream", "value": "updates"}],
               "actions": [{"action": "mark_read"}]}),
        json!({"conditions": [{"field": "sender_address", "value": GITHUB}],
               "actions": [{"action": "move_to_stream", "stream": "updates"},
                           {"action": "move_to_stream", "stream": "receipts"}]}),
        json!({"conditions": [], "actions": [{"action": "archive"}]}),
    ] {
        let refused = kit.post(&sam, &rules, Some(body)).await;
        assert_eq!(
            refused.status,
            StatusCode::BAD_REQUEST,
            "{:?}",
            refused.json
        );
        assert_eq!(refused.json["code"], "invalid_rule");
    }
}

#[derive(Debug)]
struct ActsWithoutAsking;

impl MemberPreferences for ActsWithoutAsking {
    fn acts_without_asking<'a>(
        &'a self,
        _tx: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _member: UserId,
    ) -> PreferenceFuture<'a> {
        Box::pin(async { Ok(true) })
    }
}

fn from_corrections() -> Value {
    json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "move_to_stream", "stream": "updates"}],
        "origin": "correction",
    })
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rules_from_evidence_ask_first_by_default(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(from_corrections()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["state"], "proposed");
    assert_eq!(created.json["acted_without_asking"], false);
    kit.run_jobs().await;
    assert!(streams(&kit).await.iter().all(|(_, s, _)| s == "primary"));
    // Saying yes applies it.
    let rule = created.json["rule"]["id"].as_str().unwrap();
    let confirmed = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}/confirm"),
            Some(json!({})),
        )
        .await;
    assert_eq!(confirmed.json["state"], "active");
    kit.run_jobs().await;
    assert_eq!(
        streams(&kit)
            .await
            .iter()
            .filter(|(_, s, _)| s == "updates")
            .count(),
        2
    );
    let again = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}/confirm"),
            Some(json!({})),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "rule_not_proposed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_lets_campbooks_act_gets_it_active_in_a_personal_inbox(db: PgPool) {
    let kit = kit(db).await.with_preferences(Arc::new(ActsWithoutAsking));
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(from_corrections()),
        )
        .await;
    assert_eq!(created.json["rule"]["state"], "active");
    assert_eq!(created.json["acted_without_asking"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_inbox_always_asks_first(db: PgPool) {
    let kit = kit(db).await.with_preferences(Arc::new(ActsWithoutAsking));
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let (_, leo) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo, InboxRole::Sender).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(from_corrections()),
        )
        .await;
    assert_eq!(created.json["rule"]["state"], "proposed");
    assert_eq!(created.json["acted_without_asking"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_rule_for_the_same_mail_and_kind_replaces_the_old(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    kit.post(&sam, &rules, Some(move_github())).await;
    let preview = kit
        .post(
            &sam,
            &format!("{rules}/preview"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB},
                               {"field": "subject_template", "value": "[acme/web] pull request # merged"}],
                "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
            })),
        )
        .await;
    assert_eq!(preview.json["matching"], 2, "{:?}", preview.json);
    assert_eq!(preview.json["overrides"][0]["conversations"], 2);
    let newer = json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
    });
    kit.post(&sam, &rules, Some(newer)).await;
    let listed = kit.get(&sam, &format!("/api/v1/w/{workspace}/rules")).await;
    let all = listed.json["rules"].as_array().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["actions"][0]["stream"], "newsletters");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rules_go_with_their_inbox(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
        Some(move_github()),
    )
    .await;
    kit.run_jobs().await;
    sqlx::query("DELETE FROM conversations.inboxes WHERE id = $1")
        .bind(inbox.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM conversations.rules)
              + (SELECT count(*) FROM conversations.rule_runs)
              + (SELECT count(*) FROM conversations.rule_applications)
              + (SELECT count(*) FROM conversations.rule_dismissals)",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(left, 0);
}

/// The PR's terminal recording, narrated: 1,000 lookalikes taught once, new
/// mail handled on arrival, then Undo all. Slow, so not in the default run:
/// `cargo test -p campbooks-conversations a_thousand_lookalikes -- --ignored --nocapture`.
#[sqlx::test(migrations = "../../../migrations")]
#[ignore = "narrated demo for the recording; the behaviour is covered above"]
async fn a_thousand_lookalikes_taught_once(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1_000).await;
    let in_stream = |kit: &Kit, stream: &'static str| {
        let db = kit.db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM conversations.conversations WHERE stream = $1",
            )
            .bind(stream)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    println!(
        "\nSam's inbox: {} in Primary, {} in Updates",
        in_stream(&kit, "primary").await,
        in_stream(&kit, "updates").await
    );
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let preview = kit
        .post(&sam, &format!("{rules}/preview"), Some(move_github()))
        .await;
    println!(
        "Preview \"mail from {GITHUB} -> Updates\": matches {}, would move {}, e.g. {:?}",
        preview.json["matching"],
        preview.json["would_change"],
        preview.json["samples"][0]["subject"]
    );
    let created = kit.post(&sam, &rules, Some(move_github())).await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    println!(
        "Rule created ({}), run of {} started",
        created.json["rule"]["state"], created.json["rule"]["run"]["total"]
    );
    kit.run_jobs().await;
    for event in kit.events("conversations.rule_changed").await {
        println!(
            "  progress: {} of {} done",
            event["run"]["done"], event["run"]["total"]
        );
    }
    println!(
        "Now: {} in Primary, {} in Updates",
        in_stream(&kit, "primary").await,
        in_stream(&kit, "updates").await
    );
    kit.ingest_value(workspace, inbox, &notifications("new", 1, GITHUB, None))
        .await;
    let placed = streams(&kit).await;
    let new = placed.iter().find(|(id, ..)| id == "new-0").unwrap();
    println!(
        "A new notification arrives: in {} (placed by {})",
        new.1, new.2
    );
    kit.delete(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
    )
    .await;
    kit.run_jobs().await;
    println!(
        "Undo all: {} in Primary, {} in Updates",
        in_stream(&kit, "primary").await,
        in_stream(&kit, "updates").await
    );
    assert_eq!(in_stream(&kit, "updates").await, 0);
}
