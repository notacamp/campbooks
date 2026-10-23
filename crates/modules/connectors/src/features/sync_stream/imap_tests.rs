//! Reading IMAP mailboxes (campbooks-cloud#311), end to end against the
//! fake server: a backfill then only what moved, one message wherever it
//! is filed, nothing ever marked read, and a server that refuses, is busy,
//! is shared with other mailboxes, or misbehaves.

use axum::http::StatusCode;
use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::PgPool;

use super::{imap_rate, imap_slots, imap_workspace_slots, run_slots, take_slots};
use crate::{
    config::ImapConfig,
    domain::schedule,
    imap::read::MAX_TIMEOUTS,
    testkit::{Caller, Kit, assert_contract},
    testkit_imap::{self, FakeServer, Fakes, IMAP_HOST, Script},
};

/// A member's IMAP mailbox at the fake, connected (not read yet).
struct Mailbox {
    kit: Kit,
    imap: FakeServer,
    ws: WorkspaceId,
    _ana: Caller,
    _smtp: FakeServer,
    _fakes: Fakes,
}

/// A fictional message, sent by `from`; with `Message-ID: <id>` when given.
fn raw(id: Option<&str>, from: &str, subject: &str) -> Vec<u8> {
    let mut message = format!(
        "From: {from}\r\nTo: Ana <ana@bakery.example>\r\nSubject: {subject}\r\n\
         Date: Mon, 5 Oct 2026 10:00:00 +0000\r\n"
    );
    if let Some(id) = id {
        message.push_str("Message-ID: <");
        message.push_str(id);
        message.push_str(">\r\n");
    }
    message.push_str("\r\nTwo sourdough loaves for Friday, please.\r\n");
    message.into_bytes()
}

/// A message from Bea of the bakery.
fn from_bea(id: &str, subject: &str) -> Vec<u8> {
    raw(Some(id), "Bea <bea@bakery.example>", subject)
}

