//! The module's tracked events (ADR 0020, campbooks-cloud#325), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums and counts, and nothing for a change that changes
//! nothing. Sends, notices, reads and inboxes are checked next to their
//! features' tests (they reuse those helpers).

use axum::http::StatusCode;
use campbooks_kernel::{
    tenancy::WorkspaceId,
    testing::{self, TrackedRow},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    features::rules::tests::{GITHUB, as_shown, kit as rules_kit, move_github, sams_notifications},
    model::InboxRole,
    testkit::Kit,
};

async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// `event` is version 1 of `name`, in `workspace`, caused by Sam's own
/// request to `operation`, and holds no address (a sign of personal data).
fn sams(event: &TrackedRow, name: &str, workspace: WorkspaceId, operation: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    let envelope = &event.envelope;
    assert_eq!(envelope["name"], name);
    assert_eq!(
        envelope["actor"],
        json!({"type": "user", "id": "sam"}),
        "{name}"
    );
    assert_eq!(envelope["operation"], operation, "{name}");
    assert_eq!(envelope["workspace_id"], workspace.to_string());
    assert_eq!(
        envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    no_address(event);
}

/// Ids, enums, counts and times only: no address anywhere in the body.
fn no_address(event: &TrackedRow) {
    let body = event.body.to_string();
    assert!(
        !body.contains('@'),
        "{} holds an address: {body}",
        event.name
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn each_flag_tracks_its_change_once(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");

    for (action, event, operation, field, after) in [
        (
            "read",
            "conversations.conversation_read",
            "conversations.mark_read",
            "unread_count",
            json!(0),
        ),
        (
            "unread",
            "conversations.conversation_unread_marked",
            "conversations.mark_unread",
            "unread_count",
            json!(1),
        ),
        (
            "star",
            "conversations.conversation_starred",
            "conversations.star",
            "starred",
            json!(true),
        ),
        (
            "unstar",
            "conversations.conversation_unstarred",
            "conversations.unstar",
            "starred",
            json!(false),
        ),
        (
            "trash",
            "conversations.conversation_trashed",
            "conversations.trash",
            "status",
            json!("trash"),
        ),
        (
            "untrash",
            "conversations.conversation_untrashed",
            "conversations.untrash",
            "status",
            json!("inbox"),
        ),
        (
            "spam",
            "conversations.conversation_spammed",
            "conversations.report_spam",
            "status",
            json!("spam"),
        ),
        (
            "not-spam",
            "conversations.conversation_unspammed",
            "conversations.not_spam",
            "status",
            json!("inbox"),
        ),
    ] {
        let done = kit.post(&sam, &format!("{base}/{action}"), None).await;
        assert_eq!(done.status, StatusCode::OK, "{action}: {:?}", done.json);
        // Again: nothing changes, nothing more is tracked.
        let again = kit.post(&sam, &format!("{base}/{action}"), None).await;
        assert_eq!(again.status, StatusCode::OK, "{action}: {:?}", again.json);
        let events = tracked(&kit, event).await;
        assert_eq!(events.len(), 1, "{event}");
        sams(&events[0], event, workspace, operation);
        let conversation = &events[0].body["conversation"];
        assert_eq!(conversation["id"], id.as_str(), "{event}");
        assert_eq!(conversation["inbox_id"], inbox.to_string(), "{event}");
        assert_eq!(conversation[field], after, "{event}");
        assert_eq!(conversation["message_count"], 2, "{event}");
        assert!(conversation.get("subject").is_none());
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_tracks_the_stream_it_left(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-3").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{id}/stream");

    let moved = kit
        .post(&sam, &uri, Some(json!({"stream": "primary"})))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    kit.post(&sam, &uri, Some(json!({"stream": "primary"})))
        .await;

    let events = tracked(&kit, "conversations.conversation_moved").await;
    assert_eq!(events.len(), 1, "the same stream again tracks nothing");
    sams(
        &events[0],
        "conversations.conversation_moved",
        workspace,
        "conversations.move_to_stream",
    );
    let body = &events[0].body;
    assert_eq!(body["before"]["stream"], "newsletters");
    assert_eq!(body["before"]["stream_source"], "model");
    assert_eq!(body["conversation"]["id"], id.as_str());
    assert_eq!(body["conversation"]["stream"], "primary");
    assert_eq!(body["conversation"]["stream_source"], "member");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn labels_track_what_they_change_and_never_their_name(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, _, sam) = kit.sams_inbox().await;
    let first = kit.conversation_of("en-1").await;
    let second = kit.conversation_of("en-3").await;
    let labels = |id: &str| format!("/api/v1/w/{workspace}/conversations/{id}/labels");

    let added = kit
        .post(&sam, &labels(&first), Some(json!({"name": "Accountant"})))
        .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    let label = added.json["id"].as_str().unwrap().to_owned();
    // Already on it: nothing more.
    kit.post(&sam, &labels(&first), Some(json!({"name": "Accountant"})))
        .await;
    // The inbox's label, on another conversation.
    kit.post(&sam, &labels(&second), Some(json!({"name": "Accountant"})))
        .await;

    let events = tracked(&kit, "conversations.conversation_labelled").await;
    assert_eq!(events.len(), 2);
    sams(
        &events[0],
        "conversations.conversation_labelled",
        workspace,
        "conversations.add_label",
    );
    assert_eq!(events[0].body["conversation"]["id"], first.as_str());
    assert_eq!(events[0].body["label_id"], label.as_str());
    assert_eq!(events[0].body["label_created"], true);
    assert_eq!(events[1].body["conversation"]["id"], second.as_str());
    assert_eq!(events[1].body["label_id"], label.as_str());
    assert_eq!(events[1].body["label_created"], false);
    for event in &events {
        assert!(!event.body.to_string().contains("Accountant"));
    }

    let removed = kit
        .delete(&sam, &format!("{}/{label}", labels(&first)))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let events = tracked(&kit, "conversations.conversation_unlabelled").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.conversation_unlabelled",
        workspace,
        "conversations.remove_label",
    );
    assert_eq!(events[0].body["label_id"], label.as_str());
    assert_eq!(events[0].body["conversation"]["id"], first.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn verdicts_on_a_need_are_tracked_with_the_need_afterwards(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    // The hosting invoice: a Pay need put to Sam as a question.
    let bill = kit.conversation_of("needs-3").await;
    let seen = kit.message_of("needs-3").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{bill}/need");

    let confirmed = kit
        .post(
            &sam,
            &format!("{base}/confirm"),
            Some(json!({"message_id": seen})),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{:?}", confirmed.json);
    let events = tracked(&kit, "conversations.need_confirmed").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.need_confirmed",
        workspace,
        "conversations.confirm_need",
    );
    let need = &events[0].body["need"];
    assert_eq!(need["conversation_id"], bill.as_str());
    assert_eq!(need["message_id"], seen.as_str());
    assert_eq!(need["reason"], "pay");
    assert_eq!(need["question"], "money");
    assert_eq!(need["state"], "open");
    assert!(need.get("due_on").is_none() && need["has_due_date"].is_boolean());
    assert_eq!(events[0].body["conversation"]["id"], bill.as_str());

    let paid = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "already_paid", "message_id": seen})),
        )
        .await;
    assert_eq!(paid.status, StatusCode::OK, "{:?}", paid.json);
    let events = tracked(&kit, "conversations.need_resolved").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.need_resolved",
        workspace,
        "conversations.resolve_need",
    );
    assert_eq!(events[0].body["need"]["state"], "cleared");
    assert_eq!(events[0].body["need"]["cleared_because"], "done");

    let back = kit.post(&sam, &format!("{base}/reopen"), None).await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    let events = tracked(&kit, "conversations.need_reopened").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.need_reopened",
        workspace,
        "conversations.reopen_need",
    );
    assert_eq!(events[0].body["need"]["state"], "open");
}

