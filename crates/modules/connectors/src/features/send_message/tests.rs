//! Sending through the fake Gmail: threading, the message coming back
//! once, and never sending twice (redelivery, a lost answer, a 5xx, a
//! refusal).

use campbooks_kernel::{
    clock::Clock as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use sqlx::PgPool;

use crate::{
    contract::{Address, OUTGOING_CONTENT_TYPE, OutgoingMessage},
    domain::model::{InboxId, SendId},
    events::SendRequested,
    gmail::normalize,
    testkit::{Kit, assert_contract, synthetic},
};

/// Ana connected, with one message from Sender 1 synced: the workspace,
/// the inbox and that message's Gmail thread.
async fn connected(kit: &Kit) -> (WorkspaceId, InboxId, String) {
    let at = kit.clock.now() - TimeDelta::hours(1);
    let id = kit.google.seed(&synthetic(1), &["INBOX", "UNREAD"], at);
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect(&ana, ws).await;
    kit.run_jobs().await;
    let inbox: InboxId = serde_json::from_value(kit.stream().await["inbox_id"].clone()).unwrap();
    (ws, inbox, format!("thread-{id}"))
}

/// A reply to Sender 1's message, as the conversations module would ask
/// for it: the send's id.
async fn request(kit: &Kit, ws: WorkspaceId, inbox: InboxId, thread: Option<&str>) -> SendId {
    let send_id = SendId::new();
    let document = OutgoingMessage {
        version: 1,
        inbox,
        rfc_message_id: format!("{send_id}@example.com"),
        from: Address {
            name: Some("Ana".into()),
            address: "sam@example.com".into(),
        },
        to: vec![Address {
            name: Some("Sender 1".into()),
            address: "sender1@example.com".into(),
        }],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "Re: Message 1".into(),
        date: kit.clock.now(),
        in_reply_to: Some("m1@example.com".into()),
        references: vec!["m1@example.com".into()],
        thread_key: thread.map(ToOwned::to_owned),
        text: "Thanks, see you Thursday.".into(),
        html: Some("<div><p>Thanks, see you Thursday.</p></div>".into()),
        attachments: Vec::new(),
    };
    let mut value = serde_json::to_value(&document).unwrap();
    value["send_id"] = send_id.to_string().into();
    assert_contract("outgoing-message.v1.schema.json", &value);
    let blob = kit
        .state
        .blobs
        .put(
            &ws.to_string(),
            serde_json::to_vec(&value).unwrap(),
            OUTGOING_CONTENT_TYPE,
        )
        .await
        .unwrap();
    publish(kit, ws, inbox, send_id, blob).await;
    send_id
}

async fn publish(
    kit: &Kit,
    ws: WorkspaceId,
    inbox: InboxId,
    send_id: SendId,
    message: campbooks_kernel::blobs::BlobRef,
) {
    let mut tx = kit.state.db.begin().await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &SendRequested {
                workspace: ws.to_string(),
                inbox_id: inbox,
                send_id,
                message,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// Run the jobs, then the ones waiting to retry, until none is left.
async fn settle(kit: &Kit) {
    for _ in 0..10 {
        kit.run_jobs().await;
        let waiting = sqlx::query_scalar!(
            r#"UPDATE kernel.jobs SET run_at = now()
                WHERE state = 'pending' AND kind = 'connectors.send_message' RETURNING id"#
        )
        .fetch_all(&kit.db)
        .await
        .unwrap();
        if waiting.is_empty() {
            return;
        }
    }
}

/// The `connectors.send_completed` events, checked against the contract.
async fn completed(kit: &Kit, ws: WorkspaceId) -> Vec<serde_json::Value> {
    let events = kit.payloads(ws, "connectors.send_completed").await;
    for event in &events {
        assert_contract("connectors.send_completed.v1.schema.json", event);
    }
    events
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_goes_into_its_thread_and_comes_back_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    let send_id = request(&kit, ws, inbox, Some(&thread)).await;
    settle(&kit).await;

    assert_eq!(kit.google.send_calls(), 1);
    let sent = kit.google.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].thread_id, thread);
    let parsed = normalize::parse(&sent[0].raw);
    assert_eq!(
        parsed.rfc_message_id.as_deref(),
        Some(format!("{send_id}@example.com").as_str())
    );
    assert_eq!(parsed.in_reply_to.as_deref(), Some("m1@example.com"));
    assert_eq!(parsed.references, ["m1@example.com"]);
    assert_eq!(parsed.to[0].address, "sender1@example.com");
    assert_eq!(parsed.subject, "Re: Message 1");

    let done = completed(&kit, ws).await;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0]["outcome"], "sent");
    assert_eq!(done[0]["provider_message_id"], sent[0].id);
    assert_eq!(done[0]["send_id"], send_id.to_string());
    // No address, subject or body in the event log.
    for event in kit.payloads(ws, "connectors.send_completed").await {
        let text = event.to_string();
        assert!(
            !text.contains("sender1") && !text.contains("Thursday"),
            "{text}"
        );
    }

    // The sent message is announced at once, like a synced one...
    let upserts_of_sent = |batches: &[serde_json::Value]| {
        batches
            .iter()
            .flat_map(|b| b["changes"].as_array().unwrap().clone())
            .filter(|c| c["message"]["provider_message_id"] == sent[0].id.as_str())
            .count()
    };
    let batches = kit.batches(ws).await;
    for batch in &batches {
        assert_contract("message-change-batch.v1.schema.json", batch);
    }
    assert_eq!(upserts_of_sent(&batches), 1);
    let message = batches
        .iter()
        .flat_map(|b| b["changes"].as_array().unwrap().clone())
        .find(|c| c["message"]["provider_message_id"] == sent[0].id.as_str())
        .unwrap();
    assert_eq!(message["message"]["direction"], "outbound");
    assert_eq!(message["message"]["thread_key"], thread);
    // ...and the next sync, seeing it in Gmail's history, knows it already.
    kit.next_run().await;
    assert_eq!(upserts_of_sent(&kit.batches(ws).await), 1);
    assert_eq!(kit.google.raw_fetches(&sent[0].id), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_same_request_twice_sends_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    let send_id = request(&kit, ws, inbox, Some(&thread)).await;
    let blob = serde_json::from_value(
        kit.payloads(ws, "conversations.send_requested").await[0]["message"].clone(),
    )
    .unwrap();
    publish(&kit, ws, inbox, send_id, blob).await;
    settle(&kit).await;
    assert_eq!(kit.google.send_calls(), 1);
    assert_eq!(completed(&kit, ws).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lost_answer_is_looked_up_not_sent_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    kit.google.lose_next_send_answer();
    request(&kit, ws, inbox, Some(&thread)).await;

    kit.run_jobs().await;
    // Gmail took it, but the answer was lost: the job waits, then asks.
    assert_eq!(kit.google.send_calls(), 1);
    assert!(completed(&kit, ws).await.is_empty());
    settle(&kit).await;

    assert_eq!(kit.google.send_calls(), 1, "{:?}", kit.google.calls());
    assert_eq!(kit.google.sent().len(), 1);
    let done = completed(&kit, ws).await;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0]["outcome"], "sent");
    assert_eq!(done[0]["provider_message_id"], kit.google.sent()[0].id);
    let attempts: i32 = sqlx::query_scalar!("SELECT attempts FROM connectors.sends")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(attempts, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unclear_failure_that_sent_nothing_is_sent_after_the_lookup(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    kit.google.refuse_next_send(503);
    request(&kit, ws, inbox, Some(&thread)).await;
    settle(&kit).await;
    assert_eq!(kit.google.send_calls(), 2);
    assert_eq!(kit.google.sent().len(), 1);
    assert_eq!(completed(&kit, ws).await[0]["outcome"], "sent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_message_fails_at_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    kit.google.refuse_next_send(400);
    request(&kit, ws, inbox, Some(&thread)).await;
    settle(&kit).await;
    assert_eq!(kit.google.send_calls(), 1);
    assert!(kit.google.sent().is_empty());
    let done = completed(&kit, ws).await;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0]["outcome"], "failed");
    assert_eq!(done[0]["reason"], "rejected");
    let held: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM kernel.blob_refs WHERE owner_type = 'connectors.send'"#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_inbox_without_a_connection_fails_without_calling_gmail(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, _, _) = connected(&kit).await;
    request(&kit, ws, InboxId::new(), None).await;
    settle(&kit).await;
    assert_eq!(kit.google.send_calls(), 0);
    let done = completed(&kit, ws).await;
    assert_eq!(done[0]["reason"], "not_connected");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_message_starts_a_thread(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, inbox, thread) = connected(&kit).await;
    request(&kit, ws, inbox, None).await;
    settle(&kit).await;
    let sent = kit.google.sent();
    assert_eq!(sent.len(), 1);
    assert_ne!(sent[0].thread_id, thread);
}
