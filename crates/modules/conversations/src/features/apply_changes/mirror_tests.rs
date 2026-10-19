//! Streams mirrored to the provider and moves made in the member's mail
//! client (`provider_stream`), with a scripted model as the decide pass.

use std::sync::Arc;

use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    model::InboxId,
    testkit::{Gated, Kit, Scripted, script, sorting_deps},
};

/// The model's answers: a list's mail to Newsletters, a service's alerts to
/// Updates, a shop's offers to Primary, its receipts to Receipts. Anyone
/// else is not answered.
fn model() -> Scripted {
    let sorted = |stream| script(stream, false, false, false, 0.0, false);
    Scripted::model([
        ("news@letters.example", sorted("newsletters")),
        ("alerts@service.example", sorted("updates")),
        ("promo@shop.example", sorted("primary")),
        ("receipts@shop.example", sorted("receipts")),
    ])
}

async fn kit(db: PgPool) -> Kit {
    Kit::with_deps(db, sorting_deps().with_model(Arc::new(model()))).await
}

async fn inbox(kit: &Kit) -> (WorkspaceId, InboxId) {
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    (workspace, inbox)
}

/// An upsert of received message `id` in thread `thread` from `from`, sent
/// `minutes` after a fixed time, at the provider in `stream` (left out for
/// `None`; `Some(Value::Null)` is none of the four).
fn upsert(id: &str, thread: &str, from: &str, minutes: i64, stream: Option<Value>) -> Value {
    let base = chrono::DateTime::parse_from_rfc3339("2026-10-01T08:00:00Z").unwrap();
    let mut message = json!({
        "provider_message_id": id,
        "thread_key": thread,
        "from": {"address": from},
        "to": [{"address": "sam@example.com"}],
        "subject": format!("Note {thread}"),
        "sent_at": (base + chrono::TimeDelta::minutes(minutes)).to_rfc3339(),
        "direction": "inbound",
        "text": "Hello.",
        "mailbox": {"in_inbox": true},
        "read": false, "starred": false
    });
    if let Some(stream) = stream {
        message["provider_stream"] = stream;
    }
    json!({"op": "upsert", "message": message})
}

fn batch(changes: &[Value]) -> Value {
    json!({ "changes": changes })
}

