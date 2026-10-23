//! Members' changes reaching IMAP mailboxes (campbooks-cloud#312), end to
//! end against the fake server: flags and moves made as the server allows
//! (MOVE, UIDPLUS, neither), stream folders created once and bounded, no
//! echo of our own writes (the next run, an IDLE session), stale UIDs and
//! messages moved elsewhere left alone, the server's later changes winning,
//! and a server that is busy, lies, or is shared with an IDLE session.

use std::time::Duration;

use axum::http::StatusCode;
use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::time::Instant;

use super::{imap_rate, imap_slots};
use crate::{
    config::ImapConfig,
    domain::schedule,
    events::ChangesRequested,
    features::imap_idle::{IdlePool, Timing},
    testkit::{Caller, Kit, assert_contract},
    testkit_imap::{self, FakeServer, Fakes, IMAP_HOST, Script},
};

/// A member's IMAP mailbox at the fake, connected and read once.
struct Mailbox {
    kit: Kit,
    imap: FakeServer,
    ws: WorkspaceId,
    inbox: String,
    _ana: Caller,
    _smtp: FakeServer,
    _fakes: Fakes,
}

/// A fictional message from Bea of the bakery.
fn from_bea(id: &str, subject: &str) -> Vec<u8> {
    format!(
        "From: Bea <bea@bakery.example>\r\nTo: Ana <ana@bakery.example>\r\n\
         Subject: {subject}\r\nDate: Mon, 5 Oct 2026 10:00:00 +0000\r\n\
         Message-ID: <{id}>\r\n\r\nTwo sourdough loaves for Friday, please.\r\n"
    )
    .into_bytes()
}

/// Ana's own reply.
fn from_ana(id: &str, subject: &str) -> Vec<u8> {
    format!(
        "From: Ana <ana@bakery.example>\r\nTo: Bea <bea@bakery.example>\r\n\
         Subject: {subject}\r\nDate: Mon, 5 Oct 2026 11:00:00 +0000\r\n\
         Message-ID: <{id}>\r\n\r\nFriday it is.\r\n"
    )
    .into_bytes()
}

/// Ana's mailbox: the fake set up by `script` and `seed` (an Archive, a
/// Trash and a Junk folder are always there), Campbooks by `config`, then
/// connected and read until its backfill ends.
async fn mailbox(
    db: PgPool,
    config: impl FnOnce(&mut ImapConfig),
    script: impl FnOnce(&mut Script),
    seed: impl FnOnce(&FakeServer, DateTime<Utc>),
) -> Mailbox {
    let (kit, fakes) = testkit_imap::kit_with(db, config).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let (imap, smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw").await;
    imap.mailbox(|store| {
        store.add_folder("Archive", &["\\Archive"]);
        store.add_folder("Trash", &["\\Trash"]);
        store.add_folder("Junk", &["\\Junk"]);
        store.add_folder("Sent", &["\\Sent"]);
    });
    imap.script(script);
    seed(&imap, kit.clock.now());
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            json!({
                "address": "ana@bakery.example",
                "password": "pw",
                "servers": servers,
            }),
        )
        .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental", "the backfill ended");
    Mailbox {
        inbox: stream["inbox_id"].as_str().unwrap().to_owned(),
        kit,
        imap,
        ws,
        _ana: ana,
        _smtp: smtp,
        _fakes: fakes,
    }
}

/// A server that moves with MOVE and UIDPLUS (and UNSELECT).
fn moving(script: &mut Script) {
    script.offers.moves = true;
    script.offers.uidplus = true;
    script.offers.unselect = true;
}