/// Save `changes` to the draft at `uri` as `token` reads it now.
async fn save(
    kit: &Kit,
    token: &str,
    uri: &str,
    mut changes: Value,
) -> campbooks_kernel::testing::TestResponse {
    let shown = kit.get(token, uri).await.json;
    changes["version"] = shown["version"].clone();
    changes["content_token"] = shown["content_token"].clone();
    kit.patch(token, uri, changes).await
}

#[sqlx::test(migrations = "../../../migrations")]
async fn drafts_are_tracked_by_counts_never_by_who_or_what(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let drafts = format!("/api/v1/w/{workspace}/drafts");

    let created = kit
        .post(
            &sam,
            &drafts,
            Some(json!({"kind": "new", "inbox_id": inbox,
                "to": [{"name": "Leo", "address": "leo@example.com"}],
                "bcc": [{"address": "mo@example.com"}],
                "subject": "Lunch", "body": "Thursday?"})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let id = created.json["id"].as_str().unwrap().to_owned();
    let uri = format!("{drafts}/{id}");
    let events = tracked(&kit, "conversations.draft_created").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.draft_created",
        workspace,
        "conversations.create_draft",
    );
    let draft = &events[0].body["draft"];
    assert_eq!(draft["id"], id.as_str());
    assert_eq!(draft["inbox_id"], inbox.to_string());
    assert_eq!(draft["kind"], "new");
    assert_eq!(draft["state"], "draft");
    assert_eq!(draft["to_count"], 1);
    assert_eq!(draft["cc_count"], 0);
    assert_eq!(draft["bcc_count"], 1);
    assert_eq!(draft["attachment_count"], 0);
    assert!(draft.get("conversation_id").is_none());
    let text = events[0].body.to_string();
    for private in ["Leo", "Lunch", "Thursday"] {
        assert!(!text.contains(private), "{private} in {text}");
    }

    // A save that changes nothing tracks nothing; a change does.
    let same = save(&kit, &sam, &uri, json!({"body": "Thursday?"})).await;
    assert_eq!(same.status, StatusCode::OK, "{:?}", same.json);
    assert!(
        tracked(&kit, "conversations.draft_updated")
            .await
            .is_empty()
    );
    let changed = save(&kit, &sam, &uri, json!({"body": "Friday?"})).await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.json);
    let events = tracked(&kit, "conversations.draft_updated").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.draft_updated",
        workspace,
        "conversations.update_draft",
    );
    assert_eq!(events[0].body["draft"]["version"], 3);

    let discarded = kit.delete(&sam, &uri).await;
    assert_eq!(discarded.status, StatusCode::NO_CONTENT);
    let events = tracked(&kit, "conversations.draft_discarded").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.draft_discarded",
        workspace,
        "conversations.delete_draft",
    );
    assert_eq!(events[0].body["draft"]["id"], id.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_dismissed_suggestion_is_tracked_with_its_count_not_its_key(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-3").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{id}/rule-suggestions/dismiss");
    let key = json!({"key": "sender:hello@letters.paperkite.example:archive"});

    for _ in 0..2 {
        let dismissed = kit.post(&sam, &uri, Some(key.clone())).await;
        assert_eq!(
            dismissed.status,
            StatusCode::NO_CONTENT,
            "{:?}",
            dismissed.json
        );
    }
    let events = tracked(&kit, "conversations.rule_suggestion_dismissed").await;
    assert_eq!(events.len(), 2);
    sams(
        &events[0],
        "conversations.rule_suggestion_dismissed",
        workspace,
        "conversations.dismiss_rule_suggestion",
    );
    assert_eq!(events[0].body["conversation_id"], id.as_str());
    assert_eq!(events[0].body["inbox_id"], inbox.to_string());
    assert_eq!(events[0].body["times"], 1);
    assert_eq!(events[1].body["times"], 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn person_notes_are_tracked_by_id_never_by_whom_or_what(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let notes = format!("/api/v1/w/{workspace}/person-notes");
    let keep = |note: &str| json!({"address": "maya@northwind-studio.example", "note": note});

    let kept = kit.put(&sam, &notes, keep("Prefers short replies.")).await;
    assert_eq!(kept.status, StatusCode::OK, "{:?}", kept.json);
    let id = kept.json["id"].as_str().unwrap().to_owned();
    // The same text again changes nothing.
    kit.put(&sam, &notes, keep("Prefers short replies.")).await;
    kit.put(&sam, &notes, keep("Prefers calls.")).await;

    let events = tracked(&kit, "conversations.person_note_saved").await;
    assert_eq!(events.len(), 2);
    sams(
        &events[0],
        "conversations.person_note_saved",
        workspace,
        "conversations.set_person_note",
    );
    assert_eq!(events[0].body["note_id"], id.as_str());
    assert_eq!(events[0].body["created"], true);
    assert_eq!(events[1].body["note_id"], id.as_str());
    assert_eq!(events[1].body["created"], false);
    for event in &events {
        let text = event.body.to_string();
        assert!(
            !text.contains("Prefers") && !text.contains("maya"),
            "{text}"
        );
    }

    let forgotten = kit.delete(&sam, &format!("{notes}/{id}")).await;
    assert_eq!(forgotten.status, StatusCode::NO_CONTENT);
    let events = tracked(&kit, "conversations.person_note_deleted").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.person_note_deleted",
        workspace,
        "conversations.delete_person_note",
    );
    assert_eq!(events[0].body["note_id"], id.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn provider_batches_and_the_model_pass_are_tracked_as_system_work(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, _) = kit.sams_inbox().await;

    let batches = tracked(&kit, "conversations.message_batch_applied").await;
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.version, 1);
    assert_eq!(batch.workspace_id, Some(*workspace.as_uuid()));
    assert_eq!(batch.envelope["actor"]["type"], "system");
    assert_eq!(batch.body["inbox_id"], inbox.to_string());
    assert_eq!(batch.body["messages_added"], 4);
    assert_eq!(batch.body["messages_updated"], 0);
    assert_eq!(batch.body["messages_removed"], 0);
    assert_eq!(batch.body["conversations_changed"], 3);
    no_address(batch);
    // The same batch again changes nothing.
    kit.ingest(workspace, inbox, "en").await;
    assert_eq!(
        tracked(&kit, "conversations.message_batch_applied")
            .await
            .len(),
        1
    );

    // The model pass placed the newsletter.
    let decided = tracked(&kit, "conversations.message_decided").await;
    let newsletter = kit.message_of("en-3").await;
    let placed = decided
        .iter()
        .find(|event| event.body["message_id"] == newsletter.as_str())
        .expect("the newsletter's pass is tracked");
    assert_eq!(placed.version, 1);
    assert_eq!(placed.envelope["actor"]["type"], "system");
    assert_eq!(placed.body["pass"], "arrival");
    assert_eq!(placed.body["placed"], true);
    assert_eq!(placed.body["moves_only"], false);
    assert!(placed.body["answer_count"].as_i64().unwrap() > 0);
    assert_eq!(placed.body["conversation"]["stream"], "newsletters");
    assert_eq!(placed.body["conversation"]["stream_source"], "model");
    no_address(placed);
    // Sam's own reply is never asked about.
    let reply = kit.message_of("en-2").await;
    assert!(
        decided
            .iter()
            .all(|event| event.body["message_id"] != reply.as_str())
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_taught_rule_is_tracked_through_its_life_and_its_run(db: PgPool) {
    let kit = rules_kit(db).await.tracking();
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");

    let events = tracked(&kit, "conversations.rule_created").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_created",
        workspace,
        "conversations.create_rule",
    );
    let body = &events[0].body;
    assert_eq!(body["rule"]["id"], rule.as_str());
    assert_eq!(body["rule"]["inbox_id"], inbox.to_string());
    assert_eq!(body["rule"]["state"], "active");
    assert_eq!(body["rule"]["origin"], "action");
    assert_eq!(body["rule"]["condition_count"], 1);
    assert_eq!(body["rule"]["actions"], json!(["move_to_stream"]));
    assert_eq!(body["acted_without_asking"], false);
    assert!(!body.to_string().contains("github"));

    // Its run over the mail already there: one summary when it ends.
    kit.run_jobs().await;
    let runs = tracked(&kit, "conversations.rule_run_finished").await;
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.envelope["actor"]["type"], "system");
    assert_eq!(
        run.envelope["on_behalf_of"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(run.body["run"]["rule_id"], rule.as_str());
    assert_eq!(run.body["run"]["kind"], "apply");
    assert_eq!(run.body["run"]["state"], "done");
    assert_eq!(run.body["run"]["changed"], 3);

    let paused = kit.patch(&sam, &one, json!({"paused": true})).await;
    assert_eq!(paused.status, StatusCode::OK, "{:?}", paused.json);
    let events = tracked(&kit, "conversations.rule_updated").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_updated",
        workspace,
        "conversations.update_rule",
    );
    assert_eq!(events[0].body["rule"]["state"], "paused");
    assert_eq!(events[0].body["rule"]["paused_reason"], "paused_by_member");

    let removed = kit.delete(&sam, &one).await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    let events = tracked(&kit, "conversations.rule_removed").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_removed",
        workspace,
        "conversations.remove_rule",
    );
    assert_eq!(events[0].body["rule"]["state"], "removed");
    assert_eq!(events[0].body["cause"], "member");
    assert_eq!(events[0].body["put_back"], false);

    let back = kit
        .post(&sam, &format!("{one}/restore"), Some(json!({})))
        .await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    let events = tracked(&kit, "conversations.rule_restored").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_restored",
        workspace,
        "conversations.restore_rule",
    );
    assert_eq!(events[0].body["rule"]["id"], rule.as_str());
    assert_eq!(events[0].body["rule"]["state"], "paused");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn proposals_track_their_yes_and_their_no(db: PgPool) {
    let kit = rules_kit(db).await.tracking();
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");

    let mut proposed = move_github();
    proposed["state"] = json!("proposed");
    let first = kit.post(&sam, &rules, Some(proposed)).await;
    assert_eq!(first.status, StatusCode::CREATED, "{:?}", first.json);
    let first = first.json["rule"]["id"].as_str().unwrap().to_owned();
    let created = tracked(&kit, "conversations.rule_created").await;
    assert_eq!(created[0].body["rule"]["state"], "proposed");
    let one = format!("/api/v1/w/{workspace}/rules/{first}");
    let body = as_shown(&kit, &sam, &one, json!({"apply_to_existing": false})).await;
    let kept = kit.post(&sam, &format!("{one}/confirm"), Some(body)).await;
    assert_eq!(kept.status, StatusCode::OK, "{:?}", kept.json);
    let events = tracked(&kit, "conversations.rule_confirmed").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_confirmed",
        workspace,
        "conversations.confirm_rule",
    );
    assert_eq!(events[0].body["rule"]["id"], first.as_str());
    assert_eq!(events[0].body["rule"]["state"], "active");

    let second = kit
        .post(
            &sam,
            &rules,
            Some(json!({
                "conditions": [{"field": "sender_address", "value": "news@example.com"}],
                "actions": [{"action": "archive"}],
                "state": "proposed",
            })),
        )
        .await;
    assert_eq!(second.status, StatusCode::CREATED, "{:?}", second.json);
    let second = second.json["rule"]["id"].as_str().unwrap().to_owned();
    let declined = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{second}/decline"),
            None,
        )
        .await;
    assert_eq!(declined.status, StatusCode::OK, "{:?}", declined.json);
    let events = tracked(&kit, "conversations.rule_declined").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_declined",
        workspace,
        "conversations.decline_rule",
    );
    assert_eq!(events[0].body["rule"]["id"], second.as_str());
    assert_eq!(events[0].body["rule"]["state"], "removed");
    assert_eq!(events[0].body["rule"]["actions"], json!(["archive"]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rule_a_newer_one_replaces_is_tracked_as_removed(db: PgPool) {
    let kit = rules_kit(db).await.tracking();
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    let old = kit.post(&sam, &rules, Some(move_github())).await;
    let old = old.json["rule"]["id"].as_str().unwrap().to_owned();
    let newer = json!({
        "conditions": [{"field": "sender_address", "value": GITHUB}],
        "actions": [{"action": "move_to_stream", "stream": "newsletters"}],
    });
    let new = kit.post(&sam, &rules, Some(newer)).await;
    assert_eq!(new.status, StatusCode::CREATED, "{:?}", new.json);

    let events = tracked(&kit, "conversations.rule_removed").await;
    assert_eq!(events.len(), 1);
    sams(
        &events[0],
        "conversations.rule_removed",
        workspace,
        "conversations.create_rule",
    );
    assert_eq!(events[0].body["rule"]["id"], old.as_str());
    assert_eq!(events[0].body["rule"]["state"], "removed");
    assert_eq!(events[0].body["cause"], "replaced");
    assert_eq!(tracked(&kit, "conversations.rule_created").await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_tracking_off_no_change_is_written(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let starred = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/star"),
            None,
        )
        .await;
    assert_eq!(starred.json["starred"], true);
    let any: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.tracked_events")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(any, 0);
}
