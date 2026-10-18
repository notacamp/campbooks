use axum::http::StatusCode;
use campbooks_kernel::{blobs::BlobRef, db::BeginFor as _, tenancy::WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

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
async fn undo_then_send_again_releases_only_the_second(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let draft = reply(&kit, workspace, &sam).await;

    send(&kit, &sam, &draft).await;
    let undone = kit.post(&sam, &format!("{draft}/cancel_send"), None).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json["state"], "draft");
    assert!(undone.json["send_at"].is_null());
    let version = undone.json["version"].clone();
    let edited = kit
        .patch(
            &sam,
            &draft,
            json!({"version": version, "body": "Second thoughts"}),
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
    let kit = Kit::new(db).await;
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
    kit.patch(
        &sam,
        &one,
        json!({"version": 1, "to": [{"address": "not an address"}]}),
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
    let saved = kit
        .patch(&sam, &draft, json!({"version": 1, "body": "Changed"}))
        .await;
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
    let kit = Kit::new(db).await;
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
            json!({"version": first["version"], "body": "Done, see you Thursday"}),
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