/// Ana's mailbox: the fake set up by `script` and `seed`, Campbooks by
/// `config`, then connected.
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
    seed(&imap, kit.clock.now());
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            serde_json::json!({
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
    imap.script(script);
    Mailbox {
        kit,
        imap,
        ws,
        _ana: ana,
        _smtp: smtp,
        _fakes: fakes,
    }
}

impl Mailbox {
    /// Every change announced so far, in order, each batch checked against
    /// the contract.
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

    /// The changes of the last batch.
    async fn last(&self) -> Vec<Value> {
        let batches = self.kit.batches(self.ws).await;
        batches.last().unwrap()["changes"]
            .as_array()
            .cloned()
            .unwrap()
    }

    async fn health(&self) -> String {
        sqlx::query_scalar("SELECT health FROM connectors.connections")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    async fn count(&self, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }
}

/// The message an upsert carries, or the id a removal names.
fn id(change: &Value) -> String {
    change["message"]["provider_message_id"]
        .as_str()
        .or_else(|| change["provider_message_id"].as_str())
        .unwrap()
        .to_owned()
}

fn subject(change: &Value) -> &str {
    change["message"]["subject"].as_str().unwrap()
}

/// The upsert of the message with `subject` among `changes` (the last).
fn of<'a>(changes: &'a [Value], subject: &str) -> &'a Value {
    changes
        .iter()
        .rev()
        .find(|c| c["message"]["subject"] == subject)
        .unwrap_or_else(|| panic!("no change of {subject}"))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_is_read_once_then_only_what_arrives(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Sent", &["\\Sent"]));
            for n in 1..=3 {
                imap.deliver(
                    "INBOX",
                    &from_bea(&format!("order-{n}@bakery.example"), &format!("Order {n}")),
                    now - TimeDelta::hours(n),
                );
            }
            // Older than the window: never read.
            imap.deliver(
                "INBOX",
                &from_bea("old@bakery.example", "Last year"),
                now - TimeDelta::days(200),
            );
            imap.deliver(
                "Sent",
                &raw(
                    Some("reply@bakery.example"),
                    "Ana <ana@bakery.example>",
                    "Re: Order 1",
                ),
                now - TimeDelta::minutes(30),
            );
        },
    )
    .await;
    m.kit.run_jobs().await;

    let changes = m.changes().await;
    let mut ids: Vec<String> = changes.iter().map(id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 4, "{changes:#?}");
    assert_eq!(changes.len(), 4);
    assert!(
        changes
            .iter()
            .all(|c| c["message"]["subject"] != "Last year")
    );
    let batches = m.kit.batches(m.ws).await;
    assert!(batches.iter().all(|b| b["backfill"] == true));
    let order = of(&changes, "Order 2");
    assert_eq!(order["message"]["mailbox"]["in_inbox"], true);
    assert_eq!(order["message"]["read"], false);
    assert_eq!(order["message"]["direction"], "inbound");
    // Identified by its Message-ID, never named in the id.
    assert!(id(order).starts_with("mid:") && !id(order).contains("bakery"));
    let reply = of(&changes, "Re: Order 1");
    assert_eq!(reply["message"]["direction"], "outbound");
    assert_eq!(reply["message"]["mailbox"]["in_inbox"], false);
    assert_eq!(m.kit.stream().await["phase"], "incremental");
    assert_eq!(m.imap.seen().bodies, 4);
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.imap_locations")
            .await,
        4
    );

    // A message arrives: the next run announces it alone.
    let now = m.kit.clock.now();
    m.imap.deliver(
        "INBOX",
        &from_bea("order-4@bakery.example", "Order 4"),
        now + TimeDelta::seconds(1),
    );
    m.kit.next_run().await;
    let last = m.last().await;
    assert_eq!(last.len(), 1, "{last:#?}");
    assert_eq!(subject(&last[0]), "Order 4");
    assert_ne!(m.kit.batches(m.ws).await.last().unwrap()["backfill"], true);
    assert_eq!(m.imap.seen().bodies, 5);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nothing_moved_opens_no_folder(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.condstore = false,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let opened = m.imap.seen().opened.len();
    let batches = m.kit.batches(m.ws).await.len();
    assert!(opened > 0);

    m.kit.next_run().await;
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().opened.len(), opened);
    assert_eq!(m.kit.batches(m.ws).await.len(), batches);
    assert_eq!(m.health().await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn read_in_another_app_is_read_here_without_a_download(db: PgPool) {
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
    m.kit.run_jobs().await;
    let first = id(&m.changes().await[0]);

    m.imap
        .mailbox(|store| store.set_flags("INBOX", uid, true, true));
    m.kit.next_run().await;
    let last = m.last().await;
    assert_eq!(last.len(), 1);
    assert_eq!(id(&last[0]), first);
    assert_eq!(last[0]["message"]["read"], true);
    assert_eq!(last[0]["message"]["starred"], true);
    assert_eq!(m.imap.seen().bodies, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn reading_never_marks_mail_read(db: PgPool) {
    let mut uids = Vec::new();
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.qresync = true,
        |imap, now| {
            for n in 1..=2 {
                uids.push(imap.deliver(
                    "INBOX",
                    &from_bea(&format!("{n}@bakery.example"), "Unread"),
                    now,
                ));
            }
        },
    )
    .await;
    m.kit.run_jobs().await;
    m.kit.next_run().await;
    let seen = m.imap.seen();
    assert_eq!(seen.bodies, 2);
    assert_eq!(seen.marked_read, 0);
    assert_eq!(seen.changes, 0);
    assert!(!seen.selected, "only ever opened read-only");
    for uid in uids {
        assert!(!m.imap.is_read("INBOX", uid));
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_without_special_use_is_read_by_folder_names(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.special_use = false,
        |imap, now| {
            imap.mailbox(|store| {
                store.add_folder("INBOX.Sent Items", &["\\Sent"]);
                store.add_folder("INBOX.Bulk Mail", &["\\Junk"]);
                store.add_folder("INBOX.Drafts", &["\\Drafts"]);
            });
            imap.deliver(
                "INBOX.Sent Items",
                &raw(
                    Some("s@bakery.example"),
                    "Ana <ana@bakery.example>",
                    "Sent one",
                ),
                now,
            );
            imap.deliver(
                "INBOX.Bulk Mail",
                &from_bea("j@bakery.example", "Junk one"),
                now,
            );
            imap.deliver(
                "INBOX.Drafts",
                &from_bea("d@bakery.example", "Draft one"),
                now,
            );
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 2, "drafts are never read: {changes:#?}");
    let sent = of(&changes, "Sent one");
    assert_eq!(sent["message"]["direction"], "outbound");
    let junk = of(&changes, "Junk one");
    assert_eq!(junk["message"]["mailbox"]["in_spam"], true);
    assert_eq!(junk["message"]["mailbox"]["in_inbox"], false);
    assert!(!m.imap.seen().opened.iter().any(|f| f == "INBOX.Drafts"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_oversized_message_is_recorded_not_downloaded(db: PgPool) {
    let mut lying = 0;
    let m = mailbox(
        db,
        |config| config.max_message_bytes = 600,
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("small@bakery.example", "Small"), now);
            let mut big = from_bea("big@bakery.example", "Big");
            big.extend(std::iter::repeat_n(b'x', 2_000));
            imap.deliver("INBOX", &big, now);
            // One that says it is small.
            let mut liar = from_bea("liar@bakery.example", "Liar");
            liar.extend(std::iter::repeat_n(b'y', 2_000));
            lying = imap.deliver("INBOX", &liar, now);
        },
    )
    .await;
    m.imap
        .mailbox(|store| store.mail("INBOX", lying).stated_size = Some(100));
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 1);
    assert_eq!(subject(&changes[0]), "Small");
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.oversized").await,
        2
    );
    // The big one was never asked for; the liar's bytes were not kept.
    assert_eq!(m.imap.seen().bodies, 2);
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().bodies, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_moved_in_another_app_stays_one_message(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Bakery", &[]));
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Flour"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let first = id(&m.changes().await[0]);

    m.imap
        .mailbox(|store| store.move_mail("INBOX", uid, "Bakery"));
    m.kit.next_run().await;
    let last = m.last().await;
    assert_eq!(last.len(), 1, "{last:#?}");
    assert_eq!(last[0]["op"], "upsert");
    assert_eq!(id(&last[0]), first);
    assert_eq!(last[0]["message"]["mailbox"]["in_inbox"], false);
    assert_eq!(
        last[0]["message"]["labels"],
        serde_json::json!([{"key": "Bakery", "name": "Bakery"}])
    );
    assert_eq!(m.imap.seen().bodies, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_uidvalidity_reset_relocates_without_downloading(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "One"), now);
            imap.deliver("INBOX", &from_bea("b@bakery.example", "Two"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let batches = m.kit.batches(m.ws).await.len();
    let before: i64 = m
        .count("SELECT max(uidvalidity) FROM connectors.imap_locations")
        .await;

    m.imap.mailbox(|store| store.reset_uidvalidity("INBOX"));
    m.kit.next_run().await;
    m.kit.run_jobs().await;
    assert_eq!(m.kit.batches(m.ws).await.len(), batches, "nothing new");
    assert_eq!(m.imap.seen().bodies, 2);
    let after: i64 = m
        .count("SELECT min(uidvalidity) FROM connectors.imap_locations")
        .await;
    assert!(after > before);
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.imap_locations")
            .await,
        2
    );
    assert_eq!(m.count("SELECT count(*) FROM connectors.messages").await, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_deleted_in_another_app_is_removed(db: PgPool) {
    let mut gone = 0;
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.condstore = false,
        |imap, now| {
            gone = imap.deliver("INBOX", &from_bea("gone@bakery.example", "Gone"), now);
            imap.deliver("INBOX", &from_bea("kept@bakery.example", "Kept"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    let gone_id = id(of(&changes, "Gone"));

    m.imap.mailbox(|store| store.expunge("INBOX", gone));
    m.kit.next_run().await;
    let last = m.last().await;
    assert_eq!(
        last,
        [serde_json::json!({"op": "remove", "provider_message_id": gone_id})]
    );
    assert_eq!(m.count("SELECT count(*) FROM connectors.messages").await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_message_in_two_folders_with_or_without_a_message_id(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| {
                store.add_folder("Bakery", &[]);
                let plain = store.deliver(
                    "INBOX",
                    &raw(None, "Robot <noreply@example.net>", "No id"),
                    now,
                );
                store.copy("INBOX", plain, "Bakery");
                let with_id = store.deliver("INBOX", &from_bea("a@bakery.example", "With id"), now);
                store.copy("INBOX", with_id, "Bakery");
            });
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 2, "{changes:#?}");
    let plain = of(&changes, "No id");
    assert!(id(plain).starts_with("digest:"));
    for change in &changes {
        assert_eq!(change["message"]["mailbox"]["in_inbox"], true);
        assert_eq!(change["message"]["labels"][0]["name"], "Bakery");
    }
    assert_eq!(m.imap.seen().bodies, 2);
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.imap_locations")
            .await,
        4
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn renamed_and_removed_folders_move_and_remove_their_mail(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| {
                store.add_folder("Bakery", &[]);
                store.add_folder("Old", &[]);
            });
            imap.deliver("Bakery", &from_bea("a@bakery.example", "Kept"), now);
            imap.deliver("Old", &from_bea("b@bakery.example", "Dropped"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    let kept = id(of(&changes, "Kept"));
    let dropped = id(of(&changes, "Dropped"));

    m.imap.mailbox(|store| {
        store.rename_folder("Bakery", "Cakes");
        store.remove_folder("Old");
    });
    // The folders are listed again at most every half hour.
    m.kit.tick(TimeDelta::minutes(31)).await;
    let last = m.last().await;
    assert_eq!(last.len(), 2, "{last:#?}");
    let moved = last.iter().find(|c| c["op"] == "upsert").unwrap();
    assert_eq!(id(moved), kept);
    assert_eq!(moved["message"]["labels"][0]["name"], "Cakes");
    let removed = last.iter().find(|c| c["op"] == "remove").unwrap();
    assert_eq!(id(removed), dropped);
    assert_eq!(m.imap.seen().bodies, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_stops_at_its_byte_budget_and_the_next_continues(db: PgPool) {
    let m = mailbox(
        db,
        |config| config.run_budget_bytes = 1,
        |_| {},
        |imap, now| {
            // Part 1 is the newest (the highest UID).
            for n in (1..=3).rev() {
                imap.deliver(
                    "INBOX",
                    &from_bea(&format!("{n}@bakery.example"), &format!("Part {n}")),
                    now - TimeDelta::minutes(n),
                );
            }
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 3);
    // One download a run, newest first.
    let batches = m.kit.batches(m.ws).await;
    assert_eq!(batches.len(), 3);
    assert_eq!(subject(&changes[0]), "Part 1");
    assert_eq!(subject(&changes[2]), "Part 3");
    assert_eq!(m.imap.seen().bodies, 3);
    assert_eq!(m.kit.stream().await["phase"], "incremental");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_changed_password_needs_the_member_after_three_refusals(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;

    m.imap.set_password("changed-at-the-provider");
    m.kit.next_run().await;
    assert_eq!(m.health().await, "ok");
    assert_eq!(m.kit.stream().await["cursor"]["login_refusals"], 1);
    m.kit.next_run().await;
    assert_eq!(m.health().await, "ok");
    m.kit.next_run().await;
    assert_eq!(m.health().await, "needs_reauth");
    assert_eq!(m.kit.stream().await["paused"], true);
    let reauth = m
        .kit
        .payloads(m.ws, "connectors.connection_needs_reauth")
        .await;
    assert_eq!(reauth.len(), 1);
    assert_eq!(reauth[0]["reason"], "imap_login_refused");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_busy_server_slows_the_stream_down(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}, |_, _| {}).await;
    m.kit.run_jobs().await;
    m.imap.script(|script| script.busy = true);
    m.kit.next_run().await;
    assert_eq!(m.health().await, "throttled");
    let stream = m.kit.stream().await;
    assert!(stream["throttled_until"].is_string(), "{stream}");
    assert!(stream["cursor"].get("login_refusals").is_none());
    let reauth = m
        .kit
        .payloads(m.ws, "connectors.connection_needs_reauth")
        .await;
    assert!(reauth.is_empty());

    m.imap.script(|script| script.busy = false);
    m.kit.next_run().await;
    assert_eq!(m.health().await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_host_never_sees_more_than_its_sessions(db: PgPool) {
    let m = mailbox(
        db,
        |config| config.sessions_per_host = 4,
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    // Four sessions to the host are open elsewhere.
    let slots = imap_slots(IMAP_HOST);
    let rate = imap_rate(4);
    assert_eq!(
        schedule::take(&m.kit.db, &slots, 4, rate).await.unwrap(),
        Ok(())
    );
    let connections = m.imap.seen().connections;
    m.kit.run_jobs().await;
    assert_eq!(m.imap.seen().connections, connections, "the fifth waits");
    assert!(m.changes().await.is_empty());
    let due: DateTime<Utc> =
        serde_json::from_value(m.kit.stream().await["due_at"].clone()).unwrap();
    assert!(due > m.kit.clock.now());

    // One ends: the next dispatch runs it.
    schedule::settle(&m.kit.db, &slots, 1, 0, rate)
        .await
        .unwrap();
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().connections, connections + 1);
    assert_eq!(m.changes().await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_misbehaving_server_never_wedges_a_run(db: PgPool) {
    // Endless answers: cut short, counted as failed runs, never needs_reauth.
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.chatty = true,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    m.kit.next_run().await;
    assert!(m.changes().await.is_empty());
    let failures = m.kit.stream().await["consecutive_failures"]
        .as_i64()
        .unwrap();
    assert!(failures >= 1, "{failures}");
    assert_ne!(m.health().await, "needs_reauth");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_literal_larger_than_any_memory_is_refused(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.huge_literal = true,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    assert!(m.changes().await.is_empty());
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.oversized").await,
        1
    );
    m.kit.next_run().await;
    assert_eq!(m.imap.seen().bodies, 1, "never asked for again");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_servers_own_id_tells_apart_two_messages_with_one_message_id(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.objectid = true,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("same@bakery.example", "First"), now);
            imap.deliver("INBOX", &from_bea("same@bakery.example", "Second"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 2, "{changes:#?}");
    assert_ne!(id(&changes[0]), id(&changes[1]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn locations_are_seen_only_by_their_workspace(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let app = testing::app_role_pool(&m.kit.db).await;
    let count = |ws: WorkspaceId| {
        let app = app.clone();
        async move {
            let mut tx = app.begin_for(ws).await.unwrap();
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.imap_locations")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            tx.rollback().await.unwrap();
            n
        }
    };
    assert_eq!(count(m.ws).await, 1);
    assert_eq!(count(WorkspaceId::new()).await, 0);
    let mut tx = app.begin().await.unwrap();
    let none: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.imap_locations")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(none, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_folder_that_will_not_open_is_set_aside_not_retried_at_once(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| {
                store.add_folder("Bakery", &[]);
                store.refuse_open.push("Bakery".to_owned());
            });
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Read"), now);
            imap.deliver("Bakery", &from_bea("b@bakery.example", "Set aside"), now);
        },
    )
    .await;
    let connections = m.imap.seen().connections;
    m.kit.run_jobs().await;
    // One run: the Inbox read, the backfill over, no run straight after.
    assert_eq!(m.imap.seen().connections, connections + 1);
    let changes = m.changes().await;
    assert_eq!(changes.len(), 1);
    assert_eq!(subject(&changes[0]), "Read");
    let stream = m.kit.stream().await;
    assert_eq!(stream["phase"], "incremental");
    let bakery = stream["cursor"]["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Bakery")
        .unwrap()
        .clone();
    assert!(bakery["skipped_until"].is_string(), "{bakery}");
    assert_eq!(m.kit.pending("connectors.sync_stream").await, 0);

    // Its next runs leave it alone; an hour on, it is tried again.
    m.kit.next_run().await;
    assert_eq!(m.changes().await.len(), 1);
    m.imap.mailbox(|store| store.refuse_open.clear());
    m.kit.tick(TimeDelta::minutes(61)).await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 2, "{changes:#?}");
    assert_eq!(subject(&changes[1]), "Set aside");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_flood_of_vanished_uids_is_cut_short_and_removes_nothing(db: PgPool) {
    let mut uid = 0;
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.qresync = true,
        |imap, now| {
            uid = imap.deliver("INBOX", &from_bea("a@bakery.example", "Kept"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    m.imap
        .mailbox(|store| store.set_flags("INBOX", uid, true, false));
    m.imap.script(|script| script.offers.vanish_flood = true);
    m.kit.next_run().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 1, "{changes:#?}");
    assert_eq!(m.count("SELECT count(*) FROM connectors.messages").await, 1);
    let failures = m.kit.stream().await["consecutive_failures"]
        .as_i64()
        .unwrap();
    assert!(failures >= 1, "{failures}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reused_message_id_never_replaces_the_message_it_imitates(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |_| {},
        |imap, now| {
            imap.mailbox(|store| store.add_folder("Sent", &["\\Sent"]));
            // The member's own mail…
            imap.deliver(
                "Sent",
                &raw(
                    Some("quote-7@bakery.example"),
                    "Ana <ana@bakery.example>",
                    "Quote for the wedding cake",
                ),
                now - TimeDelta::hours(2),
            );
            // …and a message from someone else reusing its Message-ID,
            // read first (the Inbox is read first).
            imap.deliver(
                "INBOX",
                &raw(
                    Some("quote-7@bakery.example"),
                    "Mallory <mallory@example.net>",
                    "Pay this account instead",
                ),
                now - TimeDelta::hours(1),
            );
        },
    )
    .await;
    m.kit.run_jobs().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 2, "{changes:#?}");
    let own = of(&changes, "Quote for the wedding cake");
    let other = of(&changes, "Pay this account instead");
    assert_ne!(id(own), id(other));
    assert_eq!(own["message"]["direction"], "outbound");
    assert_eq!(own["message"]["from"]["address"], "ana@bakery.example");
    assert_eq!(other["message"]["from"]["address"], "mallory@example.net");
    assert_eq!(m.imap.seen().bodies, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_holds_at_most_its_share_of_a_hosts_sessions(db: PgPool) {
    let m = mailbox(
        db,
        |config| {
            config.sessions_per_host = 4;
            config.workspace_sessions_per_host = 2;
        },
        |_| {},
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    // Two sessions of this workspace are open elsewhere.
    let share = imap_workspace_slots(IMAP_HOST, m.ws);
    assert_eq!(
        schedule::take(&m.kit.db, &share, 2, imap_rate(2))
            .await
            .unwrap(),
        Ok(())
    );
    let connections = m.imap.seen().connections;
    m.kit.run_jobs().await;
    assert_eq!(m.imap.seen().connections, connections, "the third waits");
    // The host's own sessions were never taken: other workspaces go on.
    assert_eq!(
        schedule::take(&m.kit.db, &imap_slots(IMAP_HOST), 4, imap_rate(4))
            .await
            .unwrap(),
        Ok(())
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_whose_id_cannot_be_searched_is_kept(db: PgPool) {
    let mut accented = 0;
    let m = mailbox(
        db,
        |_| {},
        |script| script.offers.condstore = false,
        |imap, now| {
            accented = imap.deliver(
                "INBOX",
                &from_bea("caf\u{e9}@bakery.example", "Accented id"),
                now,
            );
            imap.deliver("INBOX", &from_bea("plain@bakery.example", "Plain id"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let batches = m.kit.batches(m.ws).await.len();

    m.imap.mailbox(|store| store.expunge("INBOX", accented));
    m.kit.next_run().await;
    // Not looked for (no search can carry it), so not removed.
    assert!(
        m.changes()
            .await
            .iter()
            .all(|change| change["op"] != "remove")
    );
    assert_eq!(m.kit.batches(m.ws).await.len(), batches);
    assert_eq!(m.count("SELECT count(*) FROM connectors.messages").await, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_download_that_keeps_timing_out_is_given_up(db: PgPool) {
    let m = mailbox(
        db,
        |config| {
            config.connect_timeout_seconds = 1;
            config.timeout_seconds = 1;
        },
        |_| {},
        |imap, now| {
            imap.deliver(
                "INBOX",
                &from_bea("fine@bakery.example", "Fine"),
                now - TimeDelta::hours(1),
            );
            let stuck = imap.deliver("INBOX", &from_bea("stuck@bakery.example", "Stuck"), now);
            imap.mailbox(|store| store.stalled.push(("INBOX".to_owned(), stuck)));
        },
    )
    .await;
    m.kit.run_jobs().await;
    for _ in 1..MAX_TIMEOUTS {
        m.kit.next_run().await;
    }
    assert!(m.changes().await.is_empty());
    m.kit.next_run().await;
    let changes = m.changes().await;
    assert_eq!(changes.len(), 1, "{changes:#?}");
    assert_eq!(subject(&changes[0]), "Fine");
    assert_eq!(
        m.count("SELECT count(*) FROM connectors.oversized").await,
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_known_providers_host_is_not_held_to_a_small_servers_cap(db: PgPool) {
    // campbooks-cloud#318: four sessions per host would starve every
    // customer of a big provider.
    let pool = testing::app_role_pool(&db).await;
    let config = ImapConfig::default();
    assert!(crate::imap::is_provider_host("imap.mail.yahoo.com"));
    assert!(!crate::imap::is_provider_host(IMAP_HOST));
    for (host, provider, runs) in [("imap.mail.yahoo.com", true, 5), (IMAP_HOST, false, 4)] {
        let mut granted = 0;
        // Five streams of five workspaces, due at once.
        for _ in 0..5 {
            let slots = run_slots(&config, host, WorkspaceId::new(), provider, (0, 0));
            if take_slots(&pool, &slots).await.unwrap() {
                granted += 1;
            }
        }
        assert_eq!(granted, runs, "{host}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn idle_sessions_count_against_a_small_hosts_sessions(db: PgPool) {
    // Two sessions to the host: IDLE may hold one, the runs keep the other.
    let m = mailbox(
        db,
        |config| {
            config.sessions_per_host = 2;
            config.idle_enabled = true;
        },
        |script| script.offers.idle = true,
        |imap, now| {
            imap.deliver("INBOX", &from_bea("a@bakery.example", "Hello"), now);
        },
    )
    .await;
    m.kit.run_jobs().await;
    let pool = crate::features::imap_idle::IdlePool::with_timing(
        m.kit.deps.clone(),
        m.kit.state.clone(),
        crate::features::imap_idle::Timing {
            refresh: std::time::Duration::from_mins(1),
            debounce: std::time::Duration::from_mins(10),
            renew: std::time::Duration::from_mins(1),
            lease: std::time::Duration::from_mins(3),
            backoff: std::time::Duration::from_millis(10),
            backoff_max: std::time::Duration::from_millis(40),
            give_up_after: 3,
            cooldown: std::time::Duration::from_mins(30),
            db_timeout: std::time::Duration::from_secs(20),
        },
    );
    let mut held = crate::features::imap_idle::IdlePool::held();
    pool.tick(&mut held).await;
    assert_eq!(held.connections().len(), 1, "IDLE holds one session");
    // A run of another workspace holds the other.
    assert_eq!(
        schedule::take(&m.kit.db, &imap_slots(IMAP_HOST), 1, imap_rate(2))
            .await
            .unwrap(),
        Ok(())
    );
    let before = m.changes().await.len();
    let now = m.kit.clock.now();
    m.imap
        .deliver("INBOX", &from_bea("b@bakery.example", "Order"), now);
    m.kit.next_run().await;
    assert_eq!(m.changes().await.len(), before, "the run waits");
    // The other run ends: the next dispatch runs it.
    schedule::settle(&m.kit.db, &imap_slots(IMAP_HOST), 1, 0, imap_rate(2))
        .await
        .unwrap();
    m.kit.next_run().await;
    assert_eq!(m.changes().await.len(), before + 1);
    pool.shutdown(held).await;
}
