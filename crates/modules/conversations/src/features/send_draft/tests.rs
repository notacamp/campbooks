use axum::http::StatusCode;
use campbooks_kernel::{
    blobs::BlobRef,
    db::BeginFor as _,
    operations::{InvokeError, Output},
    tenancy::WorkspaceId,
};
use serde_json::{Value, json};
use sqlx::PgPool;

use campbooks_kernel::testing::tracked_events;

use crate::{
    events::{SendCompleted, SendOutcome},
    model::{InboxId, SendId},
    testkit::{Kit, contract},
};

/// Sam's reply to `en-1`, with Ana copied and a formatted body: the draft's
/// URI.
async fn reply(kit: &Kit, workspace: WorkspaceId, sam: &str) -> String {
    let message = kit.message_of("en-1").await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let draft = kit
        .post(
            sam,
            &uri,
            Some(json!({"kind": "reply", "message_id": message,
                "cc": [{"name": "Ana Lopes", "address": "ana@example.com"}],
                "body": "**Done**, notes attached <script>x</script>",
                "quoted": "On Sun, Maya Okafor wrote:\n> Could you review it?"})),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{:?}", draft.json);
    format!("{uri}/{}", draft.json["id"].as_str().unwrap())
}

/// Send the draft at `uri` as it reads now: its version and the content
/// token that came with it.
async fn send(kit: &Kit, token: &str, uri: &str) -> campbooks_kernel::testing::TestResponse {
    let shown = kit.get(token, uri).await.json;
    send_shown(kit, token, uri, &shown).await
}

/// Send the draft at `uri` as `shown` showed it (a read or a save's answer).
async fn send_shown(
    kit: &Kit,
    token: &str,
    uri: &str,
    shown: &Value,
) -> campbooks_kernel::testing::TestResponse {
    kit.post(
        token,
        &format!("{uri}/send"),
        Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
    )
    .await
}

/// Save `changes` to the draft at `uri` as it reads now: its version and
/// the content token that came with it.
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

/// Run `sql` (binding the draft at `uri`) as the runtime role in
/// `workspace`: what an attacker with SQL in the workspace can do.
async fn as_runtime(kit: &Kit, workspace: WorkspaceId, uri: &str, sql: &'static str) {
    let draft: sqlx::types::Uuid = uri.rsplit('/').next().unwrap().parse().unwrap();
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(sql)
        .bind(draft)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// What a refused send must not have done: the draft is still a draft, with
/// no request bound, no release queued and nothing requested.
async fn nothing_scheduled(kit: &Kit, uri: &str) {
    let draft: sqlx::types::Uuid = uri.rsplit('/').next().unwrap().parse().unwrap();
    let (state, mac, generation): (String, Option<Vec<u8>>, i32) = sqlx::query_as(
        "SELECT state, request_mac, send_generation FROM conversations.drafts WHERE id = $1",
    )
    .bind(draft)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!((state.as_str(), mac, generation), ("draft", None, 0));
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.release_send'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    window_ends(kit).await;
    assert!(kit.events("conversations.send_requested").await.is_empty());
}

/// Make every pending release due now and run the jobs.
async fn window_ends(kit: &Kit) {
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
}

/// The connector reports how send `send_id` ended.
async fn complete(kit: &Kit, workspace: WorkspaceId, inbox: InboxId, send_id: &str, sent: bool) {
    let event = SendCompleted {
        workspace: workspace.to_string(),
        inbox_id: inbox,
        send_id: send_id.parse::<SendId>().unwrap(),
        outcome: if sent {
            SendOutcome::Sent
        } else {
            SendOutcome::Failed
        },
        provider_message_id: sent.then(|| "gmail-sent-1".to_owned()),
        reason: (!sent).then(|| "rejected".to_owned()),
    };
    let (schemas, index) = contract("connectors.send_completed.v1.schema.json");
    schemas
        .validate(&serde_json::to_value(&event).unwrap(), index)
        .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    if let Some(workspace) = campbooks_kernel::DomainEvent::workspace(&event) {
        campbooks_kernel::db::set_workspace(&mut tx, workspace)
            .await
            .unwrap();
    }
    kit.state.events.publish(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn send_waits_out_the_window_then_requests_the_send_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;

    let sent = send(&kit, &sam, &draft).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    assert_eq!(sent.json["state"], "scheduled");
    assert!(sent.json["send_at"].is_string());
    // Scheduled drafts cannot change or be sent twice.
    let again = send(&kit, &sam, &draft).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "draft_not_editable");
    let edit = kit
        .patch(&sam, &draft, json!({"version": 1, "body": "x"}))
        .await;
    assert_eq!(edit.json["code"], "draft_not_editable");
    // Inside the window nothing leaves.
    kit.run_jobs().await;
    assert!(kit.events("conversations.send_requested").await.is_empty());

    window_ends(&kit).await;
    let requested = kit.events("conversations.send_requested").await;
    assert_eq!(requested.len(), 1);
    let (schemas, index) = contract("conversations.send_requested.v1.schema.json");
    schemas.validate(&requested[0], index).unwrap();
    let event = requested[0].to_string();
    for private in ["ana@example.com", "Done", "Q4 plan"] {
        assert!(!event.contains(private), "{private} in {event}");
    }

    // The outgoing document.
    let blob: BlobRef = serde_json::from_value(requested[0]["message"].clone()).unwrap();
    let document: Value = serde_json::from_str(&kit.read(workspace, &blob).await).unwrap();
    let (schemas, index) = contract("outgoing-message.v1.schema.json");
    schemas.validate(&document, index).unwrap();
    assert_eq!(document["from"]["address"], "sam@example.com");
    assert_eq!(
        document["to"][0]["address"],
        "maya@northwind-studio.example"
    );
    assert_eq!(document["cc"][0]["address"], "ana@example.com");
    assert_eq!(document["in_reply_to"], "q4-plan-1@studio.example");
    assert_eq!(document["references"], json!(["q4-plan-1@studio.example"]));
    assert_eq!(document["thread_key"], "thr-en-q4");
    let send_id = requested[0]["send_id"].as_str().unwrap().to_owned();
    assert_eq!(document["rfc_message_id"], format!("{send_id}@example.com"));
    let text = document["text"].as_str().unwrap();
    assert!(text.starts_with("**Done**, notes attached <script>x</script>\n\nOn Sun"));
    let html = document["html"].as_str().unwrap();
    assert!(html.contains("<strong>Done</strong>"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
    assert!(html.contains("<blockquote><p>Could you review it?</p></blockquote>"));

    let shown = kit.get(&sam, &draft).await;
    assert_eq!(shown.json["state"], "sending");
    let undo = kit.post(&sam, &format!("{draft}/cancel_send"), None).await;
    assert_eq!(undo.status, StatusCode::CONFLICT);
    assert_eq!(undo.json["code"], "too_late");
    // Running again releases nothing new.
    window_ends(&kit).await;
    assert_eq!(kit.events("conversations.send_requested").await.len(), 1);

    // Sent: the draft is gone, and so is what it held.
    complete(&kit, workspace, inbox, &send_id, true).await;
    assert_eq!(kit.get(&sam, &draft).await.status, StatusCode::NOT_FOUND);
    let sent_events = kit.events("conversations.draft_sent").await;
    assert_eq!(sent_events.len(), 1);
    assert_eq!(
        sent_events[0]["conversation"],
        kit.conversation_of("en-1").await
    );
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'conversations.draft'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 0);
    // A repeated report changes nothing.
    complete(&kit, workspace, inbox, &send_id, true).await;
    assert_eq!(kit.events("conversations.draft_sent").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_scheduled_send_is_never_discarded_only_cancelled(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;

    let sent = send(&kit, &sam, &draft).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    // A discard of a send on its way (a stale screen, the assistant): refused,
    // and it stays scheduled (campbooks-cloud#242).
    let discarded = kit.delete(&sam, &draft).await;
    assert_eq!(
        discarded.status,
        StatusCode::CONFLICT,
        "{:?}",
        discarded.json
    );
    assert_eq!(discarded.json["code"], "draft_not_editable");
    assert_eq!(kit.get(&sam, &draft).await.json["state"], "scheduled");
    // Its Undo, the deliberate cancel, makes it a draft again: then it goes.
    let undone = kit.post(&sam, &format!("{draft}/cancel_send"), None).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(
        kit.delete(&sam, &draft).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(kit.get(&sam, &draft).await.status, StatusCode::NOT_FOUND);
    window_ends(&kit).await;
    assert!(kit.events("conversations.send_requested").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn undo_then_send_again_releases_only_the_second(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;

    send(&kit, &sam, &draft).await;
    let undone = kit.post(&sam, &format!("{draft}/cancel_send"), None).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json["state"], "draft");
    assert!(undone.json["send_at"].is_null());
    // The draft shown again after Undo carries its own token.
    let edited = kit
        .patch(
            &sam,
            &draft,
            json!({"version": undone.json["version"], "body": "Second thoughts",
                "content_token": undone.json["content_token"]}),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.json);
    // The cancelled release finds nothing to do.
    window_ends(&kit).await;
    assert!(kit.events("conversations.send_requested").await.is_empty());

    send(&kit, &sam, &draft).await;
    window_ends(&kit).await;
    let requested = kit.events("conversations.send_requested").await;
    assert_eq!(requested.len(), 1);
    let blob: BlobRef = serde_json::from_value(requested[0]["message"].clone()).unwrap();
    let document: Value = serde_json::from_str(&kit.read(workspace, &blob).await).unwrap();
    assert!(
        document["text"]
            .as_str()
            .unwrap()
            .starts_with("Second thoughts")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_scheduled_send_stays_put_once_its_author_left(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let sent = send(&kit, &sam, &draft).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    // Removed from the workspace before the window ends, and before the
    // departure is handled (the inbox membership is still there).
    kit.leave("sam", workspace);
    window_ends(&kit).await;
    assert!(kit.events("conversations.send_requested").await.is_empty());
    let state: String = sqlx::query_scalar("SELECT state FROM conversations.drafts")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(state, "draft");
    // Tracked as taken back, by the release, on Sam's behalf.
    let back = tracked_events(&kit.db, "conversations.draft_unscheduled").await;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].envelope["actor"]["type"], "system");
    assert_eq!(back[0].body["reason"], "sender_may_not_send");
    assert_eq!(back[0].body["draft"]["state"], "draft");
    assert!(
        tracked_events(&kit.db, "conversations.draft_released")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_failed_send_can_be_fixed_and_sent_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    send(&kit, &sam, &draft).await;
    window_ends(&kit).await;
    let first = kit.events("conversations.send_requested").await[0]["send_id"]
        .as_str()
        .unwrap()
        .to_owned();
    complete(&kit, workspace, inbox, &first, false).await;
    let failed = kit.get(&sam, &draft).await;
    assert_eq!(failed.json["state"], "failed");
    assert_eq!(failed.json["failure_reason"], "rejected");
    assert_eq!(kit.events("conversations.send_failed").await.len(), 1);

    let resent = send(&kit, &sam, &draft).await;
    assert_eq!(resent.status, StatusCode::ACCEPTED);
    window_ends(&kit).await;
    let requested = kit.events("conversations.send_requested").await;
    assert_eq!(requested.len(), 2);
    assert_ne!(requested[1]["send_id"], requested[0]["send_id"]);
}

/// The tracked events named `name`.
async fn tracked(kit: &Kit, name: &str) -> Vec<campbooks_kernel::testing::TrackedRow> {
    tracked_events(&kit.db, name).await
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_is_tracked_by_ids_states_and_counts_from_scheduling_to_the_provider(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let id = draft.rsplit('/').next().unwrap().to_owned();
    let created = tracked(&kit, "conversations.draft_created").await;
    assert_eq!(created.len(), 1);
    let snapshot = &created[0].body["draft"];
    assert_eq!(snapshot["kind"], "reply");
    assert_eq!(snapshot["to_count"], 1);
    assert_eq!(snapshot["cc_count"], 1);
    assert_eq!(
        snapshot["conversation_id"],
        kit.conversation_of("en-1").await.as_str()
    );
    assert_eq!(
        snapshot["message_id"],
        kit.message_of("en-1").await.as_str()
    );

    // Scheduled, then taken back (Undo): a draft again.
    let sent = send(&kit, &sam, &draft).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    let scheduled = tracked(&kit, "conversations.draft_scheduled").await;
    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        scheduled[0].envelope["actor"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(
        scheduled[0].envelope["operation"],
        "conversations.send_draft"
    );
    assert_eq!(scheduled[0].body["draft"]["id"], id.as_str());
    assert_eq!(scheduled[0].body["draft"]["state"], "scheduled");
    assert!(scheduled[0].body["draft"]["send_at"].is_string());
    let cancel = format!("{draft}/cancel_send");
    let undone = kit.post(&sam, &cancel, None).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    // Nothing scheduled any more: nothing more to take back.
    kit.post(&sam, &cancel, None).await;
    let back = tracked(&kit, "conversations.draft_unscheduled").await;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].envelope["operation"], "conversations.cancel_send");
    assert_eq!(back[0].body["reason"], "cancelled");
    assert_eq!(back[0].body["draft"]["state"], "draft");

    // Sent again and released: handed to the connector, on Sam's behalf.
    send(&kit, &sam, &draft).await;
    window_ends(&kit).await;
    let released = tracked(&kit, "conversations.draft_released").await;
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].envelope["actor"]["type"], "system");
    assert_eq!(
        released[0].envelope["on_behalf_of"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(released[0].body["draft"]["state"], "sending");
    let first = kit.events("conversations.send_requested").await[0]["send_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // The provider refused it: an error kind, never its words.
    complete(&kit, workspace, inbox, &first, false).await;
    let failed = tracked(&kit, "conversations.draft_send_failed").await;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].body["error"], "rejected");
    assert_eq!(failed[0].body["draft"]["state"], "failed");
    assert_eq!(failed[0].body["draft"]["failure"], "rejected");

    // Fixed and sent: the provider sent it, and the draft is gone.
    send(&kit, &sam, &draft).await;
    window_ends(&kit).await;
    let requested = kit.events("conversations.send_requested").await;
    let second = requested[1]["send_id"].as_str().unwrap().to_owned();
    complete(&kit, workspace, inbox, &second, true).await;
    let gone = tracked(&kit, "conversations.draft_sent").await;
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].envelope["actor"]["type"], "system");
    assert_eq!(gone[0].body["draft"]["id"], id.as_str());
    assert_eq!(gone[0].body["draft"]["state"], "sent");
    assert_eq!(
        tracked(&kit, "conversations.draft_scheduled").await.len(),
        3
    );
    assert_eq!(tracked(&kit, "conversations.draft_released").await.len(), 2);

    // Who it went to and what it said are never in an event.
    for name in [
        "conversations.draft_created",
        "conversations.draft_scheduled",
        "conversations.draft_unscheduled",
        "conversations.draft_released",
        "conversations.draft_send_failed",
        "conversations.draft_sent",
    ] {
        for event in tracked(&kit, name).await {
            let text = event.body.to_string();
            for private in ["@", "Done", "Q4 plan", "Maya", "Ana"] {
                assert!(!text.contains(private), "{private} in {name}: {text}");
            }
        }
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_needs_valid_recipients(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let draft = kit
        .post(&sam, &uri, Some(json!({"kind": "new", "inbox_id": inbox})))
        .await;
    let one = format!("{uri}/{}", draft.json["id"].as_str().unwrap());
    let response = send(&kit, &sam, &one).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json["code"], "no_recipients");
    save(
        &kit,
        &sam,
        &one,
        json!({"to": [{"address": "not an address"}]}),
    )
    .await;
    let response = send(&kit, &sam, &one).await;
    assert_eq!(response.json["code"], "invalid_address");
    assert_eq!(kit.get(&sam, &one).await.json["state"], "draft");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_names_the_version_it_sends(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    // Saved elsewhere since the composer last saved: refuse rather than send
    // something the member did not see.
    let saved = save(&kit, &sam, &draft, json!({"body": "Changed"})).await;
    assert_eq!(saved.json["version"], 2);
    let stale = kit
        .post(&sam, &format!("{draft}/send"), Some(json!({"version": 1})))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "draft_changed");
    assert_eq!(kit.get(&sam, &draft).await.json["state"], "draft");
    let no_version = kit.post(&sam, &format!("{draft}/send"), None).await;
    assert_ne!(no_version.status, StatusCode::ACCEPTED);
    assert_eq!(send(&kit, &sam, &draft).await.status, StatusCode::ACCEPTED);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_that_never_reports_back_stops_waiting(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    send(&kit, &sam, &draft).await;
    window_ends(&kit).await;
    let send_id = kit.events("conversations.send_requested").await[0]["send_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(kit.get(&sam, &draft).await.json["state"], "sending");

    // An hour on, still nothing: the draft is editable again, honestly.
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.check_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    let stopped = kit.get(&sam, &draft).await;
    assert_eq!(stopped.json["state"], "failed");
    assert_eq!(stopped.json["failure_reason"], "unknown_outcome");
    assert_eq!(kit.events("conversations.send_failed").await.len(), 1);
    let failed = tracked_events(&kit.db, "conversations.draft_send_failed").await;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].envelope["actor"]["type"], "system");
    assert_eq!(failed[0].body["error"], "unknown_outcome");
    assert_eq!(failed[0].body["draft"]["state"], "failed");
    assert_eq!(failed[0].body["draft"]["failure"], "unknown_outcome");

    // A late report still counts: it was sent after all.
    complete(&kit, workspace, inbox, &send_id, true).await;
    assert_eq!(kit.get(&sam, &draft).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.events("conversations.draft_sent").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bcc_added_with_sql_after_the_draft_was_shown_is_not_sent(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let shown = kit.get(&sam, &draft).await.json;
    assert!(shown["content_token"].is_string(), "{shown}");
    // SQL adds a Bcc and changes the body, keeping the version: what the
    // `version` check alone let through.
    as_runtime(
        &kit,
        workspace,
        &draft,
        r#"UPDATE conversations.drafts
              SET bcc_recipients = '[{"name": null, "address": "eve@example.com"}]'::jsonb
            WHERE id = $1"#,
    )
    .await;
    let refused = send_shown(&kit, &sam, &draft, &shown).await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.json);
    assert_eq!(refused.json["code"], "content_changed");
    nothing_scheduled(&kit, &draft).await;
    as_runtime(
        &kit,
        workspace,
        &draft,
        "UPDATE conversations.drafts SET body = 'Send the files to eve' WHERE id = $1",
    )
    .await;
    let refused = send_shown(&kit, &sam, &draft, &shown).await;
    assert_eq!(refused.json["code"], "content_changed");
    nothing_scheduled(&kit, &draft).await;

    // Read again, the draft shows the Bcc: sent now, it is what Sam saw.
    let again = kit.get(&sam, &draft).await.json;
    assert_eq!(again["bcc"][0]["address"], "eve@example.com");
    assert_ne!(again["content_token"], shown["content_token"]);
    let sent = send_shown(&kit, &sam, &draft, &again).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_takes_the_token_of_the_draft_as_shown_to_its_author(db: PgPool) {
    use campbooks_kernel::tenancy::Access;

    use crate::domain::{binding::ContentToken, drafts};

    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let shown = kit.get(&sam, &draft).await.json;
    // No token (a composer opened before tokens existed): refused, once.
    let missing = kit
        .post(
            &sam,
            &format!("{draft}/send"),
            Some(json!({"version": shown["version"]})),
        )
        .await;
    assert_eq!(missing.status, StatusCode::CONFLICT, "{:?}", missing.json);
    assert_eq!(missing.json["code"], "content_changed");
    for garbage in ["", "not a token", "AQ"] {
        let refused = kit
            .post(
                &sam,
                &format!("{draft}/send"),
                Some(json!({"version": shown["version"], "content_token": garbage})),
            )
            .await;
        assert_eq!(refused.json["code"], "content_changed", "{garbage}");
    }
    nothing_scheduled(&kit, &draft).await;

    // The same draft, as it is, but the token of another member (Leo, who
    // sends from the same inbox).
    let id: crate::model::DraftId = draft.rsplit('/').next().unwrap().parse().unwrap();
    let author: sqlx::types::Uuid =
        sqlx::query_scalar("SELECT author FROM conversations.drafts WHERE id = $1")
            .bind(id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    let author = campbooks_kernel::tenancy::UserId::from_uuid(author);
    let access = |user| Access {
        workspace,
        user: Some(user),
        role: None,
    };
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let row = drafts::find(&mut tx, &access(author), id, false)
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    let (_, leo) = kit.member("leo", workspace);
    let another = ContentToken::of_draft(&access(leo), &row)
        .unwrap()
        .issue(&kit.state.binding);
    let refused = kit
        .post(
            &sam,
            &format!("{draft}/send"),
            Some(json!({"version": shown["version"], "content_token": another})),
        )
        .await;
    assert_eq!(refused.json["code"], "content_changed");
    nothing_scheduled(&kit, &draft).await;
    // Sam's own: sent.
    assert_eq!(
        send_shown(&kit, &sam, &draft, &shown).await.status,
        StatusCode::ACCEPTED
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_save_answers_with_the_token_of_what_it_saved(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let first = kit.get(&sam, &draft).await.json;
    let saved = kit
        .patch(
            &sam,
            &draft,
            json!({"version": first["version"], "body": "Done, see you Thursday",
                "content_token": first["content_token"]}),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json);
    // The token of before the save, at the new version: refused.
    let stale = kit
        .post(
            &sam,
            &format!("{draft}/send"),
            Some(
                json!({"version": saved.json["version"], "content_token": first["content_token"]}),
            ),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "content_changed");
    nothing_scheduled(&kit, &draft).await;
    // The save's own token: sent, with what was saved.
    let sent = send_shown(&kit, &sam, &draft, &saved.json).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    window_ends(&kit).await;
    let requested = kit.events("conversations.send_requested").await;
    assert_eq!(requested.len(), 1);
    let blob: BlobRef = serde_json::from_value(requested[0]["message"].clone()).unwrap();
    let document: Value = serde_json::from_str(&kit.read(workspace, &blob).await).unwrap();
    assert!(
        document["text"]
            .as_str()
            .unwrap()
            .starts_with("Done, see you Thursday")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_save_keeps_only_what_was_shown(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let message = kit.message_of("en-1").await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let forward = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "forward", "message_id": message,
                "to": [{"address": "leo@example.com"}]})),
        )
        .await;
    assert_eq!(forward.status, StatusCode::CREATED, "{:?}", forward.json);
    let draft = format!("{uri}/{}", forward.json["id"].as_str().unwrap());
    let shown = forward.json;
    let attachment = shown["attachments"][0]["id"].clone();
    // Between the open and the next autosave, SQL changes what no save
    // sends: an attachment's file, the quoted text, the reply headers.
    for sql in [
        r#"UPDATE conversations.drafts
              SET attachments = jsonb_set(attachments, '{0,filename}', '"invoice.pdf.exe"')
            WHERE id = $1"#,
        "UPDATE conversations.drafts SET quoted = 'Pay the new account' WHERE id = $1",
        "UPDATE conversations.drafts SET in_reply_to = 'planted@example.com' WHERE id = $1",
    ] {
        as_runtime(&kit, workspace, &draft, sql).await;
        let autosave = kit
            .patch(
                &sam,
                &draft,
                json!({"version": shown["version"], "content_token": shown["content_token"],
                    "to": [{"address": "leo@example.com"}], "subject": shown["subject"],
                    "body": "Here it is", "attachments": [attachment]}),
            )
            .await;
        assert_eq!(
            autosave.status,
            StatusCode::CONFLICT,
            "{sql}: {:?}",
            autosave.json
        );
        assert_eq!(autosave.json["code"], "content_changed");
        // Nothing saved: no new version, no new token for what was planted.
        let row: (i32, String) =
            sqlx::query_as("SELECT version, body FROM conversations.drafts WHERE id = $1")
                .bind(
                    draft
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .parse::<sqlx::types::Uuid>()
                        .unwrap(),
                )
                .fetch_one(&kit.db)
                .await
                .unwrap();
        assert_eq!(row, (1, String::new()));
        // And a send with what Sam was shown signs nothing.
        let sent = send_shown(&kit, &sam, &draft, &shown).await;
        assert_eq!(sent.json["code"], "content_changed");
        nothing_scheduled(&kit, &draft).await;
    }
    // Read again (the composer shows the attachment as it now is): saved.
    let again = save(&kit, &sam, &draft, json!({"body": "Here it is"})).await;
    assert_eq!(again.status, StatusCode::OK, "{:?}", again.json);
    assert_eq!(again.json["attachments"][0]["filename"], "invoice.pdf.exe");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_drafts_token_does_not_hold(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let one = reply(&kit, workspace, &sam).await;
    let other = reply(&kit, workspace, &sam).await;
    let theirs = kit.get(&sam, &other).await.json;
    let mine = kit.get(&sam, &one).await.json;
    assert_eq!(mine["version"], theirs["version"]);
    // The other draft's token, same version, same author: not this draft's.
    let saved = kit
        .patch(
            &sam,
            &one,
            json!({"version": mine["version"], "content_token": theirs["content_token"],
                "body": "x"}),
        )
        .await;
    assert_eq!(saved.status, StatusCode::CONFLICT, "{:?}", saved.json);
    assert_eq!(saved.json["code"], "content_changed");
    let sent = kit
        .post(
            &sam,
            &format!("{one}/send"),
            Some(json!({"version": mine["version"], "content_token": theirs["content_token"]})),
        )
        .await;
    assert_eq!(sent.json["code"], "content_changed");
    nothing_scheduled(&kit, &one).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_autosave_takes_the_token_of_the_last_save(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;
    let opened = kit.get(&sam, &draft).await.json;
    let first = kit
        .patch(
            &sam,
            &draft,
            json!({"version": opened["version"], "content_token": opened["content_token"],
                "body": "Thursday"}),
        )
        .await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
    // The next autosave with the token of the open: refused, nothing saved.
    let stale = kit
        .patch(
            &sam,
            &draft,
            json!({"version": first.json["version"], "content_token": opened["content_token"],
                "body": "Thursday works"}),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{:?}", stale.json);
    assert_eq!(stale.json["code"], "content_changed");
    assert_eq!(kit.get(&sam, &draft).await.json["body"], "Thursday");
    // With the last save's: saved.
    let fresh = kit
        .patch(
            &sam,
            &draft,
            json!({"version": first.json["version"], "content_token": first.json["content_token"],
                "body": "Thursday works"}),
        )
        .await;
    assert_eq!(fresh.status, StatusCode::OK, "{:?}", fresh.json);
    assert_eq!(fresh.json["body"], "Thursday works");
}

/// The kernel refused the assistant's call before the handler ran: the
/// operation does not opt the assistant in.
fn not_the_assistants(result: &Result<Output, InvokeError>) {
    assert!(
        matches!(result, Err(InvokeError::NotAllowed { .. })),
        "{result:?}"
    );
}

/// The handler refused it on its own: 403 `send_needs_you`.
fn needs_the_member(result: &Result<Output, InvokeError>) {
    match result {
        Err(InvokeError::Failed {
            status, problem, ..
        }) => {
            assert_eq!(*status, StatusCode::FORBIDDEN, "{problem}");
            assert_eq!(problem["code"], "send_needs_you", "{problem}");
        }
        other => panic!("not refused: {other:?}"),
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistant_writes_a_draft_but_only_the_member_sends_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    // Scout writes Sam's reply and reads it back: the token it gets is Sam's.
    let message = kit.message_of("en-1").await;
    let written = kit
        .as_assistant(
            "sam",
            workspace,
            (
                "conversations.create_draft",
                json!({"kind": "reply", "message_id": message, "body": "Thursday works."}),
            ),
            false,
        )
        .await
        .unwrap();
    let id = written.body["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/v1/w/{workspace}/drafts/{id}");
    let read = kit
        .as_assistant(
            "sam",
            workspace,
            ("conversations.get_draft", json!({"draft_id": id})),
            false,
        )
        .await
        .unwrap();
    assert!(read.body["content_token"].is_string(), "{}", read.body);
    let input = json!({"draft_id": id, "version": read.body["version"],
        "content_token": read.body["content_token"]});
    // Sending it with Sam's token, as a call the user confirmed or not: the
    // kernel refuses it before the handler runs.
    for confirmed in [false, true] {
        not_the_assistants(
            &kit.as_assistant(
                "sam",
                workspace,
                ("conversations.send_draft", input.clone()),
                confirmed,
            )
            .await,
        );
    }
    // Listed for the assistant by mistake, the kernel still wants a
    // confirmation, and a confirmed call is refused by the handler itself.
    let unconfirmed = kit
        .as_assistant_listed(
            "sam",
            workspace,
            ("conversations.send_draft", input.clone()),
            false,
        )
        .await;
    assert!(
        matches!(unconfirmed, Err(InvokeError::ConfirmationRequired(_))),
        "{unconfirmed:?}"
    );
    needs_the_member(
        &kit.as_assistant_listed("sam", workspace, ("conversations.send_draft", input), true)
            .await,
    );
    // Nothing was scheduled, bound or asked of the connector.
    nothing_scheduled(&kit, &uri).await;

    // Sam's own Send, with the token his screen got: it leaves, once.
    let sent = send(&kit, &sam, &uri).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    window_ends(&kit).await;
    assert_eq!(kit.events("conversations.send_requested").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistant_never_takes_back_a_send_the_member_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let uri = reply(&kit, workspace, &sam).await;
    let sent = send(&kit, &sam, &uri).await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    let id = uri.rsplit('/').next().unwrap().to_owned();
    let cancel = ("conversations.cancel_send", json!({"draft_id": id}));
    for confirmed in [false, true] {
        not_the_assistants(
            &kit.as_assistant("sam", workspace, cancel.clone(), confirmed)
                .await,
        );
        needs_the_member(
            &kit.as_assistant_listed("sam", workspace, cancel.clone(), confirmed)
                .await,
        );
    }
    // Still on its way, as Sam sent it: it leaves when the window ends.
    let shown = kit.get(&sam, &uri).await;
    assert_eq!(shown.json["state"], "scheduled", "{:?}", shown.json);
    window_ends(&kit).await;
    assert_eq!(kit.events("conversations.send_requested").await.len(), 1);
}

/// The assistant's words never carry a link whose words hide where it goes
/// (campbooks-cloud#390): `[` and `]` it writes are kept as text, in a new
/// draft and in a save.
#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistants_words_carry_no_worded_link(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _sam) = kit.sams_inbox().await;
    let message = kit.message_of("en-1").await;
    let lure = "Pay [here](https://example.net/pay) today.";
    let written = kit
        .as_assistant(
            "sam",
            workspace,
            (
                "conversations.create_draft",
                json!({"kind": "reply", "message_id": message, "body": lure}),
            ),
            false,
        )
        .await
        .unwrap();
    let plain = r"Pay \[here\](https://example.net/pay) today.";
    assert_eq!(written.body["body"], plain, "{}", written.body);
    let id = written.body["id"].as_str().unwrap().to_owned();
    let saved = kit
        .as_assistant(
            "sam",
            workspace,
            (
                "conversations.update_draft",
                json!({"draft_id": id, "version": written.body["version"],
                    "content_token": written.body["content_token"], "body": lure}),
            ),
            false,
        )
        .await
        .unwrap();
    assert_eq!(saved.body["body"], plain, "{}", saved.body);
}
