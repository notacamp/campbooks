//! The outbound-requests port against this module's own records: a send
//! stands only while its draft is being sent as that send by a member who
//! may send it, and a requested change only while its conversation shows
//! it.

use axum::http::StatusCode;
use campbooks_kernel::{
    blobs::BlobRef, db::BeginFor as _, outbound::StoredSend, tenancy::WorkspaceId,
};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use super::outbound_requests;
use crate::{model::InboxId, testkit::Kit};

/// Sam's reply to `en-1`, sent and out of its undo window: the
/// `send_requested` event it published.
async fn released(kit: &Kit, workspace: WorkspaceId, sam: &str) -> Value {
    let message = kit.message_of("en-1").await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let draft = kit
        .post(
            sam,
            &uri,
            Some(json!({"kind": "reply", "message_id": message, "body": "Done"})),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{:?}", draft.json);
    let uri = format!("{uri}/{}", draft.json["id"].as_str().unwrap());
    let version = kit.get(sam, &uri).await.json["version"].clone();
    let sent = kit
        .post(
            sam,
            &format!("{uri}/send"),
            Some(json!({"version": version})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    let requested = kit.events("conversations.send_requested").await;
    assert_eq!(requested.len(), 1);
    requested[0].clone()
}

/// What the port answers for send `send` of `workspace`.
async fn stored(kit: &Kit, workspace: WorkspaceId, send: &str) -> Option<StoredSend> {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let stored = outbound_requests()
        .stored_send(&kit.state, &mut tx, workspace, send.parse().unwrap())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    stored
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_released_send_stands_with_the_document_it_was_written_as(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let event = released(&kit, workspace, &sam).await;
    let send = event["send_id"].as_str().unwrap();
    let message: BlobRef = serde_json::from_value(event["message"].clone()).unwrap();
    assert_eq!(
        stored(&kit, workspace, send).await,
        Some(StoredSend {
            inbox: Uuid::from(inbox),
            message,
        })
    );
    // A send nobody released, or one of another workspace, does not.
    assert_eq!(
        stored(&kit, workspace, &Uuid::now_v7().to_string()).await,
        None
    );
    assert_eq!(stored(&kit, WorkspaceId::new(), send).await, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_stops_standing_once_its_author_left(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let event = released(&kit, workspace, &sam).await;
    let send = event["send_id"].as_str().unwrap().to_owned();
    // Its author left: no longer theirs to send.
    kit.leave("sam", workspace);
    assert_eq!(stored(&kit, workspace, &send).await, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_stops_standing_once_it_is_no_longer_being_sent(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let event = released(&kit, workspace, &sam).await;
    let send = event["send_id"].as_str().unwrap().to_owned();
    assert!(stored(&kit, workspace, &send).await.is_some());
    // It failed (or stopped waiting): nothing is waiting for the provider.
    sqlx::query("UPDATE conversations.drafts SET state = 'failed' WHERE send_id = $1::uuid")
        .bind(&send)
        .execute(&kit.db)
        .await
        .unwrap();
    assert_eq!(stored(&kit, workspace, &send).await, None);
    // A draft released by an older version, with no document recorded.
    sqlx::query(
        "UPDATE conversations.drafts SET state = 'sending', send_document = NULL
          WHERE send_id = $1::uuid",
    )
    .bind(&send)
    .execute(&kit.db)
    .await
    .unwrap();
    assert_eq!(stored(&kit, workspace, &send).await, None);
}

/// What the port answers for `items` of `conversation` in `inbox`.
async fn standing(
    kit: &Kit,
    workspace: WorkspaceId,
    inbox: InboxId,
    conversation: &str,
    items: &[Value],
) -> Vec<bool> {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let standing = outbound_requests()
        .changes_standing(
            &mut tx,
            workspace,
            inbox.into(),
            conversation.parse().unwrap(),
            items,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    standing
}

/// The items of the last `changes_requested` event.
async fn last_items(kit: &Kit) -> Vec<Value> {
    let requested = kit.events("conversations.changes_requested").await;
    requested.last().unwrap()["items"]
        .as_array()
        .unwrap()
        .clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_members_change_stands_until_a_later_one_undoes_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let conversation = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{conversation}");

    let read = kit.post(&sam, &format!("{base}/read"), None).await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json);
    let read_items = last_items(&kit).await;
    assert!(!read_items.is_empty());
    let all = vec![true; read_items.len()];
    assert_eq!(
        standing(&kit, workspace, inbox, &conversation, &read_items).await,
        all
    );
    // Asked in another inbox, or for another conversation: nothing stands.
    assert!(
        standing(&kit, workspace, InboxId::new(), &conversation, &read_items)
            .await
            .iter()
            .all(|s| !s)
    );
    let other = Uuid::now_v7().to_string();
    assert!(
        standing(&kit, workspace, inbox, &other, &read_items)
            .await
            .iter()
            .all(|s| !s)
    );

    // Unread again: that change stands, and "read" no longer does for it.
    kit.post(&sam, &format!("{base}/unread"), None).await;
    let unread_items = last_items(&kit).await;
    assert_eq!(
        standing(&kit, workspace, inbox, &conversation, &unread_items).await,
        [true]
    );
    let read_again = json!({
        "provider_message_id": unread_items[0]["provider_message_id"],
        "change": {"kind": "mark_read"},
    });
    assert_eq!(
        standing(&kit, workspace, inbox, &conversation, &[read_again]).await,
        [false]
    );

    // Star, archive, a move and a label, each as the conversation shows it.
    for action in ["star", "archive"] {
        kit.post(&sam, &format!("{base}/{action}"), None).await;
        let items = last_items(&kit).await;
        assert!(
            standing(&kit, workspace, inbox, &conversation, &items)
                .await
                .iter()
                .all(|s| *s),
            "{action}"
        );
    }
    let before = kit.events("conversations.changes_requested").await.len();
    let current: String =
        sqlx::query_scalar("SELECT stream FROM conversations.conversations WHERE id = $1::uuid")
            .bind(&conversation)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    let to = if current == "receipts" {
        "updates"
    } else {
        "receipts"
    };
    let moved = kit
        .post(&sam, &format!("{base}/stream"), Some(json!({"stream": to})))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(
        kit.events("conversations.changes_requested").await.len(),
        before + 1
    );
    let items = last_items(&kit).await;
    assert!(
        standing(&kit, workspace, inbox, &conversation, &items)
            .await
            .iter()
            .all(|s| *s)
    );
    let elsewhere: Vec<Value> = items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            item["change"]["stream"] = "newsletters".into();
            item
        })
        .collect();
    assert!(
        standing(&kit, workspace, inbox, &conversation, &elsewhere)
            .await
            .iter()
            .all(|s| !s)
    );
    let label = kit
        .post(
            &sam,
            &format!("{base}/labels"),
            Some(json!({"name": "Accountant"})),
        )
        .await;
    assert_eq!(label.status, StatusCode::OK, "{:?}", label.json);
    let items = last_items(&kit).await;
    assert!(
        standing(&kit, workspace, inbox, &conversation, &items)
            .await
            .iter()
            .all(|s| *s)
    );
    // The same label under a key the provider never gave it does not.
    let rekeyed: Vec<Value> = items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            item["change"]["provider_key"] = "Label_forged".into();
            item
        })
        .collect();
    assert!(
        standing(&kit, workspace, inbox, &conversation, &rekeyed)
            .await
            .iter()
            .all(|s| !s)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_change_stands_for_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let conversation = kit.conversation_of("en-1").await;
    // en-1 is unread and in the inbox: neither "read" nor "archive" shows.
    let items = [
        json!({"provider_message_id": "en-1", "change": {"kind": "mark_read"}}),
        json!({"provider_message_id": "en-1", "change": {"kind": "archive"}}),
        json!({"provider_message_id": "not-a-message", "change": {"kind": "mark_unread"}}),
        json!({"provider_message_id": "en-1", "change": {"kind": "send_everything"}}),
        json!("garbage"),
    ];
    assert_eq!(
        standing(&kit, workspace, inbox, &conversation, &items).await,
        [false, false, false, false, false]
    );
}
