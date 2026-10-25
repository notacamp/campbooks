//! Members' changes reaching Outlook mailboxes (campbooks-cloud#317), end to
//! end against the fake Graph: read, flag, archive, delete and restore, spam
//! and streams made with patches and moves, no echo of our own writes, a
//! `changeKey` conflict read again, Graph's later change winning, a move
//! that keeps the index (and one that does not keep its id), throttling,
//! and never a `DELETE`.

use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    events::ChangesRequested,
    testkit::{Caller, Kit, assert_contract, synthetic},
    testkit_microsoft::{FakeMicrosoft, Interference},
};

/// Ana's Outlook mailbox at the fake, connected and read once.
struct Mailbox {
    kit: Kit,
    ws: WorkspaceId,
    inbox: String,
    _ana: Caller,
}

/// Ana's mailbox: the fake seeded by `seed` (its answer handed back),
/// connected and read until its backfill ends.
async fn mailbox<T>(
    db: PgPool,
    seed: impl FnOnce(&FakeMicrosoft, DateTime<Utc>) -> T,
) -> (Mailbox, T) {
    let kit = Kit::with_microsoft(db).await;
    let seeded = seed(&kit.microsoft, kit.clock.now() - TimeDelta::hours(1));
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect_microsoft(&ana, ws).await;
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental", "the backfill ended");
    let mailbox = Mailbox {
        inbox: stream["inbox_id"].as_str().unwrap().to_owned(),
        kit,
        ws,
        _ana: ana,
    };
    (mailbox, seeded)
}

