//! "Scout noticed": suggestions from what an inbox does with a sender's
//! mail, answered per member, accepted through a confirmed smart rule.

use axum::http::StatusCode;
use campbooks_ai::{AiConfig, ExternalContent, Gateway, ProviderConfig, Role};
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

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

/// What the receiving provider says of mail really from `BUILDS`'s domain.
const PASSES: &str = "mx.google.com; dkim=pass header.i=@ci.example.com header.s=s1; \
                      dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=ci.example.com";
/// And of mail pretending to be from it: authenticated for another domain.
const SPOOFED: &str = "mx.google.com; dkim=pass header.i=@spoofer.example header.s=s1; \
                       spf=pass smtp.mailfrom=bounce@spoofer.example";

/// `recorded`'s messages, with the receiving provider's `results`.
fn received_with(mut recorded: Value, results: &str) -> Value {
    for change in recorded["changes"].as_array_mut().unwrap() {
        change["message"]["headers"]["authentication_results"] = json!(results);
    }
    recorded
}

/// Conversation of `provider_id`, as Sam's list shows it.
async fn listed(
    kit: &Kit,
    sam: &str,
    workspace: WorkspaceId,
    provider_id: &str,
) -> (String, Value) {
    let id = kit.conversation_of(provider_id).await;
    let list = kit
        .get(
            sam,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    (id, list.json["conversations"][0].clone())
}

/// A "Worth a look" rule on `BUILDS`, taught by Sam.
async fn highlight_rule(kit: &Kit, sam: &str, workspace: WorkspaceId, inbox: InboxId) {
    let created = kit
        .post(
            sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": BUILDS}],
                "actions": [{"action": "worth_a_look"}],
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suggestion_can_be_answered_with_another_goal_worth_a_look(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("ci", BUILDS, 12), PASSES),
    )
    .await;
    let notices = noticed(&kit, &sam, workspace).await;
    let notice = &notices[0];

    // The member likes these: "Highlight them" answers the suggestion with a
    // rule of their own choosing, not the one Scout guessed.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": notice["conditions"],
                "actions": [{"action": "worth_a_look"}],
                "origin": "suggestion",
                "notice_id": notice["id"],
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["state"], "active");
    assert!(noticed(&kit, &sam, workspace).await.is_empty());
    kit.run_jobs().await;

    // Applied to the mail already here, only the newest unread one waits on
    // Home as Worth a look, not pinned: never a backlog of the other eleven.
    let (id, before) = listed(&kit, &sam, workspace, "ci-11").await;
    assert_eq!(before["need"]["reason"], "worth_a_look", "{before}");
    assert_eq!(before["need"]["trust"], "sure");
    assert_eq!(before["need"]["question"], "worth_a_look");
    for older in ["ci-0", "ci-3", "ci-10"] {
        let (_, other) = listed(&kit, &sam, workspace, older).await;
        assert!(other["need"].is_null(), "{older}: {other}");
    }

    // Opening it once clears it.
    let read = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/read"),
            None,
        )
        .await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json);
    let (_, after) = listed(&kit, &sam, workspace, "ci-11").await;
    assert!(after["need"].is_null(), "{after}");
    let (_, still) = listed(&kit, &sam, workspace, "ci-10").await;
    assert!(still["need"].is_null(), "{still}");

    // The Home item clears as "opened".
    let cleared: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.needs
          WHERE state = 'cleared' AND cleared_because = 'opened'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(cleared, 1);

    // New mail still raises one item per message.
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("later", BUILDS, 2), PASSES),
    )
    .await;
    for arrived in ["later-0", "later-1"] {
        let (_, raised) = listed(&kit, &sam, workspace, arrived).await;
        assert_eq!(
            raised["need"]["reason"], "worth_a_look",
            "{arrived}: {raised}"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn worth_a_look_is_never_raised_for_a_sender_that_is_not_authenticated(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    // Already here: two real ones, then a spoofed one and one with no
    // results at all, both newer than the real ones.
    let mut recorded = received_with(daily("real", BUILDS, 2), PASSES);
    let fake = received_with(daily("fake", BUILDS, 1), SPOOFED);
    let bare = daily("bare", BUILDS, 1);
    for extra in [fake, bare] {
        recorded["changes"]
            .as_array_mut()
            .unwrap()
            .extend(extra["changes"].as_array().unwrap().clone());
    }
    // Make the unauthenticated ones the newest.
    recorded["changes"][2]["message"]["sent_at"] = json!(Utc::now().to_rfc3339());
    recorded["changes"][3]["message"]["sent_at"] = json!(Utc::now().to_rfc3339());
    kit.ingest_value(workspace, inbox, &recorded).await;
    highlight_rule(&kit, &sam, workspace, inbox).await;

    // The newest *authenticated* one is the one to look at.
    let (_, real) = listed(&kit, &sam, workspace, "real-1").await;
    assert_eq!(real["need"]["reason"], "worth_a_look", "{real}");
    for unsafe_one in ["fake-0", "bare-0"] {
        let (_, none) = listed(&kit, &sam, workspace, unsafe_one).await;
        assert!(none["need"].is_null(), "{unsafe_one}: {none}");
    }

    // Mail arriving later is no different: a spoofed one raises nothing, an
    // authenticated one does.
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("spoof", BUILDS, 1), SPOOFED),
    )
    .await;
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("good", BUILDS, 1), PASSES),
    )
    .await;
    let (_, spoofed) = listed(&kit, &sam, workspace, "spoof-0").await;
    assert!(spoofed["need"].is_null(), "{spoofed}");
    let (_, good) = listed(&kit, &sam, workspace, "good-0").await;
    assert_eq!(good["need"]["reason"], "worth_a_look", "{good}");
}

