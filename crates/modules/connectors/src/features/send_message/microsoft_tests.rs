//! Sending from Outlook mailboxes through Graph's `sendMail`
//! (campbooks-cloud#317), end to end against the fake Graph: once, as MIME
//! with its threading headers, no copy written to Sent Items, the copy Graph
//! filed known to the next sync, a copy filed late brought once, Graph's
//! size limit, From the verified mailbox only, a lost answer found and not
//! sent again, and throttling.

use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    contract::{Address, OUTGOING_CONTENT_TYPE, OutgoingAttachment, OutgoingMessage},
    domain::model::{InboxId, SendId},
    events::SendRequested,
    testkit::{Kit, assert_contract},
    testkit_microsoft::{ADDRESS, SUB},
};

/// Ana's Outlook mailbox, connected (its address verified as `verified`)
/// and read once.
struct Mailbox {
    kit: Kit,
    ws: WorkspaceId,
    inbox: InboxId,
}

async fn mailbox(db: PgPool, verified: bool) -> Mailbox {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.adopt_microsoft(
        &ana,
        ws,
        SUB,
        verified,
        &[
            "https://graph.microsoft.com/Mail.ReadWrite",
            "https://graph.microsoft.com/Mail.Send",
            "https://graph.microsoft.com/User.Read",
            "offline_access",
        ],
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("adopting: {e:?}"));
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental", "the backfill ended");
    let inbox: InboxId = serde_json::from_value(stream["inbox_id"].clone()).unwrap();
    Mailbox { kit, ws, inbox }
}

fn person(name: Option<&str>, address: &str) -> Address {
    Address {
        name: name.map(ToOwned::to_owned),
        address: address.to_owned(),
    }
}

