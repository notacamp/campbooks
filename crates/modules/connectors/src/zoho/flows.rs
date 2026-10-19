//! The Zoho Mail adapter end to end, against the fake Zoho: data centres,
//! several mail accounts, backfill and incremental reading, archive, move
//! and delete seen without a download, members' changes written back,
//! sending (and never twice), throttling and a revoked grant.

use std::collections::BTreeSet;

use campbooks_kernel::{
    Error,
    blobs::BlobRef,
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    contract::{Address, OUTGOING_CONTENT_TYPE, OutgoingMessage},
    domain::model::{InboxId, SendId, StreamId},
    events::{ChangesRequested, SendRequested},
    testkit::{FIXTURES, Kit, assert_contract, synthetic},
    testkit_zoho::{CLIENTS, DRAFTS, INBOX, SENT, SNOOZED, SPAM, TRASH},
};

/// A workspace with Sam's Zoho login connected and its first sync done.
struct Connected {
    ws: WorkspaceId,
    connection: Value,
}

async fn connected(kit: &Kit) -> Connected {
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    let connection = kit.connect_zoho(&sam, ws).await;
    kit.run_jobs().await;
    Connected { ws, connection }
}

impl Connected {
    fn inbox(&self) -> InboxId {
        serde_json::from_value(self.connection["streams"][0]["inbox_id"].clone()).unwrap()
    }