/// A highlight answer a rule stored on `provider_id`'s message, as one from
/// before the engine checked the sender could be stored.
async fn store_highlight(kit: &Kit, provider_id: &str) {
    sqlx::query(
        "INSERT INTO conversations.decisions
                (id, workspace_id, message_id, conversation_id, question, source, strategy,
                 value, confidence, model, latency_ms, acted, state_version)
         SELECT gen_random_uuid(), workspace_id, id, conversation_id, 'worth_a_look', 'taught',
                'taught', 'true'::jsonb, 1.0, 'taught', 0, true, 1
           FROM conversations.messages WHERE provider_message_id = $1",
    )
    .bind(provider_id)
    .execute(&kit.db)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stored_highlight_on_unauthenticated_mail_raises_no_need(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    kit.ingest_value(workspace, inbox, &daily("bare", BUILDS, 1))
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("spoof", BUILDS, 1), SPOOFED),
    )
    .await;
    kit.ingest_value(
        workspace,
        inbox,
        &received_with(daily("good", BUILDS, 1), PASSES),
    )
    .await;
    for id in ["bare-0", "spoof-0", "good-0"] {
        store_highlight(&kit, id).await;
        // Reading then unreading works the need out again.
        let conversation = kit.conversation_of(id).await;
        for step in ["read", "unread"] {
            let done = kit
                .post(
                    &sam,
                    &format!("/api/v1/w/{workspace}/conversations/{conversation}/{step}"),
                    None,
                )
                .await;
            assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
        }
    }
    kit.run_jobs().await;
    for id in ["bare-0", "spoof-0"] {
        let (_, none) = listed(&kit, &sam, workspace, id).await;
        assert!(none["need"].is_null(), "{id}: {none}");
    }
    let (_, good) = listed(&kit, &sam, workspace, "good-0").await;
    assert_eq!(good["need"]["reason"], "worth_a_look", "{good}");
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
async fn answers_are_tracked_by_kind_never_by_sender(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline())
        .await
        .tracking();
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;
    let notices = noticed(&kit, &sam, workspace).await;
    let id = notices[0]["id"].as_str().unwrap().to_owned();
    let answer = format!("/api/v1/w/{workspace}/rule-notices/{id}/answer");

    let not_now = kit
        .post(&sam, &answer, Some(json!({"answer": "not_now"})))
        .await;
    assert_eq!(not_now.status, StatusCode::NO_CONTENT);
    let answered =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.rule_notice_answered")
            .await;
    assert_eq!(answered.len(), 1);
    let event = &answered[0];
    assert_eq!(event.version, 1);
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(
        event.envelope["operation"],
        "conversations.answer_rule_notice"
    );
    assert_eq!(event.body["notice_id"], id.as_str());
    assert_eq!(event.body["inbox_id"], json!(inbox));
    assert_eq!(event.body["kind"], "keep_off_home");
    assert_eq!(event.body["answer"], "not_now");
    assert!(!event.body.to_string().contains(BUILDS));

    // Undo, then Undo again (nothing left to take back).
    assert_eq!(
        kit.delete(&sam, &answer).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        kit.delete(&sam, &answer).await.status,
        StatusCode::NO_CONTENT
    );
    let cleared = campbooks_kernel::testing::tracked_events(
        &kit.db,
        "conversations.rule_notice_answer_cleared",
    )
    .await;
    assert_eq!(cleared.len(), 1);
    assert_eq!(
        cleared[0].envelope["operation"],
        "conversations.clear_rule_notice_answer"
    );
    assert_eq!(cleared[0].body["notice_id"], id.as_str());
    assert_eq!(cleared[0].body["kind"], "keep_off_home");

    kit.post(&sam, &answer, Some(json!({"answer": "never"})))
        .await;
    let answered =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.rule_notice_answered")
            .await;
    assert_eq!(answered.len(), 2);
    assert_eq!(answered[1].body["answer"], "never");
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
async fn one_suggestion_reads_as_the_list_shows_it_and_starts_no_pass(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    kit.ingest_value(workspace, inbox, &daily("ci", BUILDS, 12))
        .await;
    let notices = noticed(&kit, &sam, workspace).await;
    let id = notices[0]["id"].as_str().unwrap().to_owned();
    let one = format!("/api/v1/w/{workspace}/rule-notices/{id}");

    // Looked at lately or not, reading one never starts a pass.
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = now() - interval '2 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let read = kit.get(&sam, &one).await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json);
    assert_eq!(read.json, notices[0]);
    kit.run_jobs().await;
    let stale: bool = sqlx::query_scalar(
        "SELECT scanned_at < now() - interval '1 day' FROM conversations.rule_notice_scans",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(stale, "a read started a pass");

    // Leo is in the workspace but not the inbox; Rita only reads it; an app
    // acts for nobody; another workspace's member asks with the id.
    let (leo, _) = kit.member("leo", workspace);
    let (rita, rita_id) = kit.member("rita", workspace);
    kit.add_to_inbox(inbox, rita_id, InboxRole::Reader).await;
    let app = kit.client(workspace, "conversations:read");
    let elsewhere = WorkspaceId::new();
    let (eve, _) = kit.member("eve", elsewhere);
    assert_eq!(kit.get(&leo, &one).await.status, StatusCode::NOT_FOUND);
    // Rita only reads the inbox: the list never shows her its suggestions.
    assert_eq!(kit.get(&rita, &one).await.status, StatusCode::NOT_FOUND);
    // An app is given no inbox, so it sees no suggestion in one.
    assert_eq!(kit.get(&app, &one).await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        kit.get(&eve, &format!("/api/v1/w/{elsewhere}/rule-notices/{id}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // A suggestion Sam hid is gone for him, until he takes the answer back.
    let answer = format!("{one}/answer");
    for hidden in ["not_now", "never"] {
        let answered = kit
            .post(&sam, &answer, Some(json!({"answer": hidden})))
            .await;
        assert_eq!(answered.status, StatusCode::NO_CONTENT);
        assert_eq!(kit.get(&sam, &one).await.status, StatusCode::NOT_FOUND);
        kit.delete(&sam, &answer).await;
        assert_eq!(kit.get(&sam, &one).await.status, StatusCode::OK);
    }

    // Once said yes to, it is no longer offered.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": notices[0]["conditions"],
                "actions": notices[0]["actions"],
                "origin": "suggestion",
                "notice_id": id,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(kit.get(&sam, &one).await.status, StatusCode::NOT_FOUND);
    let unknown = format!(
        "/api/v1/w/{workspace}/rule-notices/{}",
        crate::model::RuleNoticeId::new()
    );
    assert_eq!(kit.get(&sam, &unknown).await.status, StatusCode::NOT_FOUND);
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

// --- Kinds within a sender's mail (campbooks-cloud#356) ---------------------

const SHOP: &str = "orders@shop.example";

/// One subject, how many days ago it came, and whether it was read.
type Item = (String, i64, bool);

/// The shop's mail of `items`, automatic, one conversation each.
fn shop_mail(prefix: &str, items: &[Item]) -> Value {
    let now = Utc::now();
    let changes: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, (subject, days, read))| {
            json!({
                "op": "upsert",
                "message": {
                    "provider_message_id": format!("{prefix}-{i}"),
                    "thread_key": format!("thr-{prefix}-{i}"),
                    "from": {"name": "Northwind", "address": SHOP},
                    "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                    "subject": subject,
                    "sent_at": (now - TimeDelta::days(*days) - TimeDelta::hours(1)).to_rfc3339(),
                    "direction": "inbound",
                    "headers": {"auto_submitted": "auto-generated"},
                    "text": "Thanks for shopping with us.",
                    "mailbox": {"in_inbox": true},
                    "read": read, "starred": false
                }
            })
        })
        .collect();
    json!({"changes": changes})
}

/// `n` weekly deals, a day apart, never opened.
fn deals(n: i64) -> Vec<Item> {
    (0..n)
        .map(|i| (format!("Deals of the week {}", 40 - i), i + 1, false))
        .collect()
}

/// `n` digests, every other day, never opened.
fn digests(n: i64) -> Vec<Item> {
    (0..n)
        .map(|i| (format!("Your weekly digest {}", 20 - i), 2 * i + 1, false))
        .collect()
}

/// `n` shipping notices, every other day, all read.
fn parcels(n: i64) -> Vec<Item> {
    (0..n)
        .map(|i| {
            (
                format!("Your order {} has shipped", 5100 + i),
                2 * i + 2,
                true,
            )
        })
        .collect()
}

/// Ten promotions with subjects of their own, never opened.
fn sales() -> Vec<Item> {
    [
        "boots", "scarves", "coats", "gloves", "hats", "jackets", "socks", "shirts", "belts",
        "bags",
    ]
    .iter()
    .zip(1..)
    .map(|(item, day)| (format!("Our {item} sale starts now"), day, false))
    .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shops_mail_is_noticed_kind_by_kind_and_each_kind_is_answered_alone(db: PgPool) {
    let kit = Kit::with_deps(db, ConversationsDeps::offline()).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    let items: Vec<Item> = [deals(12), digests(6), parcels(6)].concat();
    kit.ingest_value(workspace, inbox, &shop_mail("shop", &items))
        .await;

    // The deals and the digests, never the shipping notices Sam reads, and
    // no rule for the whole shop.
    let notices = noticed(&kit, &sam, workspace).await;
    assert_eq!(notices.len(), 2, "{notices:?}");
    let deals = &notices[0];
    assert_eq!(deals["kind"], "keep_off_home");
    assert_eq!(
        deals["conditions"],
        json!([
            {"field": "sender_address", "value": SHOP},
            {"field": "subject_template", "value": "deals of the week #"},
        ])
    );
    assert_eq!(
        deals["pattern"],
        json!({
            "source": "template",
            "example": "Deals of the week 40",
            "search": "subject:\"deals of the week\"",
        })
    );
    assert_eq!(deals["evidence"]["received"], 12);
    assert_eq!(deals["evidence"]["opened"], 0);
    assert_eq!(
        notices[1]["conditions"][1],
        json!({"field": "subject_template", "value": "your weekly digest #"})
    );
    assert_eq!(notices[1]["evidence"]["received"], 6);

    // "Don't suggest this" hides the deals only, for good.
    let id = deals["id"].as_str().unwrap();
    let answered = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/rule-notices/{id}/answer"),
            Some(json!({"answer": "never"})),
        )
        .await;
    assert_eq!(answered.status, StatusCode::NO_CONTENT);
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = now() - interval '2 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let left = noticed(&kit, &sam, workspace).await;
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(left[0]["pattern"]["example"], "Your weekly digest 20");

    // A rule Sam taught for the digests: that kind is no longer offered.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": left[0]["conditions"],
                "actions": left[0]["actions"],
                "origin": "suggestion",
                "notice_id": left[0]["id"],
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert!(noticed(&kit, &sam, workspace).await.is_empty());
}

/// An instruct model grouping subjects with `kinds`.
async fn grouping_model(kinds: &Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "group-test",
            "choices": [{"message": {"role": "assistant",
                                     "content": json!({"kinds": kinds}).to_string()},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 200, "completion_tokens": 40}
        })))
        .mount(&server)
        .await;
    server
}

