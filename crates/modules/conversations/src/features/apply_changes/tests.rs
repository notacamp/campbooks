//! `conversations.apply_changes` against the recorded batches in `fixtures/`.

use std::collections::HashSet;

use campbooks_kernel::{blobs::BlobRef, tenancy::WorkspaceId};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    ConversationsDeps, dev,
    documents::ParticipantsDocument,
    events::{MessageReceived, MessageSent},
    model::InboxId,
    testkit::{Kit, fixture},
};

const FIXTURES: [&str; 6] = ["en", "pt-PT", "es-ES", "fr", "followup", "late-parent"];

async fn setup(kit: &Kit, address: &str) -> (WorkspaceId, InboxId) {
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, address, &[]).await;
    (workspace, inbox)
}

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn recorded_batches_follow_the_contract(db: PgPool) {
    let (schemas, index) = contract("message-change-batch.v1.schema.json");

    let kit = Kit::new(db).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    for name in FIXTURES {
        let batch = kit.batch(workspace, inbox, name).await;
        let document = serde_json::to_value(&batch).unwrap();
        if let Err(error) = schemas.validate(&document, index) {
            panic!("fixture {name} breaks the contract: {error}");
        }
    }
    // And the schema rejects what it should.
    let bad = serde_json::json!({"version": 1, "inbox": inbox, "changes": [
        {"op": "upsert", "message": {"provider_message_id": "x"}}
    ]});
    assert!(schemas.validate(&bad, index).is_err());
    assert!(fixture("en").contains("example"));
}

