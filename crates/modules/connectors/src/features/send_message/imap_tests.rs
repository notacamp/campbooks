//! Sending from IMAP mailboxes over SMTP (campbooks-cloud#313), end to end
//! against the fake servers: 465 and 587, never a password or a message in
//! the clear, From the mailbox only, Bcc in the envelope only, never sent
//! twice, the limits, a server that is busy, refuses or never stops
//! talking, and the copy in Sent appended (or not) without echo.

use axum::http::StatusCode;
use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    contract::{Address, OUTGOING_CONTENT_TYPE, OutgoingMessage},
    domain::model::{InboxId, SendId},
    events::SendRequested,
    imap::{Imap, Security, Servers},
    mail::{Receiver, parse as normalize},
    testkit::{Caller, Kit, assert_contract},
    testkit_imap::{self, FakeServer, Fakes, IMAP_HOST, SMTP_HOST, Script},
};

/// Ana's IMAP mailbox at the fakes, connected and read once.
struct Mailbox {
    kit: Kit,
    imap: FakeServer,
    smtp: FakeServer,
    ws: WorkspaceId,
    inbox: InboxId,
    _ana: Caller,
    _fakes: Fakes,
}

/// Ana's mailbox: an IMAP server (TLS, a `\Sent` folder, `imap` its
/// script) and an SMTP server on `security`, Campbooks' adapter changed by
/// `adapter`, connected and read once; then the SMTP server's script is
/// changed by `smtp` (after the connect check, which must pass).
async fn mailbox(
    db: PgPool,
    adapter: impl FnOnce(&mut Imap),
    security: Security,
    imap: impl FnOnce(&mut Script),
    smtp: impl FnOnce(&mut Script),
) -> Mailbox {
    let (kit, fakes) = testkit_imap::kit_with_adapter(db, adapter).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let imap_server =
        FakeServer::imap(Security::Tls, Script::accepting("ana@bakery.example", "pw")).await;
    let smtp_server =
        FakeServer::smtp(security, Script::accepting("ana@bakery.example", "pw")).await;
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    imap_server.mailbox(|store| store.add_folder("Sent", &["\\Sent"]));
    imap_server.script(imap);
    let servers = Servers {
        imap: imap_server.at(IMAP_HOST),
        smtp: smtp_server.at(SMTP_HOST),
    };
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            json!({"address": "ana@bakery.example", "password": "pw", "servers": servers}),
        )
        .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    smtp_server.script(smtp);
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental", "the backfill ended");
    let inbox: InboxId = serde_json::from_value(stream["inbox_id"].clone()).unwrap();
    Mailbox {
        kit,
        imap: imap_server,
        smtp: smtp_server,
        ws,
        inbox,
        _ana: ana,
        _fakes: fakes,
    }
}

/// The usual mailbox: SMTP on STARTTLS, an IMAP server with UIDPLUS.
async fn usual(db: PgPool, smtp: impl FnOnce(&mut Script)) -> Mailbox {
    mailbox(
        db,
        |_| {},
        Security::Starttls,
        |imap| imap.offers.uidplus = true,
        smtp,
    )
    .await
}

fn person(name: Option<&str>, address: &str) -> Address {
    Address {
        name: name.map(ToOwned::to_owned),
        address: address.to_owned(),
    }
}