impl Mailbox {
    /// conversations asks for `items` (message key, change): the event is
    /// checked against the contract, published, and its consumer and the
    /// stream's run run.
    async fn request(&self, items: &[(&str, Value)]) {
        let conversation = uuid::Uuid::now_v7();
        let ids: Vec<&str> = items.iter().map(|(id, _)| *id).collect();
        self.kit.outbound.show(conversation, &ids);
        let payload = json!({
            "workspace": self.ws.to_string(),
            "inbox_id": self.inbox,
            "conversation": conversation,
            "origin": "user",
            "items": items.iter().map(|(id, change)| json!({
                "provider_message_id": id, "change": change
            })).collect::<Vec<_>>(),
        });
        assert_contract("conversations.changes_requested.v1.schema.json", &payload);
        let event: ChangesRequested = serde_json::from_value(payload).unwrap();
        let mut tx = self.kit.state.db.begin_for(self.ws).await.unwrap();
        self.kit
            .state
            .events
            .publish(&mut tx, &event)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        self.kit.run_jobs().await;
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

    /// The key of the message announced with `subject`.
    async fn key(&self, subject: &str) -> String {
        self.changes()
            .await
            .iter()
            .find(|c| c["message"]["subject"] == subject)
            .and_then(|c| c["message"]["provider_message_id"].as_str())
            .unwrap_or_else(|| panic!("no message {subject}"))
            .to_owned()
    }

    /// Every outbound change as (kind, state, reason), oldest first.
    async fn ends(&self) -> Vec<(String, String, Option<String>)> {
        sqlx::query_as("SELECT kind, state, reason FROM connectors.outbound_changes ORDER BY id")
            .fetch_all(&self.kit.db)
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

    /// How many batches were announced.
    async fn batches(&self) -> usize {
        self.kit.batches(self.ws).await.len()
    }

    async fn health(&self) -> String {
        sqlx::query_scalar("SELECT health FROM connectors.connections")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The fake's UIDs of folder `name`.
    fn uids(&self, name: &str) -> Vec<u32> {
        self.imap.mailbox(|store| store.uids(name))
    }
}

fn end(kind: &str, state: &str, reason: Option<&str>) -> (String, String, Option<String>) {
    (
        kind.to_owned(),
        state.to_owned(),
        reason.map(ToOwned::to_owned),
    )
}

/// Whether the fake ever received a command that empties a folder or
/// removes one: `EXPUNGE` without UIDs, `CLOSE`, `DELETE`, `RENAME`.
fn never_destroyed(imap: &FakeServer) {
    let seen = imap.seen();
    for verb in ["EXPUNGE", "CLOSE", "DELETE", "RENAME"] {
        assert_eq!(seen.count(verb), 0, "{verb} sent: {:?}", seen.commands);
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn read_and_starred_round_trip_without_echo(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    let batches = m.batches().await;

    // One change per message and request (the recorder's key).
    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    m.request(&[(key.as_str(), json!({"kind": "star"}))]).await;
    let mail = m.imap.mailbox(|store| store.mail("INBOX", uid).clone());
    assert!(mail.seen && mail.flagged);
    assert_eq!(
        m.ends().await,
        [
            end("mark_read", "applied", None),
            end("star", "applied", None)
        ]
    );
    let seen = m.imap.seen();
    assert_eq!(seen.count("UID STORE"), 2);
    for flag in ["\\Seen", "\\Flagged"] {
        let written = format!("+FLAGS.SILENT ({flag})");
        assert!(
            seen.lines
                .iter()
                .any(|l| l.contains("UNCHANGEDSINCE") && l.contains(&written)),
            "{:?}",
            seen.lines
        );
    }
    assert!(seen.selected, "opened read-write to write");
    // Neither the run that wrote nor the next announces it.
    assert_eq!(m.batches().await, batches);
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches);
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn unread_and_unstarred_clear_every_copy(db: PgPool) {
    let (mut inbox, mut copy) = (0, 0);
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            inbox = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
            imap.mailbox(|store| {
                store.set_flags("INBOX", inbox, true, true);
                copy = store.copy("INBOX", inbox, "Bakery");
            });
        },
    )
    .await;
    let key = m.key("Hello").await;
    assert_eq!(m.locations(&key).await.len(), 2);

    m.request(&[(key.as_str(), json!({"kind": "mark_unread"}))])
        .await;
    m.request(&[(key.as_str(), json!({"kind": "unstar"}))])
        .await;
    for (folder, uid) in [("INBOX", inbox), ("Bakery", copy)] {
        let mail = m.imap.mailbox(|store| store.mail(folder, uid).clone());
        assert!(!mail.seen && !mail.flagged, "{folder}");
    }
    assert!(
        m.ends()
            .await
            .iter()
            .all(|(_, state, _)| state == "applied")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn archive_with_move(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    let batches = m.batches().await;

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    assert!(m.uids("INBOX").is_empty());
    let archived = m.uids("Archive");
    assert_eq!(archived.len(), 1);
    assert_eq!(m.imap.seen().count("UID MOVE"), 1);
    assert_eq!(
        m.locations(&key).await,
        [("Archive".to_owned(), i64::from(archived[0]))],
        "located at its new UID"
    );
    assert_eq!(m.ends().await, [end("archive", "applied", None)]);
    assert_eq!(m.batches().await, batches);
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches, "no echo");
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn archive_with_uidplus_only(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.uidplus = true,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("b@bakery.example", "Other"), now);
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    let batches = m.batches().await;

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    let seen = m.imap.seen();
    assert_eq!(seen.count("UID MOVE"), 0);
    assert_eq!(seen.count("UID COPY"), 1);
    assert!(
        seen.lines
            .iter()
            .any(|l| l.contains("+FLAGS.SILENT (\\Deleted)"))
    );
    assert!(
        seen.lines
            .iter()
            .any(|l| l == &format!("UID EXPUNGE {uid}")),
        "{:?}",
        seen.lines
    );
    assert_eq!(m.uids("INBOX").len(), 1, "the other message stays");
    assert_eq!(m.uids("Archive").len(), 1);
    assert_eq!(m.ends().await, [end("archive", "applied", None)]);
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches, "no echo");
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_move_and_no_uidplus(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(m.uids("INBOX").len(), 1, "it stays in the Inbox");
    assert!(m.uids("Archive").is_empty());
    assert_eq!(
        m.ends().await,
        [end("archive", "skipped", Some("unsupported_here"))]
    );
    let seen = m.imap.seen();
    for verb in ["UID MOVE", "UID COPY", "UID EXPUNGE", "UID STORE"] {
        assert_eq!(seen.count(verb), 0, "{verb}");
    }
    assert!(!seen.lines.iter().any(|l| l.contains("\\Deleted")));
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_apps_deleted_message_survives(db: PgPool) {
    let mut doomed = 0;
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.uidplus = true,
        |imap, now| {
            doomed = imap.deliver("INBOX", &from_bea("old@bakery.example", "Old"), now);
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    // Marked deleted in another app, not expunged yet.
    m.imap.mailbox(|store| store.set_deleted("INBOX", doomed));

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(m.uids("INBOX"), [doomed]);
    assert!(m.imap.mailbox(|store| store.mail("INBOX", doomed).deleted));
    assert_eq!(m.uids("Archive").len(), 1);
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            let uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
            imap.mailbox(|store| store.copy("INBOX", uid, "Bakery"));
        },
    )
    .await;
    let key = m.key("Hello").await;
    let batches = m.batches().await;

    m.request(&[(key.as_str(), json!({"kind": "trash"}))]).await;
    assert!(m.uids("INBOX").is_empty() && m.uids("Bakery").is_empty());
    assert_eq!(m.uids("Trash").len(), 2);
    assert_eq!(m.ends().await, [end("trash", "applied", None)]);
    assert!(
        m.locations(&key)
            .await
            .iter()
            .all(|(folder, _)| folder == "Trash")
    );
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches, "no echo");
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn untrash_received_mail(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.deliver("Trash", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;

    m.request(&[(key.as_str(), json!({"kind": "untrash", "inbound": true}))])
        .await;
    assert!(m.uids("Trash").is_empty());
    assert_eq!(m.uids("INBOX").len(), 1);
    assert_eq!(m.ends().await, [end("untrash", "applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn unarchive_the_members_own_reply(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.deliver("Sent", &from_ana("reply@bakery.example", "Re: Hello"), now);
        },
    )
    .await;
    let key = m.key("Re: Hello").await;

    m.request(&[(key.as_str(), json!({"kind": "unarchive"}))])
        .await;
    assert_eq!(m.uids("Sent").len(), 1, "it stays in Sent");
    assert!(m.uids("INBOX").is_empty());
    assert_eq!(
        m.ends().await,
        [end("unarchive", "skipped", Some("not_movable"))]
    );
    assert_eq!(m.imap.seen().count("SELECT"), 0, "nothing to write");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_label_change(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;

    m.request(&[(
        key.as_str(),
        json!({"kind": "add_label", "label": uuid::Uuid::now_v7(), "provider_key": "Bakery"}),
    )])
    .await;
    assert_eq!(
        m.ends().await,
        [end("add_label", "skipped", Some("unsupported"))]
    );
    assert_eq!(m.imap.seen().count("SELECT"), 0);
    assert!(m.uids("Bakery").is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stream_folder_is_created_once_and_reused(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Weekly news"), now);
            imap.deliver("INBOX", &from_bea("b@bakery.example", "Monthly news"), now);
        },
    )
    .await;
    let (weekly, monthly) = (m.key("Weekly news").await, m.key("Monthly news").await);
    let batches = m.batches().await;
    let to_newsletters = json!({"kind": "move_to_stream", "stream": "newsletters"});

    m.request(&[(weekly.as_str(), to_newsletters.clone())])
        .await;
    m.request(&[(monthly.as_str(), to_newsletters)]).await;
    let seen = m.imap.seen();
    assert_eq!(seen.count("CREATE"), 1);
    assert_eq!(seen.count("SUBSCRIBE"), 1);
    assert_eq!(
        m.imap.mailbox(|store| store.created.clone()),
        ["Campbooks.Newsletters"]
    );
    assert_eq!(
        m.imap.mailbox(|store| store.subscribed.clone()),
        ["Campbooks.Newsletters"]
    );
    assert_eq!(m.uids("Campbooks.Newsletters").len(), 2);
    assert!(m.uids("INBOX").is_empty());
    assert_eq!(
        m.ends().await,
        [
            end("move_to_stream", "applied", None),
            end("move_to_stream", "applied", None)
        ]
    );
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches, "no echo");
    never_destroyed(&m.imap);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn back_to_primary(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Campbooks.Newsletters", &[]));
            imap.deliver(
                "Campbooks.Newsletters",
                &from_bea("a@bakery.example", "Weekly news"),
                now,
            );
        },
    )
    .await;
    let key = m.key("Weekly news").await;
    let upsert = m.changes().await[0].clone();
    assert_eq!(upsert["message"]["mailbox"]["in_inbox"], true);
    assert_eq!(upsert["message"]["provider_stream"], "newsletters");

    m.request(&[(
        key.as_str(),
        json!({"kind": "move_to_stream", "stream": "primary"}),
    )])
    .await;
    assert!(m.uids("Campbooks.Newsletters").is_empty());
    assert_eq!(m.uids("INBOX").len(), 1);
    assert_eq!(m.ends().await, [end("move_to_stream", "applied", None)]);
    assert_eq!(m.imap.seen().count("CREATE"), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_folder_creation_cap(db: PgPool) {
    let m = mailbox(
        db,
        |config| config.max_created_folders = 1,
        moving,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Update"), now);
            imap.deliver("INBOX", &from_bea("b@bakery.example", "News"), now);
        },
    )
    .await;
    let (update, news) = (m.key("Update").await, m.key("News").await);

    m.request(&[
        (
            update.as_str(),
            json!({"kind": "move_to_stream", "stream": "updates"}),
        ),
        (
            news.as_str(),
            json!({"kind": "move_to_stream", "stream": "newsletters"}),
        ),
    ])
    .await;
    assert_eq!(
        m.imap.mailbox(|store| store.created.clone()),
        ["Campbooks.Updates"]
    );
    assert_eq!(m.uids("Campbooks.Updates").len(), 1);
    assert_eq!(m.uids("INBOX").len(), 1, "the news stays in the Inbox");
    let mut ends = m.ends().await;
    ends.sort();
    assert_eq!(
        ends,
        [
            end("move_to_stream", "applied", None),
            end("move_to_stream", "skipped", Some("folder_cap"))
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sorted_in_another_app(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Campbooks.Updates", &[]));
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Shipping"), now);
        },
    )
    .await;
    let batches = m.batches().await;
    m.imap
        .mailbox(|store| store.move_mail("INBOX", uid, "Campbooks.Updates"));
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches + 1);
    let last = m.changes().await.last().cloned().unwrap();
    assert_eq!(last["message"]["subject"], "Shipping");
    assert_eq!(last["message"]["mailbox"]["in_inbox"], true);
    assert_eq!(last["message"]["provider_stream"], "updates");
    assert!(
        last["message"]["labels"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "a stream folder is not a label: {last}"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn uidvalidity_changed_before_the_write(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    m.imap.mailbox(|store| store.reset_uidvalidity("INBOX"));

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(
        m.imap.seen().count("UID STORE"),
        0,
        "no write on a stale UID"
    );
    assert_eq!(m.ends().await, [end("mark_read", "pending", None)]);
    let uid = m.uids("INBOX")[0];
    assert!(!m.imap.is_read("INBOX", uid));
    // The read re-located it: the next run writes it at its new UID.
    assert_eq!(
        m.locations(&key).await,
        [("INBOX".to_owned(), i64::from(uid))]
    );
    m.kit.next_run().await;
    assert!(m.imap.is_read("INBOX", uid));
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn moved_elsewhere_first(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    // Filed in another app before Campbooks read it again.
    m.imap
        .mailbox(|store| store.move_mail("INBOX", uid, "Bakery"));

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(m.imap.seen().count("UID MOVE"), 0);
    assert_eq!(
        m.ends().await,
        [end("archive", "skipped", Some("message_gone"))]
    );
    assert_eq!(m.uids("Bakery").len(), 1);
    let last = m.changes().await.last().cloned().unwrap();
    assert_eq!(last["message"]["provider_message_id"], key.as_str());
    assert_eq!(last["message"]["mailbox"]["in_inbox"], false);
    assert_eq!(last["message"]["labels"][0]["name"], "Bakery");
    m.kit.next_run().await;
    assert_eq!(m.health().await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn changed_at_the_server_since_the_last_read(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    // Read in another app; Campbooks has not seen it yet.
    m.imap
        .mailbox(|store| store.set_flags("INBOX", uid, true, false));

    m.request(&[(key.as_str(), json!({"kind": "star"}))]).await;
    let mail = m.imap.mailbox(|store| store.mail("INBOX", uid).clone());
    assert!(mail.seen && !mail.flagged, "the server's state stands");
    assert_eq!(
        m.ends().await,
        [end("star", "superseded", Some("changed_at_provider"))]
    );
    let last = m.changes().await.last().cloned().unwrap();
    assert_eq!(last["message"]["provider_message_id"], key.as_str());
    assert_eq!(last["message"]["read"], true);
    assert_eq!(last["message"]["starred"], false);
}

/// Windows short enough for a test (see `imap_idle`'s tests).
fn timing() -> Timing {
    Timing {
        refresh: Duration::from_mins(1),
        debounce: Duration::from_mins(10),
        renew: Duration::from_mins(1),
        lease: Duration::from_mins(3),
        backoff: Duration::from_millis(10),
        backoff_max: Duration::from_millis(40),
        give_up_after: 3,
        cooldown: Duration::from_mins(30),
        db_timeout: Duration::from_secs(20),
    }
}

/// Wait (10 s at most) until `check` holds.
async fn until(what: &str, mut check: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check().await {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Ana's mailbox with IDLE on, its Inbox held in IDLE by a pool: "Hello"
/// in the Inbox, "Filed" in a folder of hers (IDLE does not watch it).
async fn idling(db: PgPool) -> (Mailbox, IdlePool, crate::features::imap_idle::Held) {
    let m = mailbox(
        db,
        |config| config.idle_enabled = true,
        |script| {
            moving(script);
            script.offers.idle = true;
        },
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
            imap.deliver("Bakery", &from_bea("b@bakery.example", "Filed"), now);
        },
    )
    .await;
    let pool = IdlePool::with_timing(m.kit.deps.clone(), m.kit.state.clone(), timing());
    let mut held = IdlePool::held();
    pool.tick(&mut held).await;
    assert_eq!(held.connections().len(), 1, "the lease is claimed");
    until("the session to be in IDLE", async || {
        m.imap.seen().idles >= 1
    })
    .await;
    (m, pool, held)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_idle_echo_of_our_own_change(db: PgPool) {
    let (m, pool, held) = idling(db).await;
    let key = m.key("Hello").await;
    let batches = m.batches().await;

    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(m.uids("Archive").len(), 1);
    // The IDLE session hears the write and asks for a run.
    until("the IDLE session to ask for a run", async || {
        pool.shared().asked() >= 1
    })
    .await;
    m.kit.run_jobs().await;
    assert_eq!(m.batches().await, batches, "that run announces nothing");
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_write_beside_idle(db: PgPool) {
    let (m, pool, held) = idling(db).await;
    // A message IDLE does not hear about, so no run of its asking adds a
    // session here.
    let key = m.key("Filed").await;
    let connections = m.imap.seen().connections;

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
    assert_eq!(
        m.imap.seen().connections,
        connections + 1,
        "one session for the write and the read"
    );
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connectors.imap_idle_leases
          WHERE holder IS NOT NULL AND expires_at > now()",
    )
    .fetch_one(&m.kit.db)
    .await
    .unwrap();
    assert_eq!(live, 1, "the IDLE session keeps its lease");
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_write_waits_for_a_session_slot(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |config| config.sessions_per_host = 4,
        |_| {},
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    // Every session to the host is open elsewhere.
    let (slots, rate) = (imap_slots(IMAP_HOST), imap_rate(4));
    assert_eq!(
        schedule::take(&m.kit.db, &slots, 4, rate).await.unwrap(),
        Ok(())
    );
    let connections = m.imap.seen().connections;

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(m.imap.seen().connections, connections, "no session opened");
    assert_eq!(m.imap.seen().count("UID STORE"), 0);
    assert_eq!(m.ends().await, [end("mark_read", "pending", None)]);

    schedule::settle(&m.kit.db, &slots, 1, 0, rate)
        .await
        .unwrap();
    m.kit.next_run().await;
    assert!(m.imap.is_read("INBOX", uid));
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn busy_while_writing(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    m.imap.script(|script| script.offers.busy_writes = true);

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(m.ends().await, [end("mark_read", "pending", None)]);
    assert_eq!(m.health().await, "throttled");
    assert!(!m.imap.is_read("INBOX", uid));

    m.imap.script(|script| script.offers.busy_writes = false);
    m.kit.next_run().await;
    assert!(m.imap.is_read("INBOX", uid));
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_retry_cap(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    m.imap.script(|script| script.offers.busy_writes = true);

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    for _ in 1..crate::domain::outbound::MAX_ATTEMPTS {
        assert_eq!(m.ends().await, [end("mark_read", "pending", None)]);
        m.kit.next_run().await;
    }
    assert_eq!(
        m.ends().await,
        [end("mark_read", "failed", Some("provider_errors"))]
    );
    let stores = m.imap.seen().count("UID STORE");
    assert_eq!(
        stores,
        usize::try_from(crate::domain::outbound::MAX_ATTEMPTS).unwrap()
    );
    m.imap.script(|script| script.offers.busy_writes = false);
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().count("UID STORE"), stores, "never again");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_that_does_not_apply_what_it_accepts(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    m.imap.script(|script| script.offers.inert_writes = true);
    let batches = m.batches().await;

    m.request(&[(key.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert!(!m.imap.is_read("INBOX", uid));
    assert_eq!(
        m.ends().await,
        [end("mark_read", "failed", Some("refused"))]
    );
    assert_eq!(m.batches().await, batches + 1, "the server's state is told");
    let last = m.changes().await.last().cloned().unwrap();
    assert_eq!(last["message"]["provider_message_id"], key.as_str());
    assert_eq!(last["message"]["read"], false);
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().count("UID STORE"), 1, "never retried");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_workspaces_changes(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        moving,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    let key = m.key("Hello").await;
    m.request(&[(key.as_str(), json!({"kind": "archive"}))])
        .await;

    let app = testing::app_role_pool(&m.kit.db).await;
    let count = async |ws: WorkspaceId| -> (i64, i64) {
        let mut tx = app.begin_for(ws).await.unwrap();
        let counts = sqlx::query_as(
            "SELECT (SELECT count(*) FROM connectors.outbound_changes),
                    (SELECT count(*) FROM connectors.imap_locations)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        counts
    };
    let (changes, locations) = count(m.ws).await;
    assert!(changes >= 1 && locations >= 1);
    let other = WorkspaceId::new();
    assert_eq!(count(other).await, (0, 0));
    let mut tx = app.begin_for(other).await.unwrap();
    let changed = sqlx::query("UPDATE connectors.outbound_changes SET state = 'pending'")
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    let moved = sqlx::query("UPDATE connectors.imap_locations SET folder = 'INBOX'")
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    tx.commit().await.unwrap();
    assert_eq!((changed, moved), (0, 0));
    assert_eq!(m.ends().await, [end("archive", "applied", None)]);
}