/// A compiled contract from `contracts/`.
fn contract(file: &str) -> (boon::Schemas, boon::SchemaIndex) {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.enable_format_assertions();
    compiler.add_resource(file, schema).unwrap();
    let index = compiler.compile(file, &mut schemas).unwrap();
    (schemas, index)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn requested_changes_follow_the_contract(db: PgPool) {
    let (schemas, index) = contract("conversations.changes_requested.v1.schema.json");
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    for action in ["read", "unread", "star", "unstar", "archive", "unarchive"] {
        kit.post(&sam, &format!("{base}/{action}"), None).await;
    }
    kit.post(
        &sam,
        &format!("{base}/stream"),
        Some(serde_json::json!({"stream": "updates"})),
    )
    .await;
    kit.post(
        &sam,
        &format!("{base}/labels"),
        Some(serde_json::json!({"name": "Tax"})),
    )
    .await;
    let events = kit.events("conversations.changes_requested").await;
    let kinds: HashSet<&str> = events
        .iter()
        .flat_map(|e| e["items"].as_array().unwrap())
        .map(|i| i["change"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 8, "{kinds:?}");
    for event in &events {
        if let Err(error) = schemas.validate(event, index) {
            panic!("{event} breaks the contract: {error}");
        }
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_batch_threads_messages_stores_bodies_and_announces_them(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    kit.ingest(workspace, inbox, "en").await;

    let threads = kit.threads(inbox).await;
    assert_eq!(threads.len(), 4);
    assert_eq!(threads["en-1"], threads["en-2"], "same provider thread key");
    let distinct: HashSet<_> = threads.values().collect();
    assert_eq!(distinct.len(), 3);

    let received = kit.events("conversations.message_received").await;
    let sent = kit.events("conversations.message_sent").await;
    assert_eq!((received.len(), sent.len()), (3, 1));
    // No personal data in the retained log: addresses and subjects live in
    // the participants document.
    for event in received.iter().chain(&sent) {
        let text = event.to_string();
        assert!(
            !text.contains('@'),
            "an address leaked into an event: {text}"
        );
        assert!(!text.contains("Q4 plan"), "a subject leaked into an event");
    }
    let first: MessageReceived = serde_json::from_value(received[0].clone()).unwrap();
    assert_eq!(first.message.provider_message_id, "en-1");
    assert_eq!(first.message.attachment_count, 1);
    let participants = BlobRef {
        hash: first.message.participants.hash.parse().unwrap(),
        size: first.message.participants.size,
        content_type: first.message.participants.content_type.clone(),
    };
    let document: ParticipantsDocument =
        serde_json::from_str(&kit.read(workspace, &participants).await).unwrap();
    assert_eq!(document.from.address, "maya@northwind-studio.example");
    assert_eq!(document.cc.len(), 1);
    assert_eq!(document.subject, "Q4 plan: can you review by Friday?");
    let outbound: MessageSent = serde_json::from_value(sent[0].clone()).unwrap();
    assert_eq!(outbound.message.conversation, first.message.conversation);

    // Bodies, attachments and participants are held by their records.
    let held = count(
        &db,
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type LIKE 'conversations.%'",
    )
    .await;
    // en-1: participants, raw, text, html + 1 attachment; en-2: participants,
    // text; en-3: participants, text, html; en-4: participants, text.
    assert_eq!(held, 12);

    let (unread, count_msgs, labels): (i32, i32, i64) = sqlx::query_as(
        "SELECT c.unread_count, c.message_count,
                (SELECT count(*) FROM conversations.conversation_labels cl
                  WHERE cl.conversation_id = c.id)
           FROM conversations.conversations c
           JOIN conversations.messages m ON m.conversation_id = c.id
          WHERE m.provider_message_id = 'en-1'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((unread, count_msgs, labels), (1, 2, 1));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn applying_a_batch_again_changes_nothing(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    kit.ingest(workspace, inbox, "en").await;
    let before = kit.threads(inbox).await;
    let events = count(&db, "SELECT count(*) FROM kernel.events").await;
    let decisions = count(&db, "SELECT count(*) FROM conversations.decisions").await;
    // Six answers or abstentions for each of the three inbound messages.
    assert!(decisions > 0 && decisions <= 18, "{decisions}");

    kit.ingest(workspace, inbox, "en").await;
    assert_eq!(kit.threads(inbox).await, before);
    assert_eq!(
        count(&db, "SELECT count(*) FROM kernel.events").await,
        events
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM conversations.decisions").await,
        decisions
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn accepted_rules_answers_place_conversations_in_streams(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    for (name, address) in [
        ("en", "sam@example.com"),
        ("pt-PT", "ines.cardoso@example.com"),
        ("es-ES", "pedidos@example.com"),
        ("fr", "camille.martin@example.com"),
    ] {
        let (workspace, inbox) = setup(&kit, address).await;
        kit.ingest(workspace, inbox, name).await;
    }
    // What the shared email signals decide (campbooks_ai::decide::signals),
    // with stream placement enabled from 0.9 for this test. Invoices sent by
    // a person's address stay in Primary, as the labelling guide rules.
    for (message, stream) in [
        ("en-1", "primary"),
        ("en-3", "newsletters"),
        ("en-4", "updates"),
        ("pt-1", "receipts"),
        ("pt-2", "primary"),
        ("pt-3", "newsletters"),
        ("es-1", "primary"),
        ("es-2", "newsletters"),
        ("fr-1", "updates"),
        ("fr-2", "primary"),
        ("fr-3", "primary"),
    ] {
        assert_eq!(
            kit.stream_of(message).await,
            (stream.to_owned(), "rules".to_owned()),
            "{message}"
        );
    }
    // Every rules answer is on record with its reasons, acted on when it
    // placed the stream.
    let (acted, reasons): (bool, Value) = sqlx::query_as(
        "SELECT d.acted, d.reasons FROM conversations.decisions d
           JOIN conversations.messages m ON m.id = d.message_id
          WHERE m.provider_message_id = 'es-2' AND d.question = 'stream' AND d.source = 'rules'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(acted);
    assert_eq!(reasons[0]["rule"], "signals.list_header");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn under_the_baseline_policy_stream_answers_are_suggestions(db: PgPool) {
    let deps = ConversationsDeps::offline();
    let kit = Kit::with_deps(db.clone(), deps).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    kit.ingest(workspace, inbox, "en").await;
    // The newsletter's rules answer is on record, but the stream question
    // does not hold its target yet: it stays in Primary.
    assert_eq!(
        kit.stream_of("en-3").await,
        ("primary".to_owned(), "default".to_owned())
    );
    let (value, acted): (Value, bool) = sqlx::query_as(
        "SELECT d.value, d.acted FROM conversations.decisions d
           JOIN conversations.messages m ON m.id = d.message_id
          WHERE m.provider_message_id = 'en-3' AND d.question = 'stream'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((value, acted), (serde_json::json!("newsletters"), false));
    // Questions whose rules hold their target are marked acted on.
    let acted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.decisions
          WHERE acted AND question IN ('needs_reply', 'asks_action', 'has_deadline', 'urgency')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(acted > 0);
    let never: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.decisions
          WHERE acted AND question IN ('stream', 'suspicious')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(never, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn later_batches_update_flags_labels_and_remove_messages(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    kit.ingest(workspace, inbox, "en").await;
    kit.ingest(workspace, inbox, "followup").await;

    let changed = kit.events("conversations.conversation_changed").await;
    let by_change: Vec<(Value, Value)> = changed
        .iter()
        .map(|e| (e["changes"].clone(), e["origin"].clone()))
        .collect();
    assert!(by_change.contains(&(
        serde_json::json!(["read", "starred", "labels"]),
        "provider".into()
    )));
    assert!(by_change.contains(&(serde_json::json!(["read", "archived"]), "provider".into())));
    assert!(by_change.contains(&(
        serde_json::json!(["messages", "removed"]),
        "provider".into()
    )));
    let removed = changed
        .iter()
        .find(|e| e["changes"].as_array().unwrap().contains(&"removed".into()))
        .unwrap();
    assert!(removed.get("state").is_none());
    assert_eq!(removed["provider_message_ids"], serde_json::json!(["en-4"]));

    // en-4 and its conversation are gone, and its blobs are let go.
    assert!(!kit.threads(inbox).await.contains_key("en-4"));
    assert_eq!(
        count(&db, "SELECT count(*) FROM conversations.conversations").await,
        2
    );
    // The provider swapped label Clients for Board.
    let labels: Vec<String> = sqlx::query_scalar(
        "SELECT l.name FROM conversations.conversation_labels cl
           JOIN conversations.labels l ON l.id = cl.label_id ORDER BY l.name",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(labels, ["Board"]);
    let (unread, starred, archived): (i32, bool, bool) = sqlx::query_as(
        "SELECT c.unread_count, c.starred, c.archived_at IS NOT NULL
           FROM conversations.conversations c
           JOIN conversations.messages m ON m.conversation_id = c.id
          WHERE m.provider_message_id = 'en-3'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((unread, starred, archived), (0, false, true));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_late_parent_joins_and_merges_its_replies(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "team@example.com").await;
    kit.ingest(workspace, inbox, "late-parent").await;

    let threads = kit.threads(inbox).await;
    let distinct: HashSet<_> = threads.values().collect();
    assert_eq!(distinct.len(), 1, "{threads:?}");
    assert_eq!(
        count(&db, "SELECT count(*) FROM conversations.conversations").await,
        1
    );
    let merged = kit.events("conversations.conversation_changed").await;
    assert_eq!(merged.len(), 2, "{merged:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_batch_for_another_inbox_or_version_fails_for_good(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    let mut batch = kit.batch(workspace, inbox, "en").await;
    batch.version = 2;
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    let stats = campbooks_kernel::jobs::Worker::new(kit.state.clone())
        .run_until_idle()
        .await;
    assert_eq!(stats.failed, 1);
    assert_eq!(
        count(&db, "SELECT count(*) FROM conversations.messages").await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_batch_for_an_inbox_not_created_yet_waits(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    let batch = kit.batch(workspace, InboxId::new(), "en").await;
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;
    let (state, error): (String, Option<String>) = sqlx::query_as(
        "SELECT state, last_error FROM kernel.jobs WHERE kind = 'conversations.apply_changes'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(state, "pending");
    assert_eq!(
        error.as_deref(),
        Some("the change batch's inbox does not exist yet")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn workspaces_never_see_each_other(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = setup(&kit, "sam@example.com").await;
    kit.ingest(workspace, inbox, "en").await;

    let pool = campbooks_kernel::testing::app_role_pool(&db).await;
    for (as_workspace, expected) in [(workspace, 4_i64), (WorkspaceId::new(), 0)] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('campbooks.workspace_id', $1, true)")
            .bind(as_workspace.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        for (table, sql) in [
            ("messages", "SELECT count(*) FROM conversations.messages"),
            ("decisions", "SELECT count(*) FROM conversations.decisions"),
            (
                "attachments",
                "SELECT count(*) FROM conversations.attachments",
            ),
        ] {
            let seen: i64 = sqlx::query_scalar(sql).fetch_one(&mut *tx).await.unwrap();
            if table == "messages" {
                assert_eq!(seen, expected);
            } else if expected == 0 {
                assert_eq!(seen, 0, "{table}");
            }
        }
    }
}