impl Mailbox {
    /// A reply from Ana to Bea, changed by `edit`, asked for as the
    /// conversations module would (stored through the port, then
    /// announced): the send's id.
    async fn send(&self, edit: impl FnOnce(&mut OutgoingMessage)) -> SendId {
        let send_id = SendId::new();
        let mut document = OutgoingMessage {
            version: 1,
            inbox: self.inbox,
            rfc_message_id: format!("{send_id}@bakery.example"),
            from: person(Some("Ana"), "ana@bakery.example"),
            to: vec![person(Some("Bea"), "bea@example.com")],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Re: Order".into(),
            date: self.kit.clock.now(),
            in_reply_to: Some("order-1@example.com".into()),
            references: vec!["order-1@example.com".into()],
            thread_key: None,
            text: "Friday it is.".into(),
            html: Some("<p>Friday it is.</p>".into()),
            attachments: Vec::new(),
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
                &self.ws.to_string(),
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
                    workspace: self.ws.to_string(),
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

    /// The stream's next run (the send made it due).
    async fn run(&self) {
        self.kit.tick(TimeDelta::minutes(1)).await;
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

    /// What became of send `send`'s copy in Sent, and why.
    async fn copy(&self, send: SendId) -> (Option<String>, Option<String>) {
        sqlx::query_as("SELECT sent_copy, sent_copy_reason FROM connectors.sends WHERE id = $1")
            .bind(send)
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The index key of send `send`'s message.
    async fn key(&self, send: SendId) -> String {
        sqlx::query_scalar("SELECT provider_message_id FROM connectors.sends WHERE id = $1")
            .bind(send)
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The locations of the message with key `key`: (folder, UID).
    async fn locations(&self, key: &str) -> Vec<(String, i64)> {
        sqlx::query_as(
            "SELECT folder, uid FROM connectors.imap_locations
              WHERE message_key = $1 ORDER BY folder, uid",
        )
        .bind(key)
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }

    /// Every change announced so far, each batch checked against the
    /// contract.
    async fn changes(&self) -> Vec<Value> {
        let batches = self.kit.batches(self.ws).await;
        for batch in &batches {
            assert_contract("message-change-batch.v1.schema.json", batch);
        }
        batches
            .iter()
            .flat_map(|b| b["changes"].as_array().cloned().unwrap_or_default())
            .collect()
    }

    async fn health(&self) -> String {
        sqlx::query_scalar("SELECT health FROM connectors.connections")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The message the SMTP fake took, as text.
    fn delivered(&self) -> String {
        let seen = self.smtp.seen();
        assert_eq!(seen.delivered.len(), 1, "one message delivered");
        String::from_utf8(seen.delivered[0].clone()).unwrap()
    }
}

/// The header block of `message`, each line unfolded.
fn header_lines(message: &str) -> Vec<String> {
    let head = message.split("\r\n\r\n").next().unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    for line in head.split("\r\n") {
        if line.starts_with([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(line);
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn has_header(message: &str, name: &str) -> bool {
    header_lines(message).iter().any(|line| {
        line.split_once(':')
            .is_some_and(|(n, _)| n.trim().eq_ignore_ascii_case(name))
    })
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sending_over_implicit_tls(db: PgPool) {
    let m = mailbox(db, |_| {}, Security::Tls, |_| {}, |_| {}).await;
    let send = m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let seen = m.smtp.seen();
    assert_eq!(seen.mail_froms, ["ana@bakery.example"]);
    assert_eq!(seen.rcpts, ["bea@example.com"]);
    assert_eq!(seen.datas, 1);
    assert!(!seen.password_in_clear && !seen.mail_in_clear);
    let parsed = normalize::parse(seen.delivered[0].as_slice(), Receiver::Imap);
    assert_eq!(
        parsed.rfc_message_id.as_deref(),
        Some(format!("{send}@bakery.example").as_str())
    );
    assert_eq!(parsed.in_reply_to.as_deref(), Some("order-1@example.com"));
    assert_eq!(parsed.references, ["order-1@example.com"]);
    assert_eq!(parsed.from.address, "ana@bakery.example");
    // The send is known by the key the reader gives its copy.
    let done = m.completed().await;
    let key = m.key(send).await;
    assert!(key.starts_with("mid:"), "{key}");
    assert_eq!(done[0]["provider_message_id"], key.as_str());
    // No address, subject or body in the event log.
    let text = done[0].to_string();
    assert!(!text.contains("bea@") && !text.contains("Friday"), "{text}");
    // The sent message is announced at once, outbound and read.
    let changes = m.changes().await;
    let sent: Vec<&Value> = changes
        .iter()
        .filter(|c| c["message"]["provider_message_id"] == key.as_str())
        .collect();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["message"]["direction"], "outbound");
    assert_eq!(sent[0]["message"]["read"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sending_over_starttls(db: PgPool) {
    let m = usual(db, |_| {}).await;
    m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let seen = m.smtp.seen();
    assert_eq!(seen.delivered.len(), 1);
    assert_eq!(seen.datas, 1);
    // Upgraded before AUTH and MAIL: nothing in the clear.
    assert!(!seen.password_in_clear && !seen.mail_in_clear);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_starttls_on_587_means_no_auth_and_no_mail(db: PgPool) {
    let m = usual(db, |smtp| smtp.starttls = false).await;
    let before = m.smtp.seen().sign_ins;
    m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("unavailable".to_owned()))
    );
    let seen = m.smtp.seen();
    assert_eq!(seen.sign_ins, before, "no AUTH after the connect check");
    assert!(!seen.mail_from && !seen.password_in_clear);
    assert_eq!(seen.datas, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn from_another_address_is_not_sent(db: PgPool) {
    let m = usual(db, |_| {}).await;
    let before = m.smtp.seen().connections;
    m.send(|d| d.from = person(Some("Boss"), "boss@bakery.example"))
        .await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(m.smtp.seen().connections, before, "no SMTP session opened");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn bcc_goes_in_the_envelope_only(db: PgPool) {
    let m = usual(db, |_| {}).await;
    let send = m
        .send(|d| d.bcc = vec![person(None, "archive@example.org")])
        .await;
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let seen = m.smtp.seen();
    assert_eq!(seen.rcpts, ["bea@example.com", "archive@example.org"]);
    let delivered = m.delivered();
    assert!(!has_header(&delivered, "Bcc"), "{delivered}");
    assert!(!delivered.contains("archive@example.org"));
    // The Sent copy keeps it.
    m.run().await;
    assert_eq!(m.copy(send).await, (Some("appended".to_owned()), None));
    let appended = m.imap.seen().appended;
    assert_eq!(appended.len(), 1);
    let copy = String::from_utf8(appended[0].1.clone()).unwrap();
    assert!(has_header(&copy, "Bcc"), "{copy}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn busy_then_accepted_is_delivered_once(db: PgPool) {
    // Busy at the first recipient (before DATA), then at the first message
    // (after it): each retried, one delivery.
    let m = usual(db, |smtp| {
        smtp.smtp.busy_rcpts = 1;
        smtp.smtp.busy_messages = 1;
    })
    .await;
    m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let seen = m.smtp.seen();
    assert_eq!(seen.delivered.len(), 1);
    assert_eq!(
        seen.received.len(),
        2,
        "the busy answer, then the one taken"
    );
    assert_eq!(seen.datas, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_lost_after_the_message_is_never_resent(db: PgPool) {
    let m = usual(db, |smtp| smtp.smtp.hang_up_after_message = true).await;
    let send = m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("unknown_outcome".to_owned()))
    );
    let seen = m.smtp.seen();
    assert_eq!(seen.datas, 1, "one DATA, never a second");
    assert_eq!(seen.received.len(), 1);
    // Nothing is appended for a message that may not have gone.
    assert_eq!(m.copy(send).await, (None, None));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn larger_than_the_server_takes_is_refused_before_mail(db: PgPool) {
    let m = usual(db, |smtp| smtp.smtp.size = Some(100)).await;
    m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("too_large".to_owned()))
    );
    let seen = m.smtp.seen();
    assert!(seen.mail_froms.is_empty());
    assert_eq!(seen.datas, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn larger_than_campbooks_sends_is_refused_before_connecting(db: PgPool) {
    let m = mailbox(
        db,
        |imap| imap.config.send_max_message_bytes = 64 * 1024,
        Security::Starttls,
        |_| {},
        |_| {},
    )
    .await;
    let before = m.smtp.seen().connections;
    m.send(|d| d.text = "Friday. ".repeat(20_000)).await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("too_large".to_owned()))
    );
    assert_eq!(m.smtp.seen().connections, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn over_the_hourly_allowance_a_send_waits(db: PgPool) {
    let m = mailbox(
        db,
        |imap| imap.config.send_per_hour = 1,
        Security::Starttls,
        |_| {},
        |_| {},
    )
    .await;
    m.send(|_| {}).await;
    m.send(|d| d.subject = "Re: Order, again".into()).await;
    m.kit.run_jobs().await;

    // One went; the other waits for the budget, nothing sent for it.
    let done = m.completed().await;
    assert_eq!(done.len(), 1, "{done:?}");
    assert_eq!(done[0]["outcome"], "sent");
    let seen = m.smtp.seen();
    assert_eq!((seen.datas, seen.delivered.len()), (1, 1));
    assert!(m.kit.pending("connectors.send_message").await >= 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn too_many_recipients_are_refused(db: PgPool) {
    let m = mailbox(
        db,
        |imap| imap.config.send_recipients_per_message = 1,
        Security::Starttls,
        |_| {},
        |_| {},
    )
    .await;
    let before = m.smtp.seen().connections;
    m.send(|d| d.cc = vec![person(None, "cleo@example.com")])
        .await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(m.smtp.seen().connections, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_recipient_refuses_the_whole_send(db: PgPool) {
    let m = usual(db, |smtp| {
        smtp.smtp.refuse_rcpt = Some("cleo@example.com".into());
    })
    .await;
    m.send(|d| d.cc = vec![person(None, "cleo@example.com")])
        .await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("rejected".to_owned()))
    );
    assert_eq!(
        m.smtp.seen().datas,
        0,
        "never DATA for part of the recipients"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn appended_with_appenduid_and_no_echo(db: PgPool) {
    let m = usual(db, |_| {}).await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    let key = m.key(send).await;
    let announced = m.changes().await.len();
    let downloads = m.imap.seen().bodies;
    let inbox = m.imap.mailbox(|store| store.idle_state("INBOX"));

    m.run().await;
    assert_eq!(m.copy(send).await, (Some("appended".to_owned()), None));
    let uids = m.imap.mailbox(|store| store.uids("Sent"));
    assert_eq!(uids.len(), 1);
    assert!(m.imap.is_read("Sent", uids[0]), "appended \\Seen");
    assert_eq!(m.imap.seen().count("APPEND"), 1);
    // APPENDUID recorded where it went; the read found it there.
    assert_eq!(
        m.locations(&key).await,
        [("Sent".to_owned(), i64::from(uids[0]))]
    );
    // Neither that run nor the next announces it again or downloads it.
    m.run().await;
    assert_eq!(m.changes().await.len(), announced);
    assert_eq!(m.imap.seen().bodies, downloads);
    assert_eq!(m.imap.seen().count("APPEND"), 1);
    // The Inbox, all IDLE watches, did not change.
    assert_eq!(m.imap.mailbox(|store| store.idle_state("INBOX")), inbox);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn appended_without_appenduid_and_no_echo(db: PgPool) {
    let m = mailbox(db, |_| {}, Security::Starttls, |_| {}, |_| {}).await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    let key = m.key(send).await;
    let announced = m.changes().await.len();

    m.run().await;
    m.run().await;
    assert_eq!(m.copy(send).await, (Some("appended".to_owned()), None));
    let uids = m.imap.mailbox(|store| store.uids("Sent"));
    // The read found the copy by its Message-ID, a known message.
    assert_eq!(
        m.locations(&key).await,
        [("Sent".to_owned(), i64::from(uids[0]))]
    );
    assert_eq!(m.changes().await.len(), announced);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_provider_that_keeps_its_own_copy_gets_no_append(db: PgPool) {
    let m = mailbox(
        db,
        |imap| imap.sent_keepers = vec![SMTP_HOST.to_owned()],
        Security::Starttls,
        |imap| imap.offers.uidplus = true,
        |_| {},
    )
    .await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(m.copy(send).await, (Some("provider".to_owned()), None));
    // The server files what it sent in Sent, read.
    let delivered = m.smtp.seen().delivered[0].clone();
    let uid = m.imap.deliver("Sent", &delivered, m.kit.clock.now());
    m.imap
        .mailbox(|store| store.set_flags("Sent", uid, true, false));
    let announced = m.changes().await.len();

    m.run().await;
    assert_eq!(m.imap.seen().count("APPEND"), 0);
    let key = m.key(send).await;
    assert_eq!(
        m.locations(&key).await,
        [("Sent".to_owned(), i64::from(uid))]
    );
    assert_eq!(
        m.changes().await.len(),
        announced,
        "read as the sent message"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_copy_already_in_sent_is_not_appended_again(db: PgPool) {
    let m = usual(db, |_| {}).await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    // A server not on the list that files a copy itself, at once.
    let delivered = m.smtp.seen().delivered[0].clone();
    let uid = m.imap.deliver("Sent", &delivered, m.kit.clock.now());
    m.imap
        .mailbox(|store| store.set_flags("Sent", uid, true, false));
    let announced = m.changes().await.len();

    m.run().await;
    assert_eq!(m.copy(send).await, (Some("found".to_owned()), None));
    assert_eq!(m.imap.seen().count("APPEND"), 0);
    assert_eq!(m.imap.mailbox(|store| store.uids("Sent")), [uid]);
    assert_eq!(m.changes().await.len(), announced);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_the_server_refuses_never_touches_the_send(db: PgPool) {
    let m = mailbox(
        db,
        |imap| imap.config.sweep_minutes = 1,
        Security::Starttls,
        |imap| {
            imap.offers.uidplus = true;
            imap.offers.append_refused = true;
        },
        |_| {},
    )
    .await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    let key = m.key(send).await;

    m.run().await;
    assert_eq!(
        m.copy(send).await,
        (Some("failed".to_owned()), Some("append_refused".to_owned()))
    );
    // Sent once, recorded sent, never again; and it stays in its
    // conversation through later runs and a sweep.
    m.run().await;
    m.kit.tick(TimeDelta::minutes(5)).await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let seen = m.smtp.seen();
    assert_eq!((seen.datas, seen.delivered.len()), (1, 1));
    assert_eq!(m.imap.seen().count("APPEND"), 1);
    let removed = m
        .changes()
        .await
        .iter()
        .any(|c| c["op"] == "remove" && c["provider_message_id"] == key.as_str());
    assert!(!removed, "the sent message stays");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_busy_append_waits_for_the_next_run(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        Security::Starttls,
        |imap| {
            imap.offers.uidplus = true;
            imap.offers.append_busy = true;
        },
        |_| {},
    )
    .await;
    let send = m.send(|_| {}).await;
    m.settle().await;

    m.run().await;
    assert_eq!(m.copy(send).await, (Some("pending".to_owned()), None));
    m.imap.script(|imap| imap.offers.append_busy = false);
    // Throttled for twice the interval at most.
    m.kit.tick(TimeDelta::hours(1)).await;
    assert_eq!(m.copy(send).await, (Some("appended".to_owned()), None));
    assert_eq!(m.imap.mailbox(|store| store.uids("Sent")).len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_that_never_ends_is_bounded(db: PgPool) {
    let m = mailbox(
        db,
        |imap| {
            imap.config.timeout_seconds = 2;
            imap.config.connect_timeout_seconds = 1;
        },
        Security::Starttls,
        |imap| imap.offers.append_hang = true,
        |_| {},
    )
    .await;
    let send = m.send(|_| {}).await;
    m.settle().await;
    assert_eq!(m.outcome().await, ("sent".to_owned(), None));

    m.run().await;
    let attempts: i32 =
        sqlx::query_scalar("SELECT sent_copy_attempts FROM connectors.sends WHERE id = $1")
            .bind(send)
            .fetch_one(&m.kit.db)
            .await
            .unwrap();
    assert_eq!(m.copy(send).await, (Some("pending".to_owned()), None));
    assert_eq!(attempts, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn larger_than_appendlimit_is_skipped(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        Security::Starttls,
        |imap| {
            imap.offers.uidplus = true;
            imap.offers.append_limit = Some(10);
        },
        |_| {},
    )
    .await;
    let send = m.send(|_| {}).await;
    m.settle().await;

    m.run().await;
    assert_eq!(
        m.copy(send).await,
        (Some("skipped".to_owned()), Some("too_large".to_owned()))
    );
    assert_eq!(m.imap.seen().count("APPEND"), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_password_is_counted_until_the_member_is_needed(db: PgPool) {
    let m = usual(db, |smtp| smtp.password = "changed".into()).await;
    m.send(|_| {}).await;
    m.settle().await;

    assert_eq!(
        m.outcome().await,
        ("failed".to_owned(), Some("needs_reauth".to_owned()))
    );
    assert_eq!(
        m.smtp.seen().sign_ins,
        1 + 3,
        "the connect check, then three"
    );
    assert_eq!(m.health().await, "needs_reauth");
    assert!(m.smtp.seen().mail_froms.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_busy_sign_in_is_retried_and_never_counted(db: PgPool) {
    let m = usual(db, |smtp| smtp.smtp.busy_auth = true).await;
    m.send(|_| {}).await;
    m.kit.run_jobs().await;
    m.smtp.script(|smtp| smtp.smtp.busy_auth = false);
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let refusals: Option<i32> =
        sqlx::query_scalar("SELECT (cursor->>'login_refusals')::int FROM connectors.streams")
            .fetch_one(&m.kit.db)
            .await
            .unwrap();
    assert_eq!(refusals.unwrap_or(0), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_line_break_in_a_subject_or_a_name_adds_no_header(db: PgPool) {
    let m = usual(db, |_| {}).await;
    m.send(|d| {
        d.subject = "Re: Order\r\nBcc: evil@example.org".into();
        d.to = vec![person(
            Some("Bea\r\nBcc: evil2@example.org"),
            "bea@example.com",
        )];
    })
    .await;
    m.settle().await;

    assert_eq!(m.outcome().await, ("sent".to_owned(), None));
    let delivered = m.delivered();
    assert!(!has_header(&delivered, "Bcc"), "{delivered}");
    assert_eq!(m.smtp.seen().rcpts, ["bea@example.com"]);
    let parsed = normalize::parse(delivered.as_bytes(), Receiver::Imap);
    assert_eq!(parsed.subject, "Re: Order  Bcc: evil@example.org");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_that_never_stops_talking_is_cut_off(db: PgPool) {
    let m = usual(db, |smtp| smtp.smtp.endless_ehlo = true).await;
    m.send(|_| {}).await;
    m.kit.run_jobs().await;

    // Nothing sent, nothing settled: the send waits to try again.
    assert!(m.completed().await.is_empty());
    let seen = m.smtp.seen();
    assert!(seen.mail_froms.is_empty() && seen.datas == 0);
    assert!(m.kit.pending("connectors.send_message").await >= 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sent_copies_are_isolated_by_workspace(db: PgPool) {
    let m = usual(db, |_| {}).await;
    m.send(|_| {}).await;
    m.settle().await;
    let copies = "SELECT count(*) FROM connectors.sends WHERE sent_copy IS NOT NULL";

    let app = testing::app_role_pool(&m.kit.db).await;
    let mut tx = app.begin_for(m.ws).await.unwrap();
    let own: i64 = sqlx::query_scalar(copies)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(own, 1);
    tx.rollback().await.unwrap();

    let mut tx = app.begin_for(WorkspaceId::new()).await.unwrap();
    let other: i64 = sqlx::query_scalar(copies)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(other, 0);
    let written = sqlx::query("UPDATE connectors.sends SET sent_copy = 'failed'")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(written.rows_affected(), 0);
    tx.rollback().await.unwrap();

    // Without a workspace, nothing.
    let mut tx = app.begin().await.unwrap();
    let none: i64 = sqlx::query_scalar(copies)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(none, 0);
    tx.rollback().await.unwrap();
}