    async fn stream(&self, kit: &Kit) -> StreamId {
        let mut tx = kit.state.db.begin_for(self.ws).await.unwrap();
        let id = sqlx::query_scalar!(
            r#"SELECT id AS "id: StreamId" FROM connectors.streams WHERE inbox_id = $1"#,
            self.inbox() as InboxId
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        id
    }

    /// Run the stream now.
    async fn run(&self, kit: &Kit) {
        let stream = self.stream(kit).await;
        let mut tx = kit.state.db.begin_for(self.ws).await.unwrap();
        crate::domain::schedule::run_now(&mut tx, self.ws, stream, kit.clock.now())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
    }

    /// conversations asks for `items` (message id, change).
    async fn request(&self, kit: &Kit, items: &[(&str, Value)]) {
        let conversation = uuid::Uuid::now_v7();
        let ids: Vec<&str> = items.iter().map(|(id, _)| *id).collect();
        kit.outbound.show(conversation, &ids);
        let payload = json!({
            "workspace": self.ws.to_string(),
            "inbox_id": self.inbox(),
            "conversation": conversation,
            "origin": "user",
            "items": items.iter().map(|(id, change)| json!({
                "provider_message_id": id, "change": change
            })).collect::<Vec<_>>(),
        });
        assert_contract("conversations.changes_requested.v1.schema.json", &payload);
        let event: ChangesRequested = serde_json::from_value(payload).unwrap();
        let mut tx = kit.state.db.begin_for(self.ws).await.unwrap();
        kit.state.events.publish(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
    }
}

/// The upserts and removals of every batch so far, in order.
async fn changes(kit: &Kit, ws: WorkspaceId) -> Vec<Value> {
    kit.batches(ws)
        .await
        .into_iter()
        .flat_map(|batch| batch["changes"].as_array().cloned().unwrap_or_default())
        .collect()
}

fn upsert_of<'a>(changes: &'a [Value], id: &str) -> Vec<&'a Value> {
    changes
        .iter()
        .filter(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
        .collect()
}

fn fixture(name: &str) -> &'static [u8] {
    FIXTURES.iter().find(|(n, _)| *n == name).unwrap().1
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_zoho_login_connects_each_mail_account_in_its_data_centre(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.zoho.add_account("billing@example.net");
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    let connection = kit.connect_zoho(&sam, ws).await;
    assert_eq!(connection["provider"], "zoho");
    let addresses: BTreeSet<&str> = connection["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["address"].as_str().unwrap())
        .collect();
    assert_eq!(
        addresses,
        BTreeSet::from(["sam@example.net", "billing@example.net"])
    );
    // One inbox per mail account, each announced with its address.
    let granted = kit.payloads(ws, "connectors.capability_granted").await;
    assert_eq!(granted.len(), 2);
    let mut announced = BTreeSet::new();
    for event in &granted {
        assert_contract("connectors.capability_granted.v1.schema.json", event);
        let blob: BlobRef = serde_json::from_value(event["account"].clone()).unwrap();
        let account: Value = serde_json::from_slice(&kit.blob(ws, &blob).await).unwrap();
        assert_contract("connection-account.v1.schema.json", &account);
        assert_eq!(account["provider"], "zoho");
        announced.insert(account["address"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        announced,
        BTreeSet::from([
            "sam@example.net".to_owned(),
            "billing@example.net".to_owned()
        ])
    );
    // Its tokens and mail live in its data centre: the fake's.
    let server: Option<String> =
        sqlx::query_scalar("SELECT provider_server FROM connectors.connections")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(server.as_deref(), Some(kit.zoho.issuer().as_str()));

    // The same login again (a reconnect) adds nothing new …
    kit.adopt_zoho(&sam, ws, "zoho-zuid-7001", &kit.zoho.issuer())
        .await
        .unwrap();
    assert_eq!(
        kit.payloads(ws, "connectors.capability_granted")
            .await
            .len(),
        2
    );
    // … unless the login gained a mail account meanwhile.
    kit.zoho.add_account("hello@example.net");
    kit.adopt_zoho(&sam, ws, "zoho-zuid-7001", &kit.zoho.issuer())
        .await
        .unwrap();
    assert_eq!(
        kit.payloads(ws, "connectors.capability_granted")
            .await
            .len(),
        3
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_zoho_grant_from_a_made_up_data_centre_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    for issuer in ["https://accounts.evil.example", "https://accounts.zoho.eu"] {
        // The real EU centre is Zoho's, but this instance talks to the fake.
        let refused = kit.adopt_zoho(&sam, ws, "zoho-zuid-7001", issuer).await;
        assert!(
            matches!(&refused, Err(Error::Conflict(reason)) if reason.code() == "unknown_data_centre"),
            "{refused:?}"
        );
    }
    let connections: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.connections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(connections, 0);
    assert!(kit.zoho.calls().is_empty(), "no request went anywhere");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn backfill_then_only_what_is_new(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let invoice = kit.zoho.deliver(fixture("en-invoice"), INBOX, false, at(5));
    let newsletter = kit
        .zoho
        .deliver(fixture("es-ES-newsletter"), INBOX, true, at(4));
    let sent = kit.zoho.deliver(&synthetic(3), SENT, true, at(3));
    let filed = kit.zoho.deliver(&synthetic(4), CLIENTS, true, at(2));
    let draft = kit.zoho.deliver(&synthetic(5), DRAFTS, true, at(1));
    // Older than the window: never read.
    let old = kit.zoho.deliver(&synthetic(6), INBOX, true, at(24 * 200));
    let mailbox = connected(&kit).await;

    let all = changes(&kit, mailbox.ws).await;
    for batch in kit.batches(mailbox.ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
    }
    let one = |id: &str| upsert_of(&all, id)[0]["message"].clone();
    let invoice_msg = one(&invoice);
    assert_eq!(invoice_msg["mailbox"]["in_inbox"], true);
    assert_eq!(invoice_msg["read"], false);
    assert_eq!(invoice_msg["thread_key"], Value::Null);
    assert_eq!(invoice_msg["subject"], "Invoice INV-2041 for September");
    // Bulk signals come from the raw message.
    let news = one(&newsletter);
    assert_eq!(
        news["headers"]["list_unsubscribe"],
        "<https://cafesol.example.com/baja>"
    );
    assert_eq!(news["headers"]["precedence"], "bulk");
    assert_eq!(one(&sent)["direction"], "outbound");
    assert_eq!(one(&filed)["mailbox"]["in_inbox"], false);
    assert!(upsert_of(&all, &draft).is_empty() && upsert_of(&all, &old).is_empty());
    assert_eq!(kit.stream().await["phase"], "incremental");

    // Something arrives: only it comes, and nothing is downloaded twice.
    let fresh = kit
        .zoho
        .deliver(&synthetic(7), INBOX, false, kit.clock.now());
    let before = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    let after = changes(&kit, mailbox.ws).await;
    assert_eq!(after.len(), before + 1);
    assert_eq!(
        after[before]["message"]["provider_message_id"],
        fresh.as_str()
    );
    assert_eq!(kit.zoho.raw_fetches(&invoice), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn archive_move_and_delete_in_zoho_are_seen_without_a_download(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let archived = kit.zoho.deliver(&synthetic(1), INBOX, true, at(3));
    let moved = kit.zoho.deliver(&synthetic(2), INBOX, true, at(2));
    let trashed = kit.zoho.deliver(&synthetic(3), INBOX, true, at(1));
    let snoozed = kit.zoho.deliver(&synthetic(4), INBOX, true, at(1));
    let mailbox = connected(&kit).await;
    let start = changes(&kit, mailbox.ws).await.len();

    kit.zoho.change(&archived, |m| m.archived = true);
    kit.zoho.change(&moved, |m| m.folder = CLIENTS.to_owned());
    // Into a folder the adapter does not read: its Inbox no longer has it,
    // so it is looked for elsewhere, and found, not removed.
    kit.zoho.change(&snoozed, |m| m.folder = SNOOZED.to_owned());
    kit.zoho.change(&trashed, |m| {
        m.folder = TRASH.to_owned();
        m.flag = "important".to_owned();
    });
    mailbox.run(&kit).await;
    let now = changes(&kit, mailbox.ws).await;
    let latest = |id: &str| upsert_of(&now[start..], id).last().unwrap()["message"].clone();
    assert_eq!(latest(&archived)["mailbox"]["in_inbox"], false);
    assert_eq!(latest(&moved)["mailbox"]["in_inbox"], false);
    assert_eq!(latest(&snoozed)["mailbox"]["in_inbox"], false);
    assert!(
        !now[start..]
            .iter()
            .any(|c| c["op"] == "remove" && c["provider_message_id"] == snoozed.as_str()),
        "a moved message is not removed"
    );
    let in_trash = latest(&trashed);
    assert_eq!(in_trash["mailbox"]["in_trash"], true);
    assert_eq!(in_trash["starred"], true);
    for id in [&archived, &moved, &trashed] {
        assert_eq!(kit.zoho.raw_fetches(id), 1, "downloaded once");
    }

    // Unarchived in Zoho: back in the inbox. Emptied from the trash: gone.
    kit.zoho.change(&archived, |m| m.archived = false);
    kit.zoho.purge(&trashed);
    let mark = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    let now = changes(&kit, mailbox.ws).await;
    assert_eq!(
        upsert_of(&now[mark..], &archived)[0]["message"]["mailbox"]["in_inbox"],
        true
    );
    assert!(
        now[mark..]
            .iter()
            .any(|c| c["op"] == "remove" && c["provider_message_id"] == trashed.as_str())
    );
    // Nothing changed since: the next run emits nothing.
    let quiet = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    assert_eq!(changes(&kit, mailbox.ws).await.len(), quiet);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn members_changes_reach_zoho_and_never_echo(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let one = kit.zoho.deliver(&synthetic(1), INBOX, false, at(3));
    let two = kit.zoho.deliver(&synthetic(2), CLIENTS, true, at(2));
    let mailbox = connected(&kit).await;
    let before = changes(&kit, mailbox.ws).await.len();

    // One intent per message per request, as conversations sends them.
    for (first, second) in [
        (json!({"kind": "mark_read"}), json!({"kind": "unarchive"})),
        (
            json!({"kind": "star"}),
            json!({"kind": "add_label", "label": uuid::Uuid::nil(), "provider_key": "1"}),
        ),
        (
            json!({"kind": "archive"}),
            json!({"kind": "move_to_stream", "stream": "newsletters"}),
        ),
    ] {
        mailbox
            .request(&kit, &[(one.as_str(), first), (two.as_str(), second)])
            .await;
    }
    let first = kit.zoho.message(&one);
    assert!(first.read && first.archived && first.flag == "important");
    // Unarchiving a message filed elsewhere brings it back to the Inbox.
    assert_eq!(kit.zoho.message(&two).folder, INBOX);
    let ends: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT state, reason FROM connectors.outbound_changes ORDER BY id")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        ends,
        [
            ("applied".to_owned(), None),
            ("applied".to_owned(), None),
            ("applied".to_owned(), None),
            (
                "skipped".to_owned(),
                Some("label_not_at_provider".to_owned())
            ),
            ("applied".to_owned(), None),
            // A move to Newsletters: its stream mark, made first.
            ("applied".to_owned(), None),
        ]
    );
    let mark = kit.zoho.label_named("Campbooks/Newsletters").unwrap();
    assert_eq!(kit.zoho.message(&two).labels, [mark]);
    let writes: Vec<String> = kit
        .zoho
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("PUT"))
        .collect();
    assert!(
        writes.iter().any(|w| w.ends_with("archiveMails")),
        "{writes:?}"
    );
    assert!(
        writes.iter().any(|w| w.ends_with("moveMessage")),
        "{writes:?}"
    );

    // The write comes back as nothing.
    mailbox.run(&kit).await;
    assert_eq!(changes(&kit, mailbox.ws).await.len(), before);
}

/// The change for `kind`, with the direction conversations sends for
/// untrash and not spam (mail the inbox received).
fn bin_change(kind: &str) -> Value {
    match kind {
        "untrash" | "unspam" => json!({ "kind": kind, "inbound": true }),
        _ => json!({ "kind": kind }),
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_and_spam_reach_zoho_and_come_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let one = kit.zoho.deliver(&synthetic(1), INBOX, false, at(3));
    let filed = kit.zoho.deliver(&synthetic(2), CLIENTS, true, at(4));
    let flagged = kit.zoho.deliver(&synthetic(3), SPAM, false, at(5));
    let mailbox = connected(&kit).await;
    let before = changes(&kit, mailbox.ws).await.len();
    let last_end = || async {
        sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT kind, state, reason FROM connectors.outbound_changes ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&kit.db)
        .await
        .unwrap()
    };

    for (id, kind, folder) in [
        (&one, "trash", TRASH),
        (&one, "untrash", INBOX),
        (&one, "spam", SPAM),
        (&one, "unspam", INBOX),
        // Out of the trash, back to the member's own folder.
        (&filed, "trash", TRASH),
        (&filed, "untrash", CLIENTS),
        // Spam Zoho flagged itself (nothing remembered): to the Inbox.
        (&flagged, "unspam", INBOX),
        // Trashed from spam: untrash brings it to the Inbox, not to spam.
        (&flagged, "spam", SPAM),
        (&flagged, "trash", TRASH),
        (&flagged, "untrash", INBOX),
    ] {
        mailbox
            .request(&kit, &[(id.as_str(), bin_change(kind))])
            .await;
        assert_eq!(kit.zoho.message(id).folder, folder, "{kind}");
        assert_eq!(
            last_end().await,
            (kind.to_owned(), "applied".to_owned(), None),
            "{kind}"
        );
        // The write comes back as nothing.
        mailbox.run(&kit).await;
        assert_eq!(changes(&kit, mailbox.ws).await.len(), before, "{kind}");
    }
    let calls = kit.zoho.calls();
    let count = |mode: &str| {
        calls
            .iter()
            .filter(|c| c.starts_with("PUT") && c.ends_with(mode))
            .count()
    };
    // Spam is Zoho's report; the trash and every way back are moves.
    assert_eq!(count("moveToSpam"), 2);
    assert_eq!(count("moveMessage"), 8);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn untrash_forgets_a_place_once_the_message_left_the_trash(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let filed = kit.zoho.deliver(&synthetic(2), CLIENTS, true, at(4));
    let mailbox = connected(&kit).await;
    mailbox
        .request(&kit, &[(filed.as_str(), bin_change("trash"))])
        .await;
    assert_eq!(kit.zoho.message(&filed).folder, TRASH);
    // At Zoho, moved out of the trash and back in: the place remembered
    // when Campbooks trashed it no longer holds.
    kit.zoho.change(&filed, |m| m.folder = INBOX.to_owned());
    mailbox.run(&kit).await;
    kit.zoho.change(&filed, |m| m.folder = TRASH.to_owned());
    mailbox.run(&kit).await;
    mailbox
        .request(&kit, &[(filed.as_str(), bin_change("untrash"))])
        .await;
    assert_eq!(kit.zoho.message(&filed).folder, INBOX);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sent_mail_out_of_spam_is_never_put_in_the_inbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let sent = kit.zoho.deliver(&synthetic(4), SENT, true, at(2));
    let mailbox = connected(&kit).await;
    mailbox
        .request(&kit, &[(sent.as_str(), bin_change("spam"))])
        .await;
    assert_eq!(kit.zoho.message(&sent).folder, SPAM);
    mailbox
        .request(
            &kit,
            &[(sent.as_str(), json!({ "kind": "unspam", "inbound": false }))],
        )
        .await;
    assert_eq!(kit.zoho.message(&sent).folder, SPAM);
    let end: (String, Option<String>) = sqlx::query_as(
        "SELECT state, reason FROM connectors.outbound_changes ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(end, ("skipped".to_owned(), Some("not_movable".to_owned())));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_trash_folder_zoho_did_not_list_is_never_guessed(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let one = kit.zoho.deliver(&synthetic(1), INBOX, false, at(3));
    kit.zoho.hide_trash_folder(true);
    let mailbox = connected(&kit).await;

    mailbox
        .request(&kit, &[(one.as_str(), json!({ "kind": "trash" }))])
        .await;
    assert_eq!(kit.zoho.message(&one).folder, INBOX);
    let end: (String, Option<String>) =
        sqlx::query_as("SELECT state, reason FROM connectors.outbound_changes")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(end, ("skipped".to_owned(), Some("not_movable".to_owned())));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_zoho_refuses_fails_and_the_others_are_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let one = kit.zoho.deliver(&synthetic(1), INBOX, false, at(2));
    let two = kit.zoho.deliver(&synthetic(2), INBOX, false, at(1));
    let mailbox = connected(&kit).await;
    kit.zoho.refuse_changes(&two);
    let before = changes(&kit, mailbox.ws).await.len();
    mailbox
        .request(
            &kit,
            &[
                (one.as_str(), json!({"kind": "mark_read"})),
                (two.as_str(), json!({"kind": "mark_read"})),
            ],
        )
        .await;
    assert!(kit.zoho.message(&one).read);
    assert!(!kit.zoho.message(&two).read);
    let ends: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT state, reason FROM connectors.outbound_changes ORDER BY id")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        ends,
        [
            ("applied".to_owned(), None),
            ("failed".to_owned(), Some("refused".to_owned())),
        ]
    );
    // conversations is told Zoho's state of the refused one.
    mailbox.run(&kit).await;
    let now = changes(&kit, mailbox.ws).await;
    let told = upsert_of(&now[before..], &two);
    assert_eq!(told.last().unwrap()["message"]["read"], false);
    assert!(upsert_of(&now[before..], &one).is_empty());
}

/// A reply to `parent`'s Message-ID from Sam's Zoho inbox: the send id.
async fn reply(kit: &Kit, mailbox: &Connected, parent: &str) -> SendId {
    let send_id = SendId::new();
    let document = OutgoingMessage {
        version: 1,
        inbox: mailbox.inbox(),
        rfc_message_id: format!("{send_id}@example.com"),
        from: Address {
            name: Some("Sam Rivera".into()),
            address: "sam@example.net".into(),
        },
        to: vec![Address {
            name: Some("Sender 1".into()),
            address: "sender1@example.com".into(),
        }],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "Re: Message 1".into(),
        date: kit.clock.now(),
        in_reply_to: Some(parent.to_owned()),
        references: vec![parent.to_owned()],
        thread_key: None,
        text: "Thanks, see you Thursday.".into(),
        html: Some("<p>Thanks, see you Thursday.</p>".into()),
        attachments: Vec::new(),
    };
    let mut value = serde_json::to_value(&document).unwrap();
    value["send_id"] = send_id.to_string().into();
    assert_contract("outgoing-message.v1.schema.json", &value);
    let blob = kit
        .state
        .blobs
        .put(
            &mailbox.ws.to_string(),
            serde_json::to_vec(&value).unwrap(),
            OUTGOING_CONTENT_TYPE,
        )
        .await
        .unwrap();
    kit.outbound.release(send_id, mailbox.inbox(), blob.clone());
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &mailbox.ws.to_string())
        .await
        .unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &SendRequested {
                workspace: mailbox.ws.to_string(),
                inbox_id: mailbox.inbox(),
                send_id,
                message: blob,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    send_id
}

/// Run the jobs, then (minutes later) the sends waiting to retry.
async fn settle_sends(kit: &Kit) {
    for _ in 0..12 {
        kit.run_jobs().await;
        kit.clock.advance(TimeDelta::minutes(2));
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

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_goes_out_as_a_zoho_reply_and_comes_back_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let parent = kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        false,
        kit.clock.now() - TimeDelta::hours(1),
    );
    let mailbox = connected(&kit).await;
    let before = changes(&kit, mailbox.ws).await.len();
    let send = reply(&kit, &mailbox, "m1@example.com").await;
    settle_sends(&kit).await;

    let sends = kit.zoho.sends();
    assert_eq!(sends.len(), 1, "one send");
    assert_eq!(
        sends[0].0.as_deref(),
        Some(parent.as_str()),
        "a reply to it"
    );
    assert_eq!(sends[0].1["toAddress"], "sender1@example.com");
    assert_eq!(sends[0].1["mailFormat"], "html");
    let completed = kit.payloads(mailbox.ws, "connectors.send_completed").await;
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0]["send_id"], send.to_string());
    assert_eq!(completed[0]["outcome"], "sent");
    let sent_id = completed[0]["provider_message_id"]
        .as_str()
        .unwrap()
        .to_owned();
    // The sent copy is announced at once, threaded by Zoho's headers …
    let now = changes(&kit, mailbox.ws).await;
    let copies = upsert_of(&now[before..], &sent_id);
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0]["message"]["in_reply_to"], "m1@example.com");
    assert_eq!(copies[0]["message"]["direction"], "outbound");
    // … and the next sync does not bring it again.
    let mark = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    assert!(upsert_of(&changes(&kit, mailbox.ws).await[mark..], &sent_id).is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lost_answer_is_found_in_the_sent_folder_and_never_sent_twice(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        false,
        kit.clock.now() - TimeDelta::hours(1),
    );
    let mailbox = connected(&kit).await;
    kit.zoho.lose_next_send_answer();
    reply(&kit, &mailbox, "m1@example.com").await;
    settle_sends(&kit).await;
    assert_eq!(kit.zoho.sends().len(), 1, "Zoho received one send");
    assert_eq!(kit.zoho.sent().len(), 1);
    let completed = kit.payloads(mailbox.ws, "connectors.send_completed").await;
    assert_eq!(completed[0]["outcome"], "sent");

    // A refusal fails the send at once.
    kit.zoho.refuse_next_send(400);
    reply(&kit, &mailbox, "m1@example.com").await;
    settle_sends(&kit).await;
    let completed = kit.payloads(mailbox.ws, "connectors.send_completed").await;
    assert_eq!(completed[1]["outcome"], "failed");
    assert_eq!(completed[1]["reason"], "rejected");
    assert_eq!(kit.zoho.sends().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttled_by_zoho_then_a_revoked_grant(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        false,
        kit.clock.now() - TimeDelta::hours(1),
    );
    let mailbox = connected(&kit).await;
    kit.zoho.rate_limit_next(120);
    mailbox.run(&kit).await;
    let connection: (String, Option<String>) =
        sqlx::query_as("SELECT health, health_reason FROM connectors.connections")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        connection,
        ("throttled".to_owned(), Some("rate_limited".to_owned()))
    );

    // Zoho refuses the refresh (with a 200 `invalid_code`): the member must
    // connect again, once.
    kit.zoho.revoke_grant();
    kit.clock.advance(TimeDelta::hours(2));
    mailbox.run(&kit).await;
    let health: String = sqlx::query_scalar("SELECT health FROM connectors.connections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(health, "needs_reauth");
    assert_eq!(
        kit.payloads(mailbox.ws, "connectors.connection_needs_reauth")
            .await
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn disconnecting_revokes_at_the_data_centre(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = connected(&kit).await;
    let sam = kit.user("sam");
    let id = mailbox.connection["id"].as_str().unwrap();
    let gone = kit
        .delete(&sam, &format!("/api/v1/w/{}/connections/{id}", mailbox.ws))
        .await;
    assert_eq!(gone.status.as_u16(), 204, "{:?}", gone.json);
    assert_eq!(kit.zoho.revoked().len(), 1);
    assert!(kit.zoho.revoked()[0].starts_with("zoho-refresh-"));
    let budgets: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connectors.provider_budgets WHERE provider LIKE 'zoho:%'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(budgets, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn unarchiving_a_conversation_never_moves_sent_mail_into_the_inbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let theirs = kit.zoho.deliver(&synthetic(1), INBOX, true, at(3));
    let mine = kit.zoho.deliver(&synthetic(2), SENT, true, at(2));
    let junk = kit.zoho.deliver(&synthetic(3), SPAM, true, at(1));
    let mailbox = connected(&kit).await;
    mailbox
        .request(&kit, &[(theirs.as_str(), json!({"kind": "archive"}))])
        .await;
    assert!(kit.zoho.message(&theirs).archived);
    kit.zoho.clear();
    // Unarchive covers every message of the conversation, the member's own
    // reply too.
    mailbox
        .request(
            &kit,
            &[
                (theirs.as_str(), json!({"kind": "unarchive"})),
                (mine.as_str(), json!({"kind": "unarchive"})),
                (junk.as_str(), json!({"kind": "unarchive"})),
            ],
        )
        .await;
    assert!(!kit.zoho.message(&theirs).archived);
    assert_eq!(
        kit.zoho.message(&mine).folder,
        SENT,
        "sent mail stays in Sent"
    );
    assert_eq!(kit.zoho.message(&junk).folder, SPAM);
    let writes: Vec<String> = kit
        .zoho
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("PUT"))
        .collect();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert!(writes[0].ends_with("unArchiveMails"), "{writes:?}");
    let ends: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT state, reason FROM connectors.outbound_changes ORDER BY id OFFSET 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(
        ends,
        [
            ("applied".to_owned(), None),
            ("skipped".to_owned(), Some("not_movable".to_owned())),
            ("skipped".to_owned(), Some("in_trash".to_owned())),
        ]
    );
}

/// A reply whose send is under way: run its first attempt only.
async fn first_attempt(kit: &Kit, mailbox: &Connected) -> SendId {
    let send = reply(kit, mailbox, "m1@example.com").await;
    kit.run_jobs().await;
    send
}

/// The outcome the last `send_completed` of `mailbox` reported.
async fn outcome(kit: &Kit, mailbox: &Connected) -> Option<Value> {
    kit.payloads(mailbox.ws, "connectors.send_completed")
        .await
        .last()
        .cloned()
}

/// Let the sends' retries come due and run them (`rounds` times).
async fn retry_sends(kit: &Kit, rounds: usize) {
    for _ in 0..rounds {
        kit.clock.advance(TimeDelta::minutes(2));
        sqlx::query!(
            r#"UPDATE kernel.jobs SET run_at = now()
                WHERE state = 'pending' AND kind = 'connectors.send_message'
                  AND payload->>'give_up' = 'false'"#
        )
        .execute(&kit.db)
        .await
        .unwrap();
        kit.run_jobs().await;
    }
}

async fn with_parent(kit: &Kit) -> Connected {
    kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        false,
        kit.clock.now() - TimeDelta::hours(1),
    );
    connected(kit).await
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_same_subject_message_minutes_later_is_not_taken_for_the_send(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = with_parent(&kit).await;
    kit.zoho.lose_next_send();
    first_attempt(&kit, &mailbox).await;
    assert!(
        kit.zoho.sends().is_empty(),
        "the attempt never reached Zoho"
    );
    // Two minutes later the member sends a follow-up from another client:
    // same subject and recipient, other words.
    let later = kit.clock.now() + TimeDelta::minutes(2);
    kit.zoho.deliver(
        b"From: sam@example.net\r\nTo: sender1@example.com\r\nSubject: Re: Message 1\r\n\r\nOne more thing: the address.\r\n",
        SENT,
        true,
        later,
    );
    retry_sends(&kit, 3).await;
    // Not mistaken for the send: it was sent, once, as itself.
    assert_eq!(kit.zoho.sends().len(), 1);
    let done = outcome(&kit, &mailbox).await.unwrap();
    assert_eq!(done["outcome"], "sent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_missing_sent_folder_proves_nothing_and_the_send_waits(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = with_parent(&kit).await;
    kit.zoho.lose_next_send_answer();
    kit.zoho.hide_sent_folder(true);
    first_attempt(&kit, &mailbox).await;
    retry_sends(&kit, 2).await;
    assert_eq!(kit.zoho.sends().len(), 1, "not sent again");
    assert!(outcome(&kit, &mailbox).await.is_none(), "still unclear");
    kit.zoho.hide_sent_folder(false);
    retry_sends(&kit, 2).await;
    assert_eq!(kit.zoho.sends().len(), 1);
    assert_eq!(outcome(&kit, &mailbox).await.unwrap()["outcome"], "sent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_queued_in_the_outbox_counts_as_sent(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = with_parent(&kit).await;
    kit.zoho.queue_next_send();
    first_attempt(&kit, &mailbox).await;
    retry_sends(&kit, 2).await;
    assert_eq!(kit.zoho.sends().len(), 1);
    assert_eq!(kit.zoho.sent().len(), 0, "still in the Outbox");
    assert_eq!(outcome(&kit, &mailbox).await.unwrap()["outcome"], "sent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_matching_sent_messages_leave_the_send_unclear(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = with_parent(&kit).await;
    kit.zoho.lose_next_send_answer();
    first_attempt(&kit, &mailbox).await;
    // A second, identical copy at the same moment: which one is ours cannot
    // be told.
    let copy = kit.zoho.sent()[0].clone();
    kit.zoho.deliver(&copy.raw, SENT, true, copy.received);
    retry_sends(&kit, 12).await;
    assert_eq!(kit.zoho.sends().len(), 1, "never sent twice");
    let done = outcome(&kit, &mailbox).await.unwrap();
    assert_eq!(done["outcome"], "failed");
    assert_eq!(done["reason"], "unknown_outcome");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_answer_without_an_id_claims_the_sent_copy(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = with_parent(&kit).await;
    kit.zoho.answer_next_send_without_id();
    first_attempt(&kit, &mailbox).await;
    let done = outcome(&kit, &mailbox).await.unwrap();
    assert_eq!(done["outcome"], "sent");
    assert_eq!(done["provider_message_id"], kit.zoho.sent()[0].id.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_zoho_fails_on_is_skipped_and_tried_again_later(db: PgPool) {
    let kit = Kit::new(db).await;
    let at = |hours: i64| kit.clock.now() - TimeDelta::hours(hours);
    let good = kit.zoho.deliver(&synthetic(1), INBOX, false, at(2));
    let bad = kit.zoho.deliver(&synthetic(2), INBOX, false, at(1));
    kit.zoho.fail_downloads_of(&bad, true);
    let mailbox = connected(&kit).await;
    let all = changes(&kit, mailbox.ws).await;
    assert_eq!(
        upsert_of(&all, &good).len(),
        1,
        "the rest of the mailbox syncs"
    );
    assert!(upsert_of(&all, &bad).is_empty());
    assert_eq!(kit.stream().await["phase"], "incremental");
    let skipped: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.skipped_messages")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(skipped, 1);
    // Not tried again before its time.
    let fetches = kit.zoho.raw_fetches(&bad);
    mailbox.run(&kit).await;
    assert_eq!(kit.zoho.raw_fetches(&bad), fetches);
    // Later, Zoho serves it.
    kit.zoho.fail_downloads_of(&bad, false);
    kit.clock.advance(TimeDelta::minutes(5));
    mailbox.run(&kit).await;
    assert_eq!(upsert_of(&changes(&kit, mailbox.ws).await, &bad).len(), 1);
}

/// Every outbound change's end, as (kind, state, reason), oldest first.
async fn outbound_ends(kit: &Kit) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as("SELECT kind, state, reason FROM connectors.outbound_changes ORDER BY id")
        .fetch_all(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn zoho_labels_and_stream_marks_are_read_without_a_download(db: PgPool) {
    let kit = Kit::new(db).await;
    let clients = kit.zoho.add_label("Clients");
    let message = kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        true,
        kit.clock.now() - TimeDelta::hours(1),
    );
    let mailbox = connected(&kit).await;
    let all = changes(&kit, mailbox.ws).await;
    let first = &upsert_of(&all, &message)[0]["message"];
    assert_eq!(first["labels"], json!([]));
    // No mark: Zoho's mark-free inbox is Primary.
    assert_eq!(first["provider_stream"], "primary");

    // Labelled in Zoho.
    kit.zoho
        .change(&message, |m| m.labels.push(clients.clone()));
    let start = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    let now = changes(&kit, mailbox.ws).await;
    let labelled = &upsert_of(&now[start..], &message)[0]["message"];
    assert_eq!(
        labelled["labels"],
        json!([{"key": clients.clone(), "name": "Clients"}])
    );

    // A member adds `Campbooks/Newsletters` in Zoho (a label made after the
    // last listing of the account's labels).
    let mark = kit.zoho.add_label("Campbooks/Newsletters");
    kit.zoho.change(&message, |m| m.labels.push(mark.clone()));
    let start = changes(&kit, mailbox.ws).await.len();
    mailbox.run(&kit).await;
    let now = changes(&kit, mailbox.ws).await;
    let marked = &upsert_of(&now[start..], &message)[0]["message"];
    assert_eq!(marked["provider_stream"], "newsletters");
    assert_eq!(
        marked["labels"],
        json!([{"key": clients, "name": "Clients"}]),
        "a mark is never a label"
    );
    for batch in kit.batches(mailbox.ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
    }
    assert_eq!(kit.zoho.raw_fetches(&message), 1, "downloaded once");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn stream_moves_and_labels_reach_zoho_and_never_echo(db: PgPool) {
    let kit = Kit::new(db).await;
    let clients = kit.zoho.add_label("Clients");
    let message = kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        true,
        kit.clock.now() - TimeDelta::hours(1),
    );
    kit.zoho
        .change(&message, |m| m.labels.push(clients.clone()));
    let mailbox = connected(&kit).await;
    let before = changes(&kit, mailbox.ws).await.len();
    let named = |name: &str| kit.zoho.label_named(name).unwrap();

    let moved = |stream: &str| json!({"kind": "move_to_stream", "stream": stream});
    mailbox
        .request(&kit, &[(message.as_str(), moved("updates"))])
        .await;
    assert_eq!(
        kit.zoho.message(&message).labels,
        [clients.clone(), named("Campbooks/Updates")]
    );
    // Updates to Receipts: the Updates mark off, the Receipts mark (made
    // first) on.
    mailbox
        .request(&kit, &[(message.as_str(), moved("receipts"))])
        .await;
    assert_eq!(
        kit.zoho.message(&message).labels,
        [clients.clone(), named("Campbooks/Receipts")]
    );
    // Already in Receipts: nothing to call.
    kit.zoho.clear();
    mailbox
        .request(&kit, &[(message.as_str(), moved("receipts"))])
        .await;
    assert!(
        !kit.zoho.calls().iter().any(|c| c.starts_with("PUT")),
        "{:?}",
        kit.zoho.calls()
    );
    // A Zoho label taken off.
    mailbox
        .request(
            &kit,
            &[(
                message.as_str(),
                json!({"kind": "remove_label", "label": uuid::Uuid::nil(), "provider_key": clients}),
            )],
        )
        .await;
    assert!(
        kit.zoho.calls().iter().any(|c| c.ends_with("removeLabel")),
        "{:?}",
        kit.zoho.calls()
    );
    assert_eq!(
        kit.zoho.message(&message).labels,
        [named("Campbooks/Receipts")]
    );
    // Back to Primary: no mark.
    mailbox
        .request(&kit, &[(message.as_str(), moved("primary"))])
        .await;
    assert!(kit.zoho.message(&message).labels.is_empty());
    let ends: Vec<(String, Option<String>)> = outbound_ends(&kit)
        .await
        .into_iter()
        .map(|(_, state, reason)| (state, reason))
        .collect();
    assert_eq!(
        ends,
        [
            ("applied".to_owned(), None),
            ("applied".to_owned(), None),
            ("skipped".to_owned(), Some("already_set".to_owned())),
            ("applied".to_owned(), None),
            ("applied".to_owned(), None),
        ]
    );

    // The writes come back as nothing.
    mailbox.run(&kit).await;
    mailbox.run(&kit).await;
    assert_eq!(changes(&kit, mailbox.ws).await.len(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_zoho_grant_without_the_labels_scope_syncs_the_rest(db: PgPool) {
    let kit = Kit::new(db).await;
    let clients = kit.zoho.add_label("Clients");
    let message = kit.zoho.deliver(
        &synthetic(1),
        INBOX,
        false,
        kit.clock.now() - TimeDelta::hours(1),
    );
    kit.zoho
        .change(&message, |m| m.labels.push(clients.clone()));
    // Granted before the labels scope was asked for.
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    let connection = kit.connect_zoho(&sam, ws).await;
    sqlx::query("UPDATE connectors.connections SET scopes = replace(scopes, $1, '')")
        .bind(crate::zoho::LABELS_SCOPE)
        .execute(&kit.db)
        .await
        .unwrap();
    kit.zoho.refuse_labels();
    kit.run_jobs().await;
    let mailbox = Connected { ws, connection };

    let all = changes(&kit, ws).await;
    let first = &upsert_of(&all, &message)[0]["message"];
    assert_eq!(first["labels"], json!([]));
    assert!(first.get("provider_stream").is_none(), "{first}");

    mailbox
        .request(
            &kit,
            &[(
                message.as_str(),
                json!({"kind": "add_label", "label": uuid::Uuid::nil(), "provider_key": clients}),
            )],
        )
        .await;
    mailbox
        .request(
            &kit,
            &[(
                message.as_str(),
                json!({"kind": "move_to_stream", "stream": "newsletters"}),
            )],
        )
        .await;
    mailbox
        .request(&kit, &[(message.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(
        outbound_ends(&kit).await,
        [
            (
                "add_label".to_owned(),
                "skipped".to_owned(),
                Some("scope_missing".to_owned())
            ),
            (
                "move_to_stream".to_owned(),
                "skipped".to_owned(),
                Some("scope_missing".to_owned())
            ),
            ("mark_read".to_owned(), "applied".to_owned(), None),
        ]
    );
    assert!(kit.zoho.message(&message).read);
    assert_eq!(kit.zoho.message(&message).labels, [clients]);
    // No label call was made, and the stream is healthy.
    assert!(
        !kit.zoho.calls().iter().any(|c| c.contains("/labels")
            || c.ends_with("applyLabel")
            || c.ends_with("removeLabel")),
        "{:?}",
        kit.zoho.calls()
    );
    assert_eq!(kit.stream().await["consecutive_failures"], 0);
}