impl Mailbox {
    /// conversations asks for `items` (message id, change): the event is
    /// checked against the contract, published, and its consumer and the
    /// stream's run run.
    async fn request(&self, items: &[(&str, Value)]) {
        request_through(&self.kit, self.ws, &self.inbox, items).await;
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

    /// The newest upsert of `id`.
    async fn last(&self, id: &str) -> Value {
        self.changes()
            .await
            .into_iter()
            .rev()
            .find(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
            .map(|c| c["message"].clone())
            .unwrap_or_else(|| panic!("no upsert of {id}"))
    }

    /// Every outbound change as (kind, state, reason), oldest first.
    async fn ends(&self) -> Vec<(String, String, Option<String>)> {
        sqlx::query_as("SELECT kind, state, reason FROM connectors.outbound_changes ORDER BY id")
            .fetch_all(&self.kit.db)
            .await
            .unwrap()
    }

    /// The index rows: (provider id, tokens).
    async fn index(&self) -> Vec<(String, Vec<String>)> {
        sqlx::query_as(
            "SELECT provider_message_id, label_ids FROM connectors.messages
              ORDER BY provider_message_id",
        )
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }

    async fn batches(&self) -> usize {
        self.kit.batches(self.ws).await.len()
    }

    /// The fake's folder of message `id`.
    fn folder(&self, id: &str) -> String {
        self.kit.microsoft.message(id).unwrap().folder
    }

    /// Nothing at Graph was ever deleted or emptied.
    fn never_deleted(&self) {
        assert_eq!(self.kit.microsoft.deletes(), 0);
        for call in self.kit.microsoft.writes() {
            assert!(
                call.starts_with("PATCH /v1.0/me/messages/")
                    || (call.starts_with("POST /v1.0/me/messages/") && call.ends_with("/move"))
                    || call == "POST /v1.0/me/sendMail",
                "an unexpected write: {call}"
            );
        }
    }
}

/// conversations asks `kit` for `items` in `ws`'s inbox `inbox`.
async fn request_through(kit: &Kit, ws: WorkspaceId, inbox: &str, items: &[(&str, Value)]) {
    let conversation = uuid::Uuid::now_v7();
    let ids: Vec<&str> = items.iter().map(|(id, _)| *id).collect();
    kit.outbound.show(conversation, &ids);
    let payload = json!({
        "workspace": ws.to_string(),
        "inbox_id": inbox,
        "conversation": conversation,
        "origin": "user",
        "items": items.iter().map(|(id, change)| json!({
            "provider_message_id": id, "change": change
        })).collect::<Vec<_>>(),
    });
    assert_contract("conversations.changes_requested.v1.schema.json", &payload);
    let event: ChangesRequested = serde_json::from_value(payload).unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state.events.publish(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn end(kind: &str, state: &str, reason: Option<&str>) -> (String, String, Option<String>) {
    (
        kind.to_owned(),
        state.to_owned(),
        reason.map(ToOwned::to_owned),
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn read_star_and_archive_reach_outlook_without_echo(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    let batches = m.batches().await;

    m.request(&[(id.as_str(), json!({"kind": "mark_read"}))])
        .await;
    m.request(&[(id.as_str(), json!({"kind": "star"}))]).await;
    let held = m.kit.microsoft.message(&id).unwrap();
    assert!(held.read && held.flagged);
    // Each patch carried the version just read.
    let patches = m.kit.microsoft.patches();
    assert_eq!(patches.len(), 2);
    assert!(patches.iter().all(|p| p.id == id));
    assert_eq!(patches[0].body, json!({"isRead": true}));
    assert_eq!(patches[1].body, json!({"flag": {"flagStatus": "flagged"}}));
    assert!(patches.iter().all(|p| p.if_match.is_some()));
    assert_eq!(
        m.ends().await,
        [
            end("mark_read", "applied", None),
            end("star", "applied", None)
        ]
    );

    // Archived: moved, the same id, one index row, no download again.
    m.request(&[(id.as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(m.folder(&id), "fld-archive");
    let index = m.index().await;
    assert_eq!(index.len(), 1);
    assert_eq!(index[0].0, id);
    assert!(index[0].1.contains(&"ARCHIVE".to_owned()));
    // No echo: neither the runs that wrote nor the next one announce it.
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches);
    assert_eq!(m.kit.microsoft.raw_fetches(&id), 1);
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn delete_restore_and_spam_are_moves_never_deletes(db: PgPool) {
    let (m, (filed, other)) = mailbox(db, |fake, at| {
        let clients = fake.add_folder("Clients", None);
        (
            fake.deliver(&clients, &synthetic(1), at),
            fake.deliver("fld-inbox", &synthetic(2), at),
        )
    })
    .await;
    let clients = m.folder(&filed);

    m.request(&[(filed.as_str(), json!({"kind": "trash"}))])
        .await;
    assert_eq!(m.folder(&filed), "fld-deleted");
    // Where it was is kept, bound to the change.
    let restore: Vec<Option<Vec<String>>> =
        sqlx::query_scalar("SELECT restore_to FROM connectors.outbound_changes")
            .fetch_all(&m.kit.db)
            .await
            .unwrap();
    assert_eq!(restore, [Some(vec![format!("folder:{clients}")])]);
    m.request(&[(filed.as_str(), json!({"kind": "untrash", "inbound": true}))])
        .await;
    assert_eq!(m.folder(&filed), clients, "back where it was");

    m.request(&[(other.as_str(), json!({"kind": "spam"}))])
        .await;
    assert_eq!(m.folder(&other), "fld-junk");
    let index = m.index().await;
    let spam = index.iter().find(|(i, _)| *i == other).unwrap();
    assert!(spam.1.contains(&"JUNK".to_owned()), "{spam:?}");
    assert_eq!(
        m.ends().await,
        [
            end("trash", "applied", None),
            end("untrash", "applied", None),
            end("spam", "applied", None)
        ]
    );
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn streams_go_to_their_folder_else_a_category(db: PgPool) {
    let (m, (updates, first, second)) = mailbox(db, |fake, at| {
        let parent = fake.add_folder("Campbooks", Some("fld-inbox"));
        let updates = fake.add_folder("Updates", Some(&parent));
        (
            updates,
            fake.deliver("fld-inbox", &synthetic(1), at),
            fake.deliver("fld-inbox", &synthetic(2), at),
        )
    })
    .await;
    m.kit
        .microsoft
        .change(&second, |msg| msg.categories = vec!["Red".to_owned()]);
    m.kit.next_run().await;
    let batches = m.batches().await;

    // Updates has its folder: moved there.
    m.request(&[(
        first.as_str(),
        json!({"kind": "move_to_stream", "stream": "updates"}),
    )])
    .await;
    assert_eq!(m.folder(&first), updates);
    // Receipts has none: kept in the Inbox, with its category.
    m.request(&[(
        second.as_str(),
        json!({"kind": "move_to_stream", "stream": "receipts"}),
    )])
    .await;
    let held = m.kit.microsoft.message(&second).unwrap();
    assert_eq!(held.folder, "fld-inbox");
    assert_eq!(held.categories, ["Campbooks/Receipts", "Red"]);
    m.kit.next_run().await;
    assert_eq!(m.batches().await, batches, "no echo");
    // Back to Primary: the category comes off, the other stays.
    m.request(&[(
        second.as_str(),
        json!({"kind": "move_to_stream", "stream": "primary"}),
    )])
    .await;
    assert_eq!(
        m.kit.microsoft.message(&second).unwrap().categories,
        ["Red"]
    );
    // Archived mail has no place in a stream.
    m.request(&[(first.as_str(), json!({"kind": "archive"}))])
        .await;
    m.request(&[(
        first.as_str(),
        json!({"kind": "move_to_stream", "stream": "receipts"}),
    )])
    .await;
    assert_eq!(m.folder(&first), "fld-archive");
    assert_eq!(
        m.ends().await.last().unwrap(),
        &end("move_to_stream", "skipped", Some("not_movable"))
    );
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_conflict_reads_the_message_again_then_writes(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    // Outlook adds a category between the push's read and its patch.
    m.kit
        .microsoft
        .interfere_on_next_patch(Interference::Category("Blue".into()));
    m.request(&[(id.as_str(), json!({"kind": "mark_read"}))])
        .await;
    let patches = m.kit.microsoft.patches();
    assert_eq!(patches.len(), 2, "412, read again, patched");
    assert_ne!(patches[0].if_match, patches[1].if_match);
    let held = m.kit.microsoft.message(&id).unwrap();
    assert!(held.read);
    assert_eq!(held.categories, ["Blue"]);
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
    // Graph's own change is shown too.
    let shown = m.last(&id).await;
    assert_eq!(shown["read"], true);
    assert_eq!(shown["labels"][0]["name"], "Blue");
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn graph_s_later_change_wins(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    // Moved to Junk Email in Outlook after the last read.
    m.kit.microsoft.move_to(&id, "fld-junk");
    m.request(&[(id.as_str(), json!({"kind": "trash"}))]).await;
    assert_eq!(m.folder(&id), "fld-junk", "nothing moved");
    assert_eq!(
        m.ends().await,
        [end("trash", "superseded", Some("changed_at_provider"))]
    );
    assert_eq!(m.last(&id).await["mailbox"]["in_spam"], true);
    assert!(
        !m.kit
            .microsoft
            .writes()
            .iter()
            .any(|c| c.ends_with("/move") || c.starts_with("PATCH")),
        "nothing written"
    );

    // Already as asked at Graph: nothing to call, and it is made.
    let calls = m.kit.microsoft.writes().len();
    m.kit.microsoft.change(&id, |msg| msg.read = true);
    m.request(&[(id.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert_eq!(m.kit.microsoft.writes().len(), calls);
    assert_eq!(
        m.ends().await.last().unwrap(),
        &end("mark_read", "applied", None)
    );
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_answering_another_id_keeps_one_message(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    m.kit.microsoft.rename_on_move(true);
    m.request(&[(id.as_str(), json!({"kind": "archive"}))])
        .await;
    m.kit.next_run().await;
    let index = m.index().await;
    assert_eq!(index.len(), 1, "{index:?}");
    let new = index[0].0.clone();
    assert_ne!(new, id);
    let changes = m.changes().await;
    assert!(
        changes
            .iter()
            .any(|c| c["op"] == "remove" && c["provider_message_id"] == id.as_str())
    );
    assert_eq!(m.last(&new).await["mailbox"]["in_inbox"], false);
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttled_while_writing_waits_then_writes(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    let before = m.kit.clock.now();
    m.kit.microsoft.throttle_next(120);
    m.request(&[(id.as_str(), json!({"kind": "mark_read"}))])
        .await;
    assert!(!m.kit.microsoft.message(&id).unwrap().read);
    assert_eq!(m.ends().await, [end("mark_read", "pending", None)]);
    let health: String = sqlx::query_scalar("SELECT health FROM connectors.connections")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert_eq!(health, "throttled");
    let due: DateTime<Utc> =
        serde_json::from_value(m.kit.stream().await["due_at"].clone()).unwrap();
    assert_eq!(due, before + TimeDelta::seconds(120));
    m.kit.next_run().await;
    assert!(m.kit.microsoft.message(&id).unwrap().read);
    assert_eq!(m.ends().await, [end("mark_read", "applied", None)]);
    m.never_deleted();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn label_changes_are_not_made_at_microsoft(db: PgPool) {
    let (m, id) = mailbox(db, |fake, at| fake.deliver("fld-inbox", &synthetic(1), at)).await;
    m.request(&[(
        id.as_str(),
        json!({"kind": "add_label", "label": uuid::Uuid::now_v7(), "provider_key": "Red"}),
    )])
    .await;
    assert_eq!(
        m.ends().await,
        [end("add_label", "skipped", Some("unsupported"))]
    );
    assert!(m.kit.microsoft.writes().is_empty());
}

/// Microsoft mail turned off with a mailbox connected: a member's change
/// is not made and a send does not go out; nothing is asked of Microsoft.
#[sqlx::test(migrations = "../../../migrations")]
async fn microsoft_mail_off_writes_and_sends_nothing(db: PgPool) {
    let (m, id) = mailbox(db.clone(), |fake, at| {
        fake.deliver("fld-inbox", &synthetic(1), at)
    })
    .await;
    m.kit.microsoft.clear_calls();
    let off = Kit::new(db).await;
    assert!(off.deps.microsoft.is_none());
    request_through(
        &off,
        m.ws,
        &m.inbox,
        &[(id.as_str(), json!({"kind": "archive"}))],
    )
    .await;
    assert_eq!(
        m.ends().await,
        [end("archive", "skipped", Some("unsupported"))]
    );

    let send = crate::domain::model::SendId::new();
    let inbox: crate::domain::model::InboxId =
        serde_json::from_value(Value::String(m.inbox.clone())).unwrap();
    let person = |address: &str| crate::contract::Address {
        name: None,
        address: address.to_owned(),
    };
    let mut document = serde_json::to_value(crate::contract::OutgoingMessage {
        version: 1,
        inbox,
        rfc_message_id: format!("{send}@example.com"),
        from: person(crate::testkit_microsoft::ADDRESS),
        to: vec![person("bea@example.net")],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "Re: Plano".into(),
        date: off.clock.now(),
        in_reply_to: None,
        references: Vec::new(),
        thread_key: None,
        text: "Done.".into(),
        html: None,
        attachments: Vec::new(),
    })
    .unwrap();
    document["send_id"] = send.to_string().into();
    assert_contract("outgoing-message.v1.schema.json", &document);
    let blob = off
        .state
        .blobs
        .put(
            &m.ws.to_string(),
            serde_json::to_vec(&document).unwrap(),
            crate::contract::OUTGOING_CONTENT_TYPE,
        )
        .await
        .unwrap();
    off.outbound.release(send, inbox, blob.clone());
    let mut tx = off.state.db.begin_for(m.ws).await.unwrap();
    off.state
        .events
        .publish(
            &mut tx,
            &crate::events::SendRequested {
                workspace: m.ws.to_string(),
                inbox_id: inbox,
                send_id: send,
                message: blob,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    off.run_jobs().await;
    let completed = off.payloads(m.ws, "connectors.send_completed").await;
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0]["outcome"], "failed");
    assert_eq!(completed[0]["reason"], "not_connected");
    off.tick(TimeDelta::hours(5)).await;
    assert!(
        m.kit.microsoft.calls().is_empty(),
        "{:?}",
        m.kit.microsoft.calls()
    );
    assert_eq!(m.folder(&id), "fld-inbox");
}
