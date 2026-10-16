//! What a conversation needs, end to end over `fixtures/needs.json`.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    ConversationsDeps, dev,
    model::{InboxId, InboxRole},
    testkit::{Kit, contract, fixture, sorting_deps},
};

/// Sam's inbox holding `fixtures/needs.json`.
async fn sams_needs(kit: &Kit) -> (WorkspaceId, InboxId, String) {
    let workspace = WorkspaceId::new();
    let (token, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    (workspace, inbox, token)
}

async fn summary(kit: &Kit, token: &str, workspace: WorkspaceId, provider_id: &str) -> Value {
    let id = kit.conversation_of(provider_id).await;
    let list = kit
        .get(
            token,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    list.json["conversations"][0].clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_model_raises_one_need_per_conversation(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = sams_needs(&kit).await;

    // A person's question: a reply, with no due date until one is
    // extracted, and no quote (the model gives its answers, not a sentence).
    let marta = summary(&kit, &sam, workspace, "needs-1").await;
    assert_eq!(marta["need"]["reason"], "reply");
    assert_eq!(marta["need"]["trust"], "sure");
    assert_eq!(marta["need"]["basis"], "model");
    assert_eq!(marta["need"]["due_on"], Value::Null);
    assert_eq!(marta["need"]["rule"], "model.decide");
    assert_eq!(marta["need"]["quote"], Value::Null);

    let pedro = summary(&kit, &sam, workspace, "needs-2").await;
    assert_eq!(pedro["need"]["reason"], "do");

    // An invoice from a service, sorted to Receipts: pay.
    let bill = summary(&kit, &sam, workspace, "needs-3").await;
    assert_eq!(bill["stream"], "receipts");
    assert_eq!(bill["need"]["reason"], "pay");

    let phish = summary(&kit, &sam, workspace, "needs-4").await;
    assert_eq!(phish["need"]["reason"], "review");
    assert_eq!(phish["need"]["question"], "suspicious");

    let jonas = summary(&kit, &sam, workspace, "needs-5").await;
    assert_eq!(jonas["need"]["reason"], "reply");

    // Every answer accepted: nothing is a question.
    let events = kit.events("conversations.need_changed").await;
    assert_eq!(events.len(), 5, "{events:?}");
    assert!(events.iter().all(|e| e["need"]["trust"] == "sure"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn need_events_follow_the_contract_and_hold_no_personal_data(db: PgPool) {
    let (schemas, index) = contract("conversations.need_changed.v1.schema.json");
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-3").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}/need");
    kit.post(
        &sam,
        &format!("{base}/resolve"),
        Some(json!({"outcome": "done", "message_id": kit.message_of("needs-3").await})),
    )
    .await;
    kit.ingest(workspace, inbox, "needs-replied").await;
    let events = kit.events("conversations.need_changed").await;
    assert!(events.len() >= 7, "{events:?}");
    for event in &events {
        if let Err(error) = schemas.validate(event, index) {
            panic!("{event} breaks the contract: {error}");
        }
        let text = event.to_string();
        for personal in [
            "Marta",
            "marta@",
            "Pedro",
            "walkthrough",
            "INV-2041",
            "Jonas",
        ] {
            assert!(!text.contains(personal), "{personal} in {text}");
        }
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn done_and_undo(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-2").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}/need");

    let done = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "done", "message_id": kit.message_of("needs-2").await})),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert_eq!(done.json["need"], Value::Null);
    let cleared = kit.events("conversations.need_changed").await;
    let last = cleared.last().unwrap();
    assert_eq!(last["need"], Value::Null);
    assert_eq!(last["cleared_because"], "done");
    assert!(last["cleared_by"].is_string());

    let again = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "done", "message_id": kit.message_of("needs-2").await})),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "no_open_need");

    let back = kit.post(&sam, &format!("{base}/reopen"), None).await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(back.json["need"]["reason"], "do");
    let nothing = kit.post(&sam, &format!("{base}/reopen"), None).await;
    assert_eq!(nothing.status, StatusCode::CONFLICT);
    assert_eq!(nothing.json["code"], "nothing_to_reopen");
    let revisions: Vec<i64> = kit
        .events("conversations.need_changed")
        .await
        .iter()
        .filter(|e| e["conversation"] == id.as_str())
        .map(|e| e["revision"].as_i64().unwrap())
        .collect();
    assert_eq!(revisions, [1, 2, 3]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_needed_teaches_and_undo_withdraws_it(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-5").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}/need");

    kit.post(
        &sam,
        &format!("{base}/resolve"),
        Some(json!({"outcome": "not_needed", "message_id": kit.message_of("needs-5").await})),
    )
    .await;
    let (question, from, to, source, withdrawn): (String, String, String, String, bool) =
        sqlx::query_as(
            "SELECT question, from_value, to_value, overridden_source, withdrawn_at IS NOT NULL
               FROM conversations.corrections",
        )
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(
        (
            question.as_str(),
            from.as_str(),
            to.as_str(),
            source.as_str(),
            withdrawn
        ),
        ("needs_reply", "true", "false", "model", false)
    );
    let last = kit.events("conversations.need_changed").await;
    assert_eq!(last.last().unwrap()["cleared_because"], "not_needed");

    kit.post(&sam, &format!("{base}/reopen"), None).await;
    let withdrawn: bool =
        sqlx::query_scalar("SELECT withdrawn_at IS NOT NULL FROM conversations.corrections")
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(withdrawn);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn it_is_safe_corrects_the_warning(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-4").await;
    let safe = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/need/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": kit.message_of("needs-4").await})),
        )
        .await;
    assert_eq!(safe.json["need"], Value::Null);
    let (question, to): (String, String) =
        sqlx::query_as("SELECT question, to_value FROM conversations.corrections")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((question.as_str(), to.as_str()), ("suspicious", "false"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_from_another_client_clears_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = sams_needs(&kit).await;
    kit.ingest(workspace, inbox, "needs-replied").await;
    let jonas = summary(&kit, &sam, workspace, "needs-5").await;
    assert_eq!(jonas["need"], Value::Null);
    let events = kit.events("conversations.need_changed").await;
    assert_eq!(events.last().unwrap()["cleared_because"], "replied");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn archiving_keeps_bills_and_clears_replies(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    for (provider_id, kept) in [("needs-3", true), ("needs-2", true), ("needs-1", false)] {
        let id = kit.conversation_of(provider_id).await;
        let archived = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/conversations/{id}/archive"),
                None,
            )
            .await;
        assert_eq!(archived.json["archived"], true);
        assert_eq!(!archived.json["need"].is_null(), kept, "{provider_id}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn done_in_a_shared_inbox_clears_it_for_everyone(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = sams_needs(&kit).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let id = kit.conversation_of("needs-1").await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{id}/need/resolve"),
        Some(json!({"outcome": "done", "message_id": kit.message_of("needs-1").await})),
    )
    .await;
    let seen_by_leo = summary(&kit, &leo, workspace, "needs-1").await;
    assert_eq!(seen_by_leo["need"], Value::Null);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn readers_and_strangers_cannot_resolve(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = sams_needs(&kit).await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let (eve, _) = kit.member("eve", workspace);
    let id = kit.conversation_of("needs-1").await;
    let uri = format!("/api/v1/w/{workspace}/conversations/{id}/need/resolve");
    let body = Some(json!({"outcome": "done", "message_id": kit.message_of("needs-1").await}));
    let denied = kit.post(&ana, &uri, body.clone()).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(denied.json["code"], "inbox_read_only");
    assert_eq!(
        kit.post(&eve, &uri, body).await.status,
        StatusCode::NOT_FOUND
    );
    let bad = kit
        .post(
            &ana,
            &uri,
            Some(json!({"outcome": "later", "message_id": kit.message_of("needs-1").await})),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn old_mail_raises_nothing(db: PgPool) {
    let mut deps: ConversationsDeps = sorting_deps();
    deps.config.needs_window_days = crate::DEFAULT_NEEDS_WINDOW_DAYS;
    let kit = Kit::with_deps(db, deps).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    // The same mail, sent six years ago: a first sync of an old mailbox.
    let old: Value = serde_json::from_str(&fixture("needs").replace("\"2026-", "\"2020-")).unwrap();
    let batch = dev::record(&kit.state, workspace, inbox, &old)
        .await
        .unwrap();
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;
    assert!(kit.events("conversations.need_changed").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn listing_by_ids_leaves_out_what_you_cannot_see(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let (eve, _) = kit.member("eve", workspace);
    let ids = [
        kit.conversation_of("needs-1").await,
        kit.conversation_of("needs-2").await,
    ];
    let uri = format!("/api/v1/w/{workspace}/conversations?ids={}", ids.join(","));
    let list = kit.get(&sam, &uri).await;
    assert_eq!(list.json["conversations"].as_array().unwrap().len(), 2);
    let none = kit.get(&eve, &uri).await;
    assert_eq!(none.json["conversations"].as_array().unwrap().len(), 0);
    let too_many = vec![ids[0].clone(); 101].join(",");
    let bad = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations?ids={too_many}"),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_ids");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn acting_on_a_need_that_changed_records_nothing(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-1").await;
    // The member saw the need of another (older) message.
    let stale = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/need/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": kit.message_of("needs-2").await})),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "need_changed");
    let corrections: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.corrections")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(corrections, 0);
    let marta = summary(&kit, &sam, workspace, "needs-1").await;
    assert_eq!(marta["need"]["reason"], "reply");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_sender_writing_again_is_announced_even_without_a_new_need(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = sams_needs(&kit).await;
    let before = kit.events("conversations.need_changed").await.len();
    kit.ingest(workspace, inbox, "needs-followup").await;
    let events = kit.events("conversations.need_changed").await;
    assert_eq!(events.len(), before + 1, "{events:?}");
    let last = events.last().unwrap();
    // Same need (Pedro's lease), a newer message in the conversation.
    assert_eq!(last["need"]["reason"], "do");
    assert_eq!(last["message"], kit.message_of("needs-2").await.as_str());
    assert_eq!(
        last["latest_message"],
        kit.message_of("needs-7").await.as_str()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn spam_and_trash_need_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = sams_needs(&kit).await;
    // Pedro writes again about the lease (asking nothing new).
    kit.ingest(workspace, inbox, "needs-followup").await;
    let fixtures: Value = serde_json::from_str(&fixture("needs")).unwrap();
    let changes = fixtures["changes"].as_array().unwrap();
    // His first message, the one the need comes from, goes to the trash;
    // a stranger's question lands in spam inside Marta's conversation.
    let mut pedro = changes[1].clone();
    pedro["message"]["mailbox"] = json!({"in_inbox": false, "in_trash": true});
    let mut spam = changes[4].clone();
    spam["message"]["provider_message_id"] = json!("needs-spam");
    spam["message"]["thread_key"] = json!("thr-walkthrough");
    spam["message"]["rfc_message_id"] = json!("needs-spam@needs.example");
    spam["message"]["sent_at"] = json!("2026-10-12T18:00:00Z");
    spam["message"]["mailbox"] = json!({"in_inbox": false, "in_spam": true});
    let recorded = json!({"changes": [pedro, spam]});
    let batch = dev::record(&kit.state, workspace, inbox, &recorded)
        .await
        .unwrap();
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;

    // The lease's need goes with its message.
    let lease = summary(&kit, &sam, workspace, "needs-2").await;
    assert_eq!(lease["need"], Value::Null, "{lease}");
    let events = kit.events("conversations.need_changed").await;
    let pedro = kit.conversation_of("needs-2").await;
    let last = events
        .iter()
        .rfind(|e| e["conversation"] == pedro.as_str())
        .unwrap();
    assert_eq!(last["cleared_because"], "trashed");
    // Marta's need still comes from her message, not from the spam.
    let marta = summary(&kit, &sam, workspace, "needs-1").await;
    assert_eq!(
        marta["need"]["message_id"],
        kit.message_of("needs-1").await.as_str()
    );
}