/// Every `move_to_stream` asked so far, as (message, stream).
async fn moves(kit: &Kit) -> Vec<(String, String)> {
    kit.events("conversations.changes_requested")
        .await
        .iter()
        .flat_map(|event| event["items"].as_array().cloned().unwrap_or_default())
        .filter(|item| item["change"]["kind"] == "move_to_stream")
        .map(|item| {
            (
                item["provider_message_id"].as_str().unwrap().to_owned(),
                item["change"]["stream"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The stored provider stream of message `id`, and whether it is mirrored.
async fn stored(kit: &Kit, id: &str) -> (Option<String>, bool) {
    sqlx::query_as(
        "SELECT provider_stream, mirrors_stream FROM conversations.messages
          WHERE provider_message_id = $1",
    )
    .bind(id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

fn pair(id: &str, stream: &str) -> (String, String) {
    (id.to_owned(), stream.to_owned())
}

#[sqlx::test(migrations = "../../../migrations")]
async fn scouts_sort_reaches_the_provider(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    // In Gmail's Promotions tab; Scout places it in Primary.
    let offer = upsert(
        "p1",
        "t-p",
        "promo@shop.example",
        0,
        Some(json!("newsletters")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[offer])).await;
    assert_eq!(
        kit.stream_of("p1").await,
        ("primary".to_owned(), "model".to_owned())
    );
    assert_eq!(moves(&kit).await, [pair("p1", "primary")]);
    assert_eq!(stored(&kit, "p1").await, (Some("newsletters".into()), true));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_already_where_scout_put_it_is_left_alone(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let alert = upsert(
        "a1",
        "t-a",
        "alerts@service.example",
        0,
        Some(json!("updates")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[alert])).await;
    assert_eq!(
        kit.stream_of("a1").await,
        ("updates".to_owned(), "model".to_owned())
    );
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_joining_a_sorted_conversation_follows_it(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let first = upsert(
        "n1",
        "t-n",
        "news@letters.example",
        0,
        Some(json!("newsletters")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[first])).await;
    assert_eq!(
        kit.stream_of("n1").await,
        ("newsletters".to_owned(), "model".to_owned())
    );
    assert!(moves(&kit).await.is_empty());
    // The next issue lands in the Updates tab.
    let next = upsert(
        "n2",
        "t-n",
        "news@letters.example",
        60,
        Some(json!("updates")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[next])).await;
    assert_eq!(moves(&kit).await, [pair("n2", "newsletters")]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_backfill_is_left_alone(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let offer = upsert(
        "p1",
        "t-p",
        "promo@shop.example",
        0,
        Some(json!("newsletters")),
    );
    let mut first_sync = batch(&[offer]);
    first_sync["backfill"] = true.into();
    kit.ingest_value(workspace, inbox, &first_sync).await;
    // Scout sorts it all the same; the provider is not asked.
    assert_eq!(
        kit.stream_of("p1").await,
        ("primary".to_owned(), "model".to_owned())
    );
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
    assert_eq!(
        stored(&kit, "p1").await,
        (Some("newsletters".into()), false)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_in_the_mail_client_moves_the_conversation(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let issues = |stream: &str| {
        batch(&[
            upsert("n1", "t-n", "news@letters.example", 0, Some(json!(stream))),
            upsert(
                "n2",
                "t-n",
                "news@letters.example",
                5,
                Some(json!("newsletters")),
            ),
        ])
    };
    kit.ingest_value(workspace, inbox, &issues("newsletters"))
        .await;
    assert_eq!(kit.stream_of("n1").await.0, "newsletters");
    assert!(moves(&kit).await.is_empty());
    let changed = kit.events("conversations.conversation_changed").await.len();

    // The member drags the first one to Primary in Gmail.
    kit.ingest_value(workspace, inbox, &issues("primary")).await;
    assert_eq!(
        kit.stream_of("n1").await,
        ("primary".to_owned(), "member".to_owned())
    );
    let events = kit.events("conversations.conversation_changed").await;
    assert_eq!(events.len(), changed + 1, "{events:#?}");
    let last = events.last().unwrap();
    assert_eq!(last["origin"], "provider");
    assert_eq!(last["changes"], json!(["stream"]));
    // The other one is asked to follow; the moved one is where it is.
    assert_eq!(moves(&kit).await, [pair("n2", "primary")]);
    // No correction is recorded for Scout.
    let corrections: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.corrections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(corrections, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn our_own_write_coming_back_changes_only_the_stored_stream(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let offer = |stream: &str| {
        batch(&[upsert(
            "p1",
            "t-p",
            "promo@shop.example",
            0,
            Some(json!(stream)),
        )])
    };
    kit.ingest_value(workspace, inbox, &offer("newsletters"))
        .await;
    assert_eq!(moves(&kit).await, [pair("p1", "primary")]);
    let changed = kit.events("conversations.conversation_changed").await.len();
    let requested = kit.events("conversations.changes_requested").await.len();

    // The provider made the move: the next batch says Primary.
    kit.ingest_value(workspace, inbox, &offer("primary")).await;
    assert_eq!(
        kit.stream_of("p1").await,
        ("primary".to_owned(), "model".to_owned())
    );
    assert_eq!(stored(&kit, "p1").await.0.as_deref(), Some("primary"));
    assert_eq!(
        kit.events("conversations.conversation_changed").await.len(),
        changed
    );
    assert_eq!(
        kit.events("conversations.changes_requested").await.len(),
        requested
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_providers_sort_on_arrival_places_nothing(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    // Gmail filed it in Updates; nobody answers for this sender.
    let note = upsert(
        "x1",
        "t-x",
        "someone@else.example",
        0,
        Some(json!("updates")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[note])).await;
    assert_eq!(stored(&kit, "x1").await, (Some("updates".into()), true));
    assert_eq!(
        kit.stream_of("x1").await,
        ("primary".to_owned(), "default".to_owned())
    );
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
    // In a tab of no stream (Social): stored as none.
    let social = upsert("x2", "t-y", "someone@else.example", 5, Some(Value::Null));
    kit.ingest_value(workspace, inbox, &batch(&[social])).await;
    assert_eq!(stored(&kit, "x2").await, (Some("none".into()), true));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_in_the_mail_client_sticks(db: PgPool) {
    let model = Gated::closed(model());
    let kit = Kit::with_deps(db.clone(), sorting_deps().with_model(model.clone())).await;
    let (workspace, inbox) = inbox(&kit).await;
    let issue = |stream: &str| {
        batch(&[upsert(
            "n1",
            "t-n",
            "news@letters.example",
            0,
            Some(json!(stream)),
        )])
    };
    kit.ingest_value(workspace, inbox, &issue("newsletters"))
        .await;
    // Moved to Gmail's Primary tab before the model answers.
    kit.ingest_value(workspace, inbox, &issue("primary")).await;
    model.open();
    kit.state
        .events
        .replay(
            &kit.state.db,
            "conversations.decide_message",
            campbooks_kernel::events::ReplayFrom::Beginning,
        )
        .await
        .unwrap();
    kit.run_jobs().await;
    assert_eq!(
        kit.stream_of("n1").await,
        ("primary".to_owned(), "member".to_owned())
    );
    let acted: bool = sqlx::query_scalar(
        "SELECT d.acted FROM conversations.decisions d
           JOIN conversations.messages m ON m.id = d.message_id
          WHERE m.provider_message_id = 'n1' AND d.question = 'stream' AND d.source = 'model'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!acted);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_upsert_without_a_provider_stream_changes_nothing(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let issue = upsert(
        "n1",
        "t-n",
        "news@letters.example",
        0,
        Some(json!("newsletters")),
    );
    kit.ingest_value(workspace, inbox, &batch(&[issue])).await;
    let changed = kit.events("conversations.conversation_changed").await.len();
    // Read at the provider, by a connector that says no stream.
    let mut read = upsert("n1", "t-n", "news@letters.example", 0, None);
    read["message"]["read"] = true.into();
    kit.ingest_value(workspace, inbox, &batch(&[read])).await;
    assert_eq!(stored(&kit, "n1").await.0.as_deref(), Some("newsletters"));
    assert_eq!(
        kit.stream_of("n1").await,
        ("newsletters".to_owned(), "model".to_owned())
    );
    let events = kit.events("conversations.conversation_changed").await;
    assert_eq!(events.len(), changed + 1);
    assert_eq!(events.last().unwrap()["changes"], json!(["read"]));
    assert!(moves(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_first_provider_stream_moves_nothing(db: PgPool) {
    // Mail stored before provider streams were carried: the first upsert
    // that says one (a label changed at Gmail) is no move.
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let issue =
        |stream: Option<Value>| batch(&[upsert("n1", "t-n", "news@letters.example", 0, stream)]);
    kit.ingest_value(workspace, inbox, &issue(None)).await;
    assert_eq!(kit.stream_of("n1").await.0, "newsletters");
    assert_eq!(stored(&kit, "n1").await, (None, false));
    kit.ingest_value(workspace, inbox, &issue(Some(json!("updates"))))
        .await;
    assert_eq!(
        kit.stream_of("n1").await,
        ("newsletters".to_owned(), "model".to_owned())
    );
    assert_eq!(stored(&kit, "n1").await.0.as_deref(), Some("updates"));
    assert!(moves(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_stream_mark_is_taken_off(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox) = inbox(&kit).await;
    let receipt = |stream: &str, stale: bool| {
        let mut change = upsert("r1", "t-r", "receipts@shop.example", 0, Some(json!(stream)));
        if stale {
            change["message"]["stream_mark_stale"] = true.into();
        }
        batch(&[change])
    };
    kit.ingest_value(workspace, inbox, &receipt("receipts", false))
        .await;
    assert_eq!(kit.stream_of("r1").await.0, "receipts");
    assert!(moves(&kit).await.is_empty());
    // Dragged to Promotions in Gmail, its Receipts mark still on.
    kit.ingest_value(workspace, inbox, &receipt("newsletters", true))
        .await;
    assert_eq!(
        kit.stream_of("r1").await,
        ("newsletters".to_owned(), "member".to_owned())
    );
    // The message itself is asked to Newsletters: the stale mark comes off.
    assert_eq!(moves(&kit).await, [pair("r1", "newsletters")]);
}