impl Mailbox {
    /// A reply from Ana to Bea, changed by `edit`, with `attachments` (their
    /// bytes), asked for as the conversations module would: the send's id.
    async fn send_with(
        &self,
        edit: impl FnOnce(&mut OutgoingMessage),
        attachments: Vec<Vec<u8>>,
    ) -> SendId {
        let send_id = SendId::new();
        let ws = self.ws.to_string();
        let mut stored = Vec::new();
        for bytes in attachments {
            let blob = self
                .kit
                .state
                .blobs
                .put(&ws, bytes, "application/pdf")
                .await
                .unwrap();
            stored.push(OutgoingAttachment {
                filename: "plan.pdf".into(),
                content_type: "application/pdf".into(),
                blob,
            });
        }
        let mut document = OutgoingMessage {
            version: 1,
            inbox: self.inbox,
            rfc_message_id: format!("{send_id}@example.com"),
            from: person(Some("Ana"), ADDRESS),
            to: vec![person(Some("Bea"), "bea@example.net")],
            cc: Vec::new(),
            bcc: vec![person(None, "joana@example.org")],
            subject: "Re: Order".into(),
            date: self.kit.clock.now(),
            in_reply_to: Some("order-1@example.net".into()),
            references: vec!["order-1@example.net".into()],
            thread_key: None,
            text: "Friday it is.".into(),
            html: Some("<p>Friday it is.</p>".into()),
            attachments: stored,
        };
        edit(&mut document);
        let mut value = serde_json::to_value(&document).unwrap();
        value["send_id"] = send_id.to_string().into();
        assert_contract("outgoing-message.v1.schema.json", &value);
        let blob = self
            .kit
            .state
            .blobs
            .put(
                &ws,
                serde_json::to_vec(&value).unwrap(),
                OUTGOING_CONTENT_TYPE,
            )
            .await
            .unwrap();
        self.kit.outbound.release(send_id, self.inbox, blob.clone());
        let mut tx = self.kit.state.db.begin_for(self.ws).await.unwrap();
        self.kit
            .state
            .events
            .publish(
                &mut tx,
                &SendRequested {
                    workspace: ws,
                    inbox_id: self.inbox,
                    send_id,
                    message: blob,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        send_id
    }

    async fn send(&self, edit: impl FnOnce(&mut OutgoingMessage)) -> SendId {
        self.send_with(edit, Vec::new()).await
    }

    /// Run the jobs, then (minutes later) the sends waiting to retry, until
    /// none is left.
    async fn settle(&self) {
        for _ in 0..12 {
            self.kit.run_jobs().await;
            self.kit.clock.advance(TimeDelta::minutes(2));
            let waiting: Vec<uuid::Uuid> = sqlx::query_scalar(
                "UPDATE kernel.jobs SET run_at = now()
                  WHERE state = 'pending' AND kind = 'connectors.send_message' RETURNING id",
            )
            .fetch_all(&self.kit.db)
            .await
            .unwrap();
            if waiting.is_empty() {
                return;
            }
        }
    }

    /// The `connectors.send_completed` events, checked against the contract.
    async fn completed(&self) -> Vec<Value> {
        let events = self
            .kit
            .payloads(self.ws, "connectors.send_completed")
            .await;
        for event in &events {
            assert_contract("connectors.send_completed.v1.schema.json", event);
        }
        events
    }

    /// The one send's outcome and reason.
    async fn outcome(&self) -> (String, Option<String>) {
        let done = self.completed().await;
        assert_eq!(done.len(), 1, "{done:?}");
        (
            done[0]["outcome"].as_str().unwrap().to_owned(),
            done[0]["reason"].as_str().map(ToOwned::to_owned),
        )
    }

    /// The provider id recorded on send `send`.
    async fn key(&self, send: SendId) -> Option<String> {
        sqlx::query_scalar("SELECT provider_message_id FROM connectors.sends WHERE id = $1")
            .bind(send)
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// How many upserts of `id` were announced.
    async fn upserts(&self, id: &str) -> usize {
        self.kit
            .batches(self.ws)
            .await
            .iter()
            .flat_map(|b| b["changes"].as_array().cloned().unwrap_or_default())
            .filter(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
            .count()
    }

    /// The stream's next run, past its longest interval.
    async fn run(&self) {
        self.kit.tick(TimeDelta::hours(5)).await;
    }

    /// Only `sendMail` ever changed anything at Graph; nothing was deleted.
    fn only_sent(&self) {
        assert_eq!(self.kit.microsoft.deletes(), 0);
        for call in self.kit.microsoft.writes() {
            assert_eq!(call, "POST /v1.0/me/sendMail");
        }
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_goes_out_once_with_its_thread_and_no_copy_written(db: PgPool) {
    let m = mailbox(db, true).await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let sent = m.kit.microsoft.sent_mime();
    assert_eq!(sent.len(), 1);
    // The message Gmail is sent, threading headers and all.
    let parsed = crate::mail::parse::parse(&sent[0], crate::mail::Receiver::Microsoft);
    let rfc_id = format!("{send}@example.com");
    assert_eq!(parsed.rfc_message_id.as_deref(), Some(rfc_id.as_str()));
    assert_eq!(parsed.in_reply_to.as_deref(), Some("order-1@example.net"));
    assert_eq!(parsed.references, ["order-1@example.net"]);
    assert_eq!(parsed.from.address, ADDRESS, "From the mailbox");
    let mime = String::from_utf8_lossy(&sent[0]).to_ascii_lowercase();
    assert!(mime.contains("bcc:") && mime.contains("joana@example.org"));
    assert!(!mime.contains("\nsender:"), "no Sender");
    // The copy Graph filed is the one indexed.
    let copies = m.kit.microsoft.sent_items();
    assert_eq!(copies.len(), 1);
    let id = copies[0].id.clone();
    assert_eq!(m.key(send).await.as_deref(), Some(id.as_str()));
    assert_eq!(m.upserts(&id).await, 1);
    // The next sync knows it: not announced again.
    m.run().await;
    m.run().await;
    assert_eq!(m.upserts(&id).await, 1);
    assert_eq!(m.kit.microsoft.raw_fetches(&id), 0);
    m.only_sent();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_copy_filed_late_is_found_and_never_sent_again(db: PgPool) {
    let m = mailbox(db, true).await;
    m.kit.microsoft.delay_filing(true);
    let send = m.send(|_| {}).await;
    m.kit.run_jobs().await;
    // Taken, its copy not in Sent Items yet: not settled, not sent again.
    assert!(m.completed().await.is_empty());
    assert_eq!(m.kit.microsoft.send_calls(), 1);
    m.kit.microsoft.file_sent();
    m.settle().await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    assert_eq!(m.kit.microsoft.send_calls(), 1);
    let id = m.kit.microsoft.sent_items()[0].id.clone();
    assert_eq!(m.key(send).await.as_deref(), Some(id.as_str()));
    m.run().await;
    assert_eq!(m.upserts(&id).await, 1);
    m.only_sent();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_over_graph_s_limit_is_never_sent(db: PgPool) {
    let m = mailbox(db, true).await;
    m.send_with(|_| {}, vec![vec![7_u8; 3_200_000]]).await;
    m.settle().await;
    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("too_large".to_owned()))
    );
    assert_eq!(m.kit.microsoft.send_calls(), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unverified_address_is_never_sent_from(db: PgPool) {
    let m = mailbox(db, false).await;
    m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(m.kit.microsoft.send_calls(), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_from_is_never_sent(db: PgPool) {
    let m = mailbox(db, true).await;
    m.send(|d| d.from = person(Some("Boss"), "ceo@example.com"))
        .await;
    m.settle().await;
    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(m.kit.microsoft.send_calls(), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_message_is_never_sent_again(db: PgPool) {
    let m = mailbox(db, true).await;
    m.kit.microsoft.refuse_next_send(400);
    m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(m.kit.microsoft.send_calls(), 1);
    assert!(m.kit.microsoft.sent_items().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lost_answer_is_found_and_never_sent_again(db: PgPool) {
    let m = mailbox(db, true).await;
    m.kit.microsoft.lose_next_send();
    let send = m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    assert_eq!(m.kit.microsoft.send_calls(), 1);
    let id = m.kit.microsoft.sent_items()[0].id.clone();
    assert_eq!(m.key(send).await.as_deref(), Some(id.as_str()));
    m.only_sent();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unclear_send_not_found_is_never_sent_again(db: PgPool) {
    let m = mailbox(db, true).await;
    // Taken, its answer lost, and not filed: nothing proves it went.
    m.kit.microsoft.delay_filing(true);
    m.kit.microsoft.lose_next_send();
    m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("unknown_outcome".to_owned()))
    );
    assert_eq!(m.kit.microsoft.send_calls(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_throttled_send_waits_and_goes_once(db: PgPool) {
    let m = mailbox(db, true).await;
    m.kit.microsoft.throttle_next(120);
    m.send(|_| {}).await;
    m.kit.run_jobs().await;
    assert!(m.completed().await.is_empty(), "waiting");
    assert!(m.kit.microsoft.sent_mime().is_empty());
    m.settle().await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    assert_eq!(m.kit.microsoft.sent_mime().len(), 1);
}