fn with_complete(provider: ProviderConfig) -> ConversationsDeps {
    ConversationsDeps {
        ai: Some(
            Gateway::new(AiConfig {
                roles: [(Role::Complete, provider)].into(),
                ..AiConfig::default()
            })
            .unwrap(),
        ),
        ..ConversationsDeps::offline()
    }
}

/// The grouping calls `server` received, as their bodies.
async fn grouping_calls(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .filter(|body| body.contains("subject_kinds"))
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_models_words_find_kinds_no_template_shows_and_are_kept_for_days(db: PgPool) {
    let server = grouping_model(&json!([
        {"phrase": "Sale starts now"},
        {"phrase": "has shipped"},
        {"phrase": "boots"},
    ]))
    .await;
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "test");
    let kit = Kit::with_deps(db, with_complete(provider)).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    let items: Vec<Item> = [sales(), parcels(6)].concat();
    kit.ingest_value(workspace, inbox, &shop_mail("shop", &items))
        .await;

    let notices = noticed(&kit, &sam, workspace).await;
    // The promotions, by the model's words, checked against the mail: the
    // shipping notices are read, and "boots" fits one email.
    assert_eq!(notices.len(), 1, "{notices:?}");
    let sales = &notices[0];
    assert_eq!(
        sales["conditions"],
        json!([
            {"field": "sender_address", "value": SHOP},
            {"field": "subject_contains", "value": "sale starts now"},
        ])
    );
    assert_eq!(sales["pattern"]["source"], "phrase");
    assert_eq!(sales["pattern"]["search"], "subject:\"sale starts now\"");
    assert_eq!(sales["pattern"]["example"], "Our boots sale starts now");
    assert_eq!(sales["evidence"]["received"], 10);

    // The model saw the subjects and nothing else of the mail.
    let calls = grouping_calls(&server).await;
    assert_eq!(calls.len(), 1);
    assert!(calls[0].contains("Our boots sale starts now"));
    for private in [SHOP, "Northwind", "Thanks for shopping", "sam@example.com"] {
        assert!(!calls[0].contains(private), "{private}");
    }

    // A later pass reuses the grouping: the model is not asked again.
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = now() - interval '2 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    assert_eq!(noticed(&kit, &sam, workspace).await.len(), 1);
    assert_eq!(grouping_calls(&server).await.len(), 1);

    // Once it is a week old, it is asked again.
    sqlx::query(
        "UPDATE conversations.rule_notice_groupings SET grouped_at = now() - interval '8 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE conversations.rule_notice_scans SET scanned_at = now() - interval '2 days'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    noticed(&kit, &sam, workspace).await;
    assert_eq!(grouping_calls(&server).await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_model_that_may_read_mail_templates_alone_find_kinds(db: PgPool) {
    // A model off the instance, with no opt-in to send it mail.
    let provider = ProviderConfig::new("https://ai.example.com/v1".parse().unwrap(), "m")
        .with_external_content(ExternalContent::Deny);
    let kit = Kit::with_deps(db, with_complete(provider)).await;
    let (workspace, inbox, sam) = sams_inbox(&kit).await;
    let items: Vec<Item> = [deals(12), sales(), parcels(6)].concat();
    kit.ingest_value(workspace, inbox, &shop_mail("shop", &items))
        .await;

    let notices = noticed(&kit, &sam, workspace).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0]["pattern"]["source"], "template");
    assert_eq!(notices[0]["pattern"]["example"], "Deals of the week 40");
    let grouped: i64 =
        sqlx::query_scalar("SELECT count(*) FROM conversations.rule_notice_groupings")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(grouped, 0);
}

#[test]
fn a_template_narrows_the_search_by_its_longest_run_of_words() {
    assert_eq!(
        super::template_search("your order # has shipped from our store"),
        "subject:\"has shipped from our store\""
    );
    assert_eq!(
        super::template_search("deals of the week #"),
        "subject:\"deals of the week\""
    );
    assert_eq!(super::template_search("# * #"), "");
}
