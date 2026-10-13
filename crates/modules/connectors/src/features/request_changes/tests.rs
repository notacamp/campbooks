//! Members' changes reaching Gmail, against the fake Google: every intent
//! kind, merging, batching, echo suppression, last writer wins, retries and
//! permanent failures.

use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    domain::{model::StreamId, schedule},
    events::ChangesRequested,
    testkit::{Kit, assert_contract, synthetic},
};

/// A connected workspace whose mailbox holds `labels.len()` messages, synced.
struct Mailbox {
    ws: WorkspaceId,
    inbox: String,
    stream: StreamId,
    ids: Vec<String>,
}

async fn mailbox(kit: &Kit, labels: &[&[&str]]) -> Mailbox {
    let ids = labels
        .iter()
        .enumerate()
        .map(|(n, labels)| {
            let at = kit.clock.now() - TimeDelta::hours(i64::try_from(n).unwrap() + 1);
            kit.google.seed(&synthetic(n), labels, at)
        })
        .collect();
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    let connection = kit.connect(&sam, ws).await;
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental");
    Mailbox {
        ws,
        inbox: connection["streams"][0]["inbox_id"]
            .as_str()
            .unwrap()
            .to_owned(),
        stream: serde_json::from_value(stream["id"].clone()).unwrap(),
        ids,
    }
}

