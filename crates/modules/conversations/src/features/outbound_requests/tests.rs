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

/// What the port answers for `items` of request `request` on
/// `conversation` in `inbox`.
async fn standing(
    kit: &Kit,
    workspace: WorkspaceId,
    request: &str,
    inbox: InboxId,
    conversation: &str,
    items: &[Value],
) -> Vec<bool> {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let standing = outbound_requests()
        .changes_standing(
            &mut tx,
            workspace,
            request.parse().unwrap(),
            inbox.into(),
            conversation.parse().unwrap(),
            items,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    standing
}

/// Every answer is yes (and there is one).
fn every(answers: &[bool]) -> bool {
    !answers.is_empty() && answers.iter().all(|s| *s)
}

/// No answer is yes.
fn none(answers: &[bool]) -> bool {
    answers.iter().all(|s| !s)
}

/// The last `changes_requested` event: its id and items.
async fn last_request(kit: &Kit) -> (String, Vec<Value>) {
    let (id, payload): (String, Value) = sqlx::query_as(
        "SELECT id::text, payload FROM kernel.events
          WHERE name = 'conversations.changes_requested' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    (id, payload["items"].as_array().unwrap().clone())
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_members_change_stands_as_it_was_requested(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let conversation = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{conversation}");

    let read = kit.post(&sam, &format!("{base}/read"), None).await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json);
    let (read_request, read_items) = last_request(&kit).await;
    assert!(!read_items.is_empty());
    assert!(every(
        &standing(
            &kit,
            workspace,
            &read_request,
            inbox,
            &conversation,
            &read_items
        )
        .await
    ));
    // Under another request id, in another inbox, for another conversation
    // or another workspace: nothing stands.
    let other = Uuid::now_v7().to_string();
    assert!(none(
        &standing(&kit, workspace, &other, inbox, &conversation, &read_items).await
    ));
    assert!(none(
        &standing(
            &kit,
            workspace,
            &read_request,
            InboxId::new(),
            &conversation,
            &read_items
        )
        .await
    ));
    assert!(none(
        &standing(&kit, workspace, &read_request, inbox, &other, &read_items).await
    ));
    assert!(none(
        &standing(
            &kit,
            WorkspaceId::new(),
            &read_request,
            inbox,
            &conversation,
            &read_items
        )
        .await
    ));
    // A change the request did not hold, smuggled in with it.
    let smuggled = [
        json!({"provider_message_id": read_items[0]["provider_message_id"], "change": {"kind": "archive"}}),
        json!({"provider_message_id": "en-3", "change": {"kind": "mark_read"}}),
    ];
    assert_eq!(
        standing(
            &kit,
            workspace,
            &read_request,
            inbox,
            &conversation,
            &smuggled
        )
        .await,
        [false, false]
    );

    // Undone before the connector asks: both requests stand, as made (the
    // connector merges them, the later winning).
    kit.post(&sam, &format!("{base}/unread"), None).await;
    let (unread_request, unread_items) = last_request(&kit).await;
    assert_ne!(unread_request, read_request);
    assert!(every(
        &standing(
            &kit,
            workspace,
            &unread_request,
            inbox,
            &conversation,
            &unread_items
        )
        .await
    ));
    assert!(every(
        &standing(
            &kit,
            workspace,
            &read_request,
            inbox,
            &conversation,
            &read_items
        )
        .await
    ));

    // A label, under the key it was requested with only.
    let label = kit
        .post(
            &sam,
            &format!("{base}/labels"),
            Some(json!({"name": "Accountant"})),
        )
        .await;
    assert_eq!(label.status, StatusCode::OK, "{:?}", label.json);
    let (label_request, items) = last_request(&kit).await;
    assert!(every(
        &standing(
            &kit,
            workspace,
            &label_request,
            inbox,
            &conversation,
            &items
        )
        .await
    ));
    let rekeyed: Vec<Value> = items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            item["change"]["provider_key"] = "Label_forged".into();
            item
        })
        .collect();
    assert!(none(
        &standing(
            &kit,
            workspace,
            &label_request,
            inbox,
            &conversation,
            &rekeyed
        )
        .await
    ));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_change_stands_for_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let conversation = kit.conversation_of("en-1").await;
    // No request was ever made here: whatever it names, nothing stands,
    // even a change the conversation happens to show already.
    let items = [
        json!({"provider_message_id": "en-1", "change": {"kind": "archive"}}),
        json!({"provider_message_id": "en-2", "change": {"kind": "mark_read"}}),
        json!({"provider_message_id": "en-1", "change": {"kind": "send_everything"}}),
        json!("garbage"),
    ];
    let request = Uuid::now_v7().to_string();
    assert_eq!(
        standing(&kit, workspace, &request, inbox, &conversation, &items).await,
        [false, false, false, false]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_spam_and_flags_stand_as_requested(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let conversation = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{conversation}");
    // Each through its own operation, the flags through `request_each`:
    // every one is recorded and stands as the connector re-reads it.
    for (action, kind) in [
        ("unread", "mark_unread"),
        ("read", "mark_read"),
        ("trash", "trash"),
        ("untrash", "untrash"),
        ("spam", "spam"),
        ("not-spam", "unspam"),
    ] {
        let done = kit.post(&sam, &format!("{base}/{action}"), None).await;
        assert_eq!(done.status, StatusCode::OK, "{action}: {:?}", done.json);
        let (request, items) = last_request(&kit).await;
        assert!(
            items.iter().all(|item| item["change"]["kind"] == kind),
            "{action}: {items:?}"
        );
        assert!(
            every(&standing(&kit, workspace, &request, inbox, &conversation, &items).await),
            "{action}"
        );
        // Untrash and unspam carry whether the message was received: the
        // other value was not requested.
        if matches!(kind, "untrash" | "unspam") {
            let flipped: Vec<Value> = items
                .iter()
                .map(|item| {
                    let mut item = item.clone();
                    let inbound = item["change"]["inbound"].as_bool().unwrap();
                    item["change"]["inbound"] = (!inbound).into();
                    item
                })
                .collect();
            assert!(
                none(&standing(&kit, workspace, &request, inbox, &conversation, &flipped).await),
                "{action}"
            );
        }
    }
}
