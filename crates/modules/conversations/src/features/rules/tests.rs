//! Tests of the rule operations, their runs, rules on arrival and exceptions.

use std::sync::Arc;

use axum::http::StatusCode;
use campbooks_kernel::{
    preferences::{MemberPreferences, PreferenceFuture},
    tenancy::{ClientActor, ClientDirectory, ClientsFuture, UserId, WorkspaceId},
};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::{Kit, notifications},
};

pub(crate) const GITHUB: &str = "notifications@github.com";

/// A workspace with Sam's inbox holding `n` GitHub notifications and one
/// message from a person; nothing sorted (the shipped policy).
pub(crate) async fn sams_notifications(
    kit: &Kit,
    n: usize,
) -> (WorkspaceId, InboxId, String, UserId) {
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

pub(crate) async fn kit(db: PgPool) -> Kit {
    Kit::with_deps(db, ConversationsDeps::offline()).await
}

pub(crate) fn move_github() -> Value {
    json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "move_to_stream", "stream": "updates"}],
    })
}

pub(crate) async fn streams(kit: &Kit) -> Vec<(String, String, String)> {
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
    assert_eq!(created.json["rule"]["run"]["counted"], false);
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
    // Announced when it starts, once counted, then after each batch.
    assert_eq!(progress, vec![0, 0, 100, 200, 250]);
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
    // Still readable while (and after) its mail goes back, but no longer listed.
    let gone = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(gone.json["state"], "removed");
    assert_eq!(gone.json["run"]["kind"], "revert");
    assert_eq!(gone.json["run"]["state"], "done");
    assert_eq!(gone.json["run"]["changed"], 119);
    let listed = kit.get(&sam, &format!("/api/v1/w/{workspace}/rules")).await;
    assert_eq!(listed.json["rules"].as_array().unwrap().len(), 0);
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

    // A client Sam registered arrives as Sam: it reads rules, but cannot
    // make one, even with write access. Rules are taught by members.
    let kit = kit.with_clients(Arc::new(ActsFor("sam")));
    let client = kit.client(workspace, "conversations:read conversations:write");
    let listed = kit
        .get(&client, &format!("/api/v1/w/{workspace}/rules"))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let refused = kit.post(&client, &rules, Some(move_github())).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "rule_needs_member");
}