impl Mailbox {
    /// conversations asks for `items` (message id, change): the event is
    /// checked against the contract, published and its consumer run.
    async fn request(&self, kit: &Kit, items: &[(&str, Value)]) {
        let payload = json!({
            "workspace": self.ws.to_string(),
            "inbox_id": self.inbox,
            "conversation": uuid::Uuid::now_v7(),
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

    /// Run the stream now.
    async fn run(&self, kit: &Kit) {
        let mut tx = kit.state.db.begin_for(self.ws).await.unwrap();
        schedule::run_now(&mut tx, self.ws, self.stream, kit.clock.now())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
    }

    /// Pause or resume the stream (a run then makes no write).
    async fn pause(&self, kit: &Kit, paused: bool) {
        sqlx::query!(
            "UPDATE connectors.streams SET paused = $2 WHERE id = $1",
            self.stream as StreamId,
            paused
        )
        .execute(&kit.db)
        .await
        .unwrap();
    }
}

/// Every outbound change as (message, kind, state, reason), oldest first.
async fn outbound(kit: &Kit) -> Vec<(String, String, String, Option<String>)> {
    sqlx::query!(
        "SELECT provider_message_id, kind, state, reason FROM connectors.outbound_changes ORDER BY id"
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
    .into_iter()
    .map(|r| (r.provider_message_id, r.kind, r.state, r.reason))
    .collect()
}

/// The ends of the outbound changes as (state, reason) only.
async fn ends(kit: &Kit) -> Vec<(String, Option<String>)> {
    outbound(kit)
        .await
        .into_iter()
        .map(|(_, _, state, reason)| (state, reason))
        .collect()
}

fn end(state: &str, reason: Option<&str>) -> (String, Option<String>) {
    (state.to_owned(), reason.map(ToOwned::to_owned))
}

/// Every change of every batch announced so far.
async fn changes(kit: &Kit, ws: WorkspaceId) -> Vec<Value> {
    kit.batches(ws)
        .await
        .into_iter()
        .flat_map(|b| b["changes"].as_array().unwrap().clone())
        .collect()
}

fn labels(list: &[&str]) -> Vec<String> {
    let mut list: Vec<String> = list.iter().map(|&s| s.to_owned()).collect();
    list.sort();
    list
}

#[sqlx::test(migrations = "../../../migrations")]
async fn every_intent_reaches_gmail(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(
        &kit,
        &[
            &["INBOX", "UNREAD"],
            &["INBOX"],
            &["INBOX", "STARRED"],
            &["INBOX", "CATEGORY_PROMOTIONS"],
            &["INBOX", "Label_7"],
        ],
    )
    .await;
    let id = |n: usize| m.ids[n].as_str();
    let label = uuid::Uuid::now_v7();
    m.request(&kit, &[(id(0), json!({"kind": "mark_read"}))])
        .await;
    m.request(&kit, &[(id(1), json!({"kind": "mark_unread"}))])
        .await;
    m.request(&kit, &[(id(1), json!({"kind": "star"}))]).await;
    m.request(&kit, &[(id(2), json!({"kind": "unstar"}))]).await;
    m.request(&kit, &[(id(2), json!({"kind": "archive"}))])
        .await;
    m.request(
        &kit,
        &[(
            id(3),
            json!({"kind": "move_to_stream", "stream": "primary"}),
        )],
    )
    .await;
    m.request(
        &kit,
        &[(
            id(4),
            json!({"kind": "remove_label", "label": label, "provider_key": "Label_7"}),
        )],
    )
    .await;
    m.request(
        &kit,
        &[(
            id(0),
            json!({"kind": "add_label", "label": label, "provider_key": "Label_9"}),
        )],
    )
    .await;

    assert_eq!(kit.google.labels_of(id(0)), labels(&["INBOX", "Label_9"]));
    assert_eq!(
        kit.google.labels_of(id(1)),
        labels(&["INBOX", "STARRED", "UNREAD"])
    );
    assert_eq!(kit.google.labels_of(id(2)), Vec::<String>::new());
    assert_eq!(
        kit.google.labels_of(id(3)),
        labels(&["CATEGORY_PERSONAL", "INBOX"])
    );
    assert_eq!(kit.google.labels_of(id(4)), labels(&["INBOX"]));
    assert!(ends(&kit).await.iter().all(|e| *e == end("applied", None)));

    // Undo: unarchive, and a move to Updates.
    m.request(&kit, &[(id(2), json!({"kind": "unarchive"}))])
        .await;
    m.request(
        &kit,
        &[(
            id(3),
            json!({"kind": "move_to_stream", "stream": "updates"}),
        )],
    )
    .await;
    assert_eq!(kit.google.labels_of(id(2)), labels(&["INBOX"]));
    assert_eq!(
        kit.google.labels_of(id(3)),
        labels(&["CATEGORY_UPDATES", "INBOX"])
    );

    // What Gmail cannot do is recorded, not attempted.
    kit.google.clear_calls();
    m.request(
        &kit,
        &[
            (id(0), json!({"kind": "add_label", "label": label})),
            (
                id(1),
                json!({"kind": "add_label", "label": label, "provider_key": "Label_404"}),
            ),
        ],
    )
    .await;
    m.request(
        &kit,
        &[(
            id(4),
            json!({"kind": "move_to_stream", "stream": "receipts"}),
        )],
    )
    .await;
    m.request(&kit, &[("msg-never", json!({"kind": "archive"}))])
        .await;
    let rows = outbound(&kit).await;
    let tail: Vec<_> = rows[rows.len() - 4..]
        .iter()
        .map(|(_, kind, state, reason)| (kind.as_str(), state.as_str(), reason.as_deref()))
        .collect();
    assert_eq!(
        tail,
        [
            ("add_label", "skipped", Some("label_not_at_provider")),
            ("add_label", "skipped", Some("label_not_at_provider")),
            ("move_to_stream", "skipped", Some("no_provider_equivalent")),
            ("archive", "skipped", Some("message_gone")),
        ]
    );
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());

    // A replayed event records nothing twice.
    let before = outbound(&kit).await.len();
    let replayed = kit
        .state
        .events
        .replay(
            &kit.db,
            "connectors.request_changes",
            campbooks_kernel::events::ReplayFrom::Beginning,
        )
        .await;
    assert!(replayed.is_ok(), "{replayed:?}");
    kit.run_jobs().await;
    assert_eq!(outbound(&kit).await.len(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_write_back_never_echoes(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"], &["INBOX"]]).await;
    let batches = kit.batches(m.ws).await.len();
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "star"}))])
        .await;
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["INBOX", "STARRED", "UNREAD"])
    );
    // Gmail's history now holds our write; the next runs read it and emit
    // nothing, and record nothing to write back.
    m.run(&kit).await;
    m.run(&kit).await;
    assert_eq!(kit.batches(m.ws).await.len(), batches);
    assert_eq!(outbound(&kit).await.len(), 1);

    // A change made in Gmail meanwhile still arrives: starred there, not
    // yet read by Campbooks, while a member archives.
    kit.google.relabel(&m.ids[1], &["STARRED"], &[]);
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "archive"}))])
        .await;
    assert_eq!(kit.google.labels_of(&m.ids[1]), labels(&["STARRED"]));
    let new: Vec<Value> = kit.batches(m.ws).await[batches..]
        .iter()
        .flat_map(|b| b["changes"].as_array().unwrap().clone())
        .collect();
    assert_eq!(new.len(), 1, "{new:#?}");
    assert_eq!(new[0]["message"]["provider_message_id"], m.ids[1].as_str());
    assert_eq!(new[0]["message"]["starred"], true);
    assert_eq!(new[0]["message"]["mailbox"]["in_inbox"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn changes_before_a_run_are_merged(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"]]).await;
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "archive"}))])
        .await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "unarchive"}))])
        .await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "mark_read"}))])
        .await;
    m.pause(&kit, false).await;
    kit.google.clear_calls();
    m.run(&kit).await;
    assert_eq!(
        ends(&kit).await,
        [
            end("superseded", Some("replaced")),
            end("applied", None),
            end("applied", None)
        ]
    );
    // One call: INBOX was put back (already there) with UNREAD taken off.
    assert_eq!(kit.google.writes().len(), 1, "{:?}", kit.google.writes());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn many_messages_go_in_one_batch(db: PgPool) {
    let kit = Kit::new(db).await;
    let twelve: Vec<&[&str]> = vec![&["INBOX", "UNREAD"]; 12];
    let m = mailbox(&kit, &twelve).await;
    kit.google.clear_calls();
    let items: Vec<(&str, Value)> = m
        .ids
        .iter()
        .map(|id| (id.as_str(), json!({"kind": "mark_read"})))
        .collect();
    m.request(&kit, &items).await;
    assert_eq!(
        kit.google.writes(),
        ["POST /gmail/v1/users/me/messages/batchModify"]
    );
    for id in &m.ids {
        assert_eq!(kit.google.labels_of(id), labels(&["INBOX"]));
    }
    // A batch with one message Gmail refuses is retried one by one.
    kit.google.refuse_changes(&m.ids[3]);
    kit.google.clear_calls();
    let items: Vec<(&str, Value)> = m
        .ids
        .iter()
        .map(|id| (id.as_str(), json!({"kind": "archive"})))
        .collect();
    m.request(&kit, &items).await;
    let writes = kit.google.writes();
    assert_eq!(writes.len(), 1 + 12, "{writes:?}");
    let rows = outbound(&kit).await;
    let failed: Vec<_> = rows.iter().filter(|r| r.2 == "failed").collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, m.ids[3]);
    assert_eq!(failed[0].3.as_deref(), Some("refused"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_that_refuses_changes_fails_alone(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"], &["INBOX"], &["INBOX"]]).await;
    kit.google.refuse_changes(&m.ids[1]);
    let batches = kit.batches(m.ws).await.len();
    let items: Vec<(&str, Value)> = m
        .ids
        .iter()
        .map(|id| (id.as_str(), json!({"kind": "archive"})))
        .collect();
    m.request(&kit, &items).await;
    assert!(kit.google.labels_of(&m.ids[0]).is_empty());
    assert_eq!(kit.google.labels_of(&m.ids[1]), labels(&["INBOX"]));
    assert!(kit.google.labels_of(&m.ids[2]).is_empty());
    assert_eq!(
        ends(&kit).await,
        [
            end("applied", None),
            end("failed", Some("refused")),
            end("applied", None)
        ]
    );
    // Recorded without content, and Campbooks is told Gmail's state.
    let row: Value = sqlx::query_scalar!(
        r#"SELECT to_jsonb(o) AS "row!" FROM connectors.outbound_changes o WHERE state = 'failed'"#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let text = row.to_string();
    assert!(
        !text.contains("Message 1") && !text.contains("example.com"),
        "{text}"
    );
    let new = &kit.batches(m.ws).await[batches..];
    assert_eq!(new.len(), 1);
    let change = &new[0]["changes"][0]["message"];
    assert_eq!(change["provider_message_id"], m.ids[1].as_str());
    assert_eq!(change["mailbox"]["in_inbox"], true);
    // Said once: the next run is quiet.
    m.run(&kit).await;
    assert_eq!(kit.batches(m.ws).await.len(), batches + 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_later_change_in_gmail_wins(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "star"}))])
        .await;
    // After the member: starred and unstarred again in Gmail.
    kit.google.relabel(&m.ids[0], &["STARRED"], &[]);
    kit.google.relabel(&m.ids[0], &[], &["STARRED"]);
    m.pause(&kit, false).await;
    kit.google.clear_calls();
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(
        ends(&kit).await,
        [end("superseded", Some("changed_at_provider"))]
    );
    let last = changes(&kit, m.ws).await.pop().unwrap();
    assert_eq!(last["message"]["provider_message_id"], m.ids[0].as_str());
    assert_eq!(last["message"]["starred"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_earlier_change_in_gmail_loses(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    // Archived in Gmail; Campbooks has not read that yet when the member
    // puts the message back in the inbox: the member's change is later.
    kit.google.relabel(&m.ids[0], &[], &["INBOX"]);
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "unarchive"}))])
        .await;
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(ends(&kit).await, [end("applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_undo_during_a_write_is_not_taken_for_gmail(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "archive"}))])
        .await;
    assert!(kit.google.labels_of(&m.ids[0]).is_empty());
    // The undo was recorded while the archive was being written: its stamp
    // is from before that write.
    sqlx::query!(
        r#"INSERT INTO connectors.outbound_changes
               (id, workspace_id, stream_id, request_id, provider_message_id, kind,
                add_labels, stamp_history_id)
           SELECT $1, workspace_id, stream_id, $2, provider_message_id, 'unarchive',
                  '{INBOX}', stamp_history_id
             FROM connectors.outbound_changes"#,
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
    )
    .execute(&kit.db)
    .await
    .unwrap();
    m.run(&kit).await;
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(
        ends(&kit).await,
        [end("applied", None), end("applied", None)]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttling_and_outages_retry_then_give_up(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"], &["INBOX"]]).await;

    // 429 with Retry-After: pending, throttled, written once it passes.
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "mark_read"}))])
        .await;
    m.pause(&kit, false).await;
    kit.google.rate_limit_next(120);
    m.run(&kit).await;
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["INBOX", "UNREAD"])
    );
    assert_eq!(ends(&kit).await, [end("pending", None)]);
    let stream = kit.stream().await;
    assert!(stream["throttled_until"].is_string());
    kit.tick(TimeDelta::minutes(3)).await;
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(ends(&kit).await, [end("applied", None)]);

    // 503: backoff, then written.
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "star"}))])
        .await;
    m.pause(&kit, false).await;
    kit.google.unavailable_next(1);
    m.run(&kit).await;
    assert_eq!(kit.stream().await["consecutive_failures"], 1);
    assert_eq!(ends(&kit).await[1], end("pending", None));
    kit.next_run().await;
    assert_eq!(
        kit.google.labels_of(&m.ids[1]),
        labels(&["INBOX", "STARRED"])
    );

    // A change that keeps meeting trouble is given up and re-emitted.
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "unstar"}))])
        .await;
    // (Written already: make it one that has met trouble seven times.)
    sqlx::query!(
        r#"UPDATE connectors.outbound_changes SET state = 'pending', attempts = 7, done_at = NULL
            WHERE kind = 'unstar'"#
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.google.relabel(&m.ids[1], &["STARRED"], &[]);
    let batches = kit.batches(m.ws).await.len();
    kit.google.unavailable_next(1);
    m.run(&kit).await;
    assert_eq!(ends(&kit).await[2], end("failed", Some("provider_errors")));
    kit.next_run().await;
    let last = changes(&kit, m.ws).await.pop().unwrap();
    assert!(kit.batches(m.ws).await.len() > batches);
    assert_eq!(last["message"]["provider_message_id"], m.ids[1].as_str());
    assert_eq!(last["message"]["starred"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_pending_change_shows_as_the_member_left_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"]]).await;
    // A member archives; before it is written, Gmail marks it read and a
    // run reads that.
    sqlx::query!(
        r#"INSERT INTO connectors.outbound_changes
               (id, workspace_id, stream_id, request_id, provider_message_id, kind,
                remove_labels)
           VALUES ($1, $2, $3, $4, $5, 'archive', '{INBOX}')"#,
        uuid::Uuid::now_v7(),
        *m.ws.as_uuid(),
        m.stream as StreamId,
        uuid::Uuid::now_v7(),
        m.ids[0],
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.google.relabel(&m.ids[0], &[], &["UNREAD"]);
    let page = crate::features::sync_stream::read_page(&kit.state, &kit.deps, m.ws, m.stream).await;
    let message = &page[0];
    assert_eq!(message["message"]["read"], true);
    assert_eq!(message["message"]["mailbox"]["in_inbox"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_change_is_given_up_not_made_late(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    // Recorded ten days ago and never made (a long pause, a reconnect).
    sqlx::query!(
        r#"INSERT INTO connectors.outbound_changes
               (id, workspace_id, stream_id, request_id, provider_message_id, kind,
                remove_labels, created_at)
           VALUES ($1, $2, $3, $4, $5, 'archive', '{INBOX}', $6)"#,
        uuid::Uuid::now_v7(),
        *m.ws.as_uuid(),
        m.stream as StreamId,
        uuid::Uuid::now_v7(),
        m.ids[0],
        kit.clock.now() - TimeDelta::days(10),
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let batches = kit.batches(m.ws).await.len();
    kit.google.clear_calls();
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(ends(&kit).await, [end("failed", Some("expired"))]);
    // Campbooks is told Gmail's state: still in the inbox.
    let new = &kit.batches(m.ws).await[batches..];
    assert_eq!(new.len(), 1);
    assert_eq!(new[0]["changes"][0]["message"]["mailbox"]["in_inbox"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_partly_overtaken_move_is_not_made_at_all(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_UPDATES"]]).await;
    m.pause(&kit, true).await;
    m.request(
        &kit,
        &[(
            m.ids[0].as_str(),
            json!({"kind": "move_to_stream", "stream": "newsletters"}),
        )],
    )
    .await;
    // Later, in Gmail: moved to Social.
    kit.google
        .relabel(&m.ids[0], &["CATEGORY_SOCIAL"], &["CATEGORY_UPDATES"]);
    m.pause(&kit, false).await;
    kit.google.clear_calls();
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["CATEGORY_SOCIAL", "INBOX"])
    );
    assert_eq!(
        ends(&kit).await,
        [end("superseded", Some("changed_at_provider"))]
    );
    let last = changes(&kit, m.ws).await.pop().unwrap();
    assert_eq!(last["message"]["provider_message_id"], m.ids[0].as_str());
    assert_eq!(last["message"]["categories"], json!(["social"]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_write_window_left_open_is_closed(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "archive"}))])
        .await;
    m.pause(&kit, false).await;
    // The profile call after the write fails: the window has no end yet,
    // and the same run's read (which follows Gmail past the write) closes it.
    kit.google.profile_unavailable_next();
    m.run(&kit).await;
    assert!(kit.google.labels_of(&m.ids[0]).is_empty());
    let row = sqlx::query!("SELECT applied_from, applied_to FROM connectors.outbound_changes")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let cursor: i64 = kit.stream().await["cursor"]["history_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(row.applied_to, Some(cursor));
    assert!(row.applied_from.unwrap() < cursor);

    // A window no run could close covers nothing after ten minutes.
    sqlx::query!("UPDATE connectors.outbound_changes SET applied_to = NULL")
        .execute(&kit.db)
        .await
        .unwrap();
    sqlx::query!(
        r#"INSERT INTO connectors.outbound_changes
               (id, workspace_id, stream_id, request_id, provider_message_id, kind, add_labels)
           VALUES ($1, $2, $3, $4, $5, 'unarchive', '{INBOX}')"#,
        uuid::Uuid::now_v7(),
        *m.ws.as_uuid(),
        m.stream as StreamId,
        uuid::Uuid::now_v7(),
        m.ids[0],
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let own = |backlog: crate::domain::outbound::Backlog| backlog.own[&m.ids[0]][0].to;
    let fresh = crate::domain::outbound::backlog(&kit.state.db, m.ws, m.stream, kit.clock.now())
        .await
        .unwrap();
    assert_eq!(own(fresh), None);
    let later = kit.clock.now() + TimeDelta::minutes(11);
    let stale = crate::domain::outbound::backlog(&kit.state.db, m.ws, m.stream, later)
        .await
        .unwrap();
    assert_eq!(own(stale), row.applied_from);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_in_the_trash_or_spam_is_not_brought_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(
        &kit,
        &[&["TRASH"], &["SPAM", "CATEGORY_UPDATES"], &["INBOX"]],
    )
    .await;
    kit.google.clear_calls();
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "unarchive"}))])
        .await;
    m.request(
        &kit,
        &[(
            m.ids[1].as_str(),
            json!({"kind": "move_to_stream", "stream": "primary"}),
        )],
    )
    .await;
    // A system label is never a label change, whatever the intent says.
    m.request(
        &kit,
        &[(
            m.ids[2].as_str(),
            json!({"kind": "add_label", "label": uuid::Uuid::now_v7(), "provider_key": "TRASH"}),
        )],
    )
    .await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["TRASH"]));
    assert_eq!(
        kit.google.labels_of(&m.ids[1]),
        labels(&["CATEGORY_UPDATES", "SPAM"])
    );
    assert_eq!(kit.google.labels_of(&m.ids[2]), labels(&["INBOX"]));
    assert_eq!(
        ends(&kit).await,
        [
            end("skipped", Some("in_trash")),
            end("skipped", Some("in_trash")),
            end("skipped", Some("label_not_at_provider")),
        ]
    );
}