/// A client directory where every client acts for one member.
#[derive(Debug)]
struct ActsFor(&'static str);

impl ClientDirectory for ActsFor {
    fn acting_for<'a>(
        &'a self,
        _db: &'a PgPool,
        _client_id: &'a str,
        _workspace: WorkspaceId,
    ) -> ClientsFuture<'a> {
        let subject = self.0.to_owned();
        Box::pin(async move { Ok(ClientActor::ActsFor(subject)) })
    }
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
        "Preview \"mail from {GITHUB} -> Updates\": matches {}, would move {}, e.g. \"{}\"",
        preview.json["matching"],
        preview.json["would_change"],
        preview.json["samples"][0]["subject"]
            .as_str()
            .unwrap_or_default()
    );
    let created = kit.post(&sam, &rules, Some(move_github())).await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    println!(
        "Rule created ({}); its run counts and moves them in the background",
        created.json["rule"]["state"].as_str().unwrap_or_default()
    );
    kit.run_jobs().await;
    let rule_now = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    println!("  counted: {}", rule_now.json["run"]["total"]);
    for event in kit.events("conversations.rule_changed").await {
        let done = &event["run"]["done"];
        if done.as_i64().unwrap_or_default() > 0 {
            println!("  progress: {done} of {} done", event["run"]["total"]);
        }
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

#[sqlx::test(migrations = "../../../migrations")]
async fn nobody_outside_the_inbox_or_the_workspace_reaches_a_rule(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();

    // A member (an owner or admin, say) who is not in the inbox.
    let (admin, _) = kit.member("ola", workspace);
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    assert_eq!(kit.get(&admin, &one).await.status, StatusCode::NOT_FOUND);
    let listed = kit
        .get(&admin, &format!("/api/v1/w/{workspace}/rules"))
        .await;
    assert_eq!(listed.json["rules"].as_array().unwrap().len(), 0);
    let preview = kit
        .post(
            &admin,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules/preview"),
            Some(move_github()),
        )
        .await;
    assert_eq!(preview.status, StatusCode::NOT_FOUND);

    // Another workspace naming this rule's id.
    let other = WorkspaceId::new();
    let (eve, eve_id) = kit.member("eve", other);
    kit.inbox(other, "eve@example.com", &[(eve_id, InboxRole::Manager)])
        .await;
    let theirs = format!("/api/v1/w/{other}/rules/{rule}");
    assert_eq!(kit.get(&eve, &theirs).await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        kit.patch(&eve, &theirs, json!({"paused": true}))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        kit.delete(&eve, &theirs).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_previews_confirms_and_changes_nothing(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let mut proposed = move_github();
    proposed["state"] = json!("proposed");
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(proposed),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let preview = kit
        .post(
            &ana,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules/preview"),
            Some(move_github()),
        )
        .await;
    assert_eq!(preview.status, StatusCode::FORBIDDEN);
    assert_eq!(preview.json["code"], "inbox_read_only");
    let confirm = kit
        .post(
            &ana,
            &format!("/api/v1/w/{workspace}/rules/{rule}/confirm"),
            Some(json!({})),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::FORBIDDEN);
    let update = kit
        .patch(
            &ana,
            &format!("/api/v1/w/{workspace}/rules/{rule}"),
            json!({"paused": true}),
        )
        .await;
    assert_eq!(update.status, StatusCode::FORBIDDEN);
    assert_eq!(update.json["code"], "inbox_read_only");
    // Pausing a proposed rule never makes it act without a yes.
    let paused = kit
        .patch(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}"),
            json!({"paused": true}),
        )
        .await;
    assert_eq!(paused.status, StatusCode::CONFLICT);
    assert_eq!(paused.json["code"], "rule_not_active");
    assert_eq!(
        kit.get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
            .await
            .json["state"],
        "proposed"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_cannot_replace_someone_elses_rule(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    let (ines, ines_id) = kit.member("ines", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    kit.add_to_inbox(inbox, ines_id, InboxRole::Sender).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let leos = kit.post(&leo, &rules, Some(move_github())).await;
    let leos = leos.json["rule"]["id"].as_str().unwrap().to_owned();

    // The same rule, written differently, with another stream.
    let same = json!({
        "conditions": [{"field": "sender_address", "value": " Notifications@GitHub.com "}],
        "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
    });
    let refused = kit.post(&ines, &rules, Some(same.clone())).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.json);
    assert_eq!(refused.json["code"], "rule_not_yours");
    assert_eq!(
        kit.get(&sam, &format!("/api/v1/w/{workspace}/rules/{leos}"))
            .await
            .json["state"],
        "active"
    );

    // A manager may, and Leo's rule is retired and announced.
    let replaced = kit.post(&sam, &rules, Some(same)).await;
    assert_eq!(replaced.status, StatusCode::CREATED);
    assert_eq!(replaced.json["rule"]["conditions"][0]["value"], GITHUB);
    let old = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{leos}"))
        .await;
    assert_eq!(old.json["state"], "removed");
    assert_ne!(old.json["run"]["state"], "running");
    let removed: Vec<Value> = kit
        .events("conversations.rule_changed")
        .await
        .into_iter()
        .filter(|e| e["rule"] == leos.as_str() && e["state"] == "removed")
        .collect();
    assert_eq!(removed.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn previews_never_show_suspicious_mail_and_say_when_complete(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    // gh-2, the newest, looks suspicious.
    sqlx::query(
        "UPDATE conversations.decisions d SET value = 'true'::jsonb
           FROM conversations.messages m
          WHERE d.message_id = m.id AND m.provider_message_id = 'gh-2'
            AND d.question = 'suspicious'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let label = json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "needs_reply", "value": true}],
    });
    let preview = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules/preview"),
            Some(label),
        )
        .await;
    assert_eq!(preview.json["would_change"], 3, "{:?}", preview.json);
    assert_eq!(preview.json["complete"], true);
    let subjects: Vec<&str> = preview.json["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects.len(), 2);
    assert!(!subjects.contains(&"[acme/web] Pull request #4002 merged"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn putting_back_leaves_another_rules_placement_alone(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let first = kit.post(&sam, &rules, Some(move_github())).await;
    let first = first.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    // A more specific rule moves gh-0 on to Newsletters.
    let narrower = json!({
        "conditions": [
            {"field": "sender_address", "value": GITHUB},
            {"field": "subject_contains", "value": "#4000"}
        ],
        "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
    });
    let created = kit.post(&sam, &rules, Some(narrower)).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    kit.run_jobs().await;
    kit.delete(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{first}?restore=true"),
    )
    .await;
    kit.run_jobs().await;
    let placed = streams(&kit).await;
    let at = |id: &str| placed.iter().find(|(m, ..)| m == id).unwrap().1.clone();
    assert_eq!(at("gh-0"), "newsletters");
    assert_eq!(at("gh-1"), "primary");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn public_suffixes_and_long_values_are_refused(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    for body in [
        json!({"conditions": [{"field": "sender_domain", "value": "co.uk"},
                              {"field": "subject_template", "value": "your order # has shipped"}],
               "actions": [{"action": "archive"}]}),
        json!({"conditions": [{"field": "subject_contains", "value": "x ".repeat(200)}],
               "actions": [{"action": "archive"}]}),
    ] {
        let refused = kit.post(&sam, &rules, Some(body)).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        assert_eq!(refused.json["code"], "invalid_rule");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_same_conditions_in_any_order_or_spelling_are_one_rule(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let first = kit
        .post(
            &sam,
            &rules,
            Some(json!({
                "conditions": [{"field": "subject_contains", "value": "  Pull   Request "},
                               {"field": "sender_address", "value": "Notifications@GitHub.com"}],
                "actions": [{"action": "move_to_stream", "stream": "updates"}],
            })),
        )
        .await;
    assert_eq!(first.status, StatusCode::CREATED, "{:?}", first.json);
    assert_eq!(
        first.json["rule"]["conditions"],
        json!([{"field": "sender_address", "value": GITHUB},
               {"field": "subject_contains", "value": "pull request"}])
    );
    // The same rule, written another way and proposed: saying yes replaces
    // the first rather than adding a second.
    let second = kit
        .post(
            &sam,
            &rules,
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB},
                               {"field": "subject_contains", "value": "pull request"}],
                "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
                "state": "proposed",
            })),
        )
        .await;
    let second = second.json["rule"]["id"].as_str().unwrap().to_owned();
    let listed = kit.get(&sam, &format!("/api/v1/w/{workspace}/rules")).await;
    assert_eq!(listed.json["rules"].as_array().unwrap().len(), 2);
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{second}/confirm"),
        Some(json!({"apply_to_existing": false})),
    )
    .await;
    let listed = kit.get(&sam, &format!("/api/v1/w/{workspace}/rules")).await;
    let all = listed.json["rules"].as_array().unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0]["id"], second.as_str());
    // Changing a third rule into the same one replaces it too.
    let third = kit
        .post(
            &sam,
            &rules,
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB}],
                "actions": [{"action": "move_to_stream", "stream": "updates"}],
                "apply_to_existing": false,
            })),
        )
        .await;
    let third = third.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.patch(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{third}"),
        json!({"conditions": [{"field": "subject_contains", "value": "Pull request"},
                              {"field": "sender_address", "value": GITHUB}]}),
    )
    .await;
    let listed = kit.get(&sam, &format!("/api/v1/w/{workspace}/rules")).await;
    let all = listed.json["rules"].as_array().unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0]["id"], third.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn undo_all_still_works_after_the_rule_moves_mail_elsewhere(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    // The rule now says Newsletters; what it moved is still in Updates.
    let changed = kit
        .patch(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}"),
            json!({"actions": [{"action": "move_to_stream", "stream": "newsletters"}]}),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.json);
    kit.delete(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
    )
    .await;
    kit.run_jobs().await;
    for (id, stream, source) in streams(&kit).await {
        assert_eq!(
            (stream.as_str(), source.as_str()),
            ("primary", "default"),
            "{id}"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_inbox_owner_teaches_without_a_membership(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 2).await;
    sqlx::query("UPDATE conversations.inboxes SET owner = $2 WHERE id = $1")
        .bind(inbox.as_uuid())
        .bind(sam_id.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM conversations.inbox_members WHERE user_id = $1")
        .bind(sam_id.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["may_change"], true);
    assert_eq!(created.json["rule"]["state"], "active");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_removed_rule_comes_back_as_it_was_within_the_undo_window(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    // Undo all: removed, its mail put back.
    kit.delete(
        &sam,
        &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
    )
    .await;
    kit.run_jobs().await;
    assert!(streams(&kit).await.iter().all(|(_, s, _)| s == "primary"));
    let restore = format!("/api/v1/w/{workspace}/rules/{rule}/restore");
    // Only who may remove it brings it back.
    let refused = kit.post(&leo, &restore, Some(json!({}))).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "rule_not_yours");
    // Undo of Undo all: the same rule, active again, applied again.
    let back = kit
        .post(&sam, &restore, Some(json!({"apply_to_existing": true})))
        .await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(back.json["id"], rule.as_str());
    assert_eq!(back.json["state"], "active");
    assert_eq!(back.json["run"]["kind"], "apply");
    kit.run_jobs().await;
    let moved = streams(&kit).await;
    assert!(
        moved
            .iter()
            .filter(|(id, ..)| id.starts_with("gh-"))
            .all(|(_, s, _)| s == "updates")
    );
    // Counted once per conversation it holds changed, not once per pass.
    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["applied_count"], 3);
    // Not removed: nothing to bring back.
    let again = kit.post(&sam, &restore, Some(json!({}))).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "rule_not_restorable");
    // Removed too long ago: taught again instead.
    kit.delete(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    sqlx::query(
        "UPDATE conversations.rules SET removed_at = now() - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(&rule)
    .execute(&kit.db)
    .await
    .unwrap();
    let late = kit.post(&sam, &restore, Some(json!({}))).await;
    assert_eq!(late.status, StatusCode::CONFLICT);
    assert_eq!(late.json["code"], "rule_not_restorable");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rows_say_who_may_stop_the_rule_that_moved_them(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    kit.post(
        &leo,
        &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
        Some(move_github()),
    )
    .await;
    kit.run_jobs().await;
    let marks = |token: String| {
        let kit = &kit;
        async move {
            let list = kit
                .get(
                    &token,
                    &format!("/api/v1/w/{workspace}/conversations?stream=updates"),
                )
                .await;
            list.json["conversations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["rule"]["may_change"].as_bool().unwrap())
                .collect::<Vec<_>>()
        }
    };
    // Its author and a manager may; a reader may not.
    assert_eq!(marks(leo).await, [true, true]);
    assert_eq!(marks(sam).await, [true, true]);
    assert_eq!(marks(ana).await, [false, false]);
}
