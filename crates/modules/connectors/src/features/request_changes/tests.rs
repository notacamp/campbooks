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
    domain::{marks::MarkIds, model::StreamId, schedule},
    events::ChangesRequested,
    testkit::{Kit, assert_contract, synthetic},
    tracking_tests::{job, tracked, tracked_as},
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
        // The conversation shows every change asked (conversations applied
        // them before asking).
        let conversation = uuid::Uuid::now_v7();
        let ids: Vec<&str> = items.iter().map(|(id, _)| *id).collect();
        kit.outbound.show(conversation, &ids);
        self.publish(kit, conversation, items).await;
    }

    /// Publish `changes_requested` for `items` of `conversation` and run its
    /// consumer, whatever conversations shows.
    async fn publish(&self, kit: &Kit, conversation: uuid::Uuid, items: &[(&str, Value)]) {
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
    m.request(&kit, &[("msg-never", json!({"kind": "archive"}))])
        .await;
    let rows = outbound(&kit).await;
    let tail: Vec<_> = rows[rows.len() - 3..]
        .iter()
        .map(|(_, kind, state, reason)| (kind.as_str(), state.as_str(), reason.as_deref()))
        .collect();
    assert_eq!(
        tail,
        [
            ("add_label", "skipped", Some("label_not_at_provider")),
            ("add_label", "skipped", Some("label_not_at_provider")),
            ("archive", "skipped", Some("message_gone")),
        ]
    );
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());

    // Receipts: the Updates tab and the Receipts mark, made first.
    m.request(
        &kit,
        &[(
            id(4),
            json!({"kind": "move_to_stream", "stream": "receipts"}),
        )],
    )
    .await;
    let mark = kit.google.label_named("Campbooks/Receipts").unwrap();
    let mut want = labels(&["CATEGORY_UPDATES", "INBOX"]);
    want.push(mark);
    want.sort();
    assert_eq!(kit.google.labels_of(id(4)), want);
    assert_eq!(outbound(&kit).await.last().unwrap().2, "applied");

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
    // One summary of the run's push, counted by end.
    let made = tracked(&kit, "connectors.outbound_changes_made").await;
    assert_eq!(made.len(), 1);
    tracked_as(
        &made[0],
        "connectors.outbound_changes_made",
        m.ws,
        &job("connectors.sync_stream"),
        None,
    );
    assert_eq!(made[0].body["stream_id"], m.stream.to_string());
    assert_eq!(made[0].body["provider"], "google");
    assert_eq!(made[0].body["applied"], 2);
    assert_eq!(made[0].body["superseded"], 1);
    assert_eq!(made[0].body["skipped"], 0);
    assert_eq!(made[0].body["failed"], 0);
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
async fn a_push_counts_the_units_its_calls_spent(db: PgPool) {
    // campbooks-cloud#252: the run settles its reservation with this.
    let kit = Kit::new(db).await;
    let twelve: Vec<&[&str]> = vec![&["INBOX", "UNREAD"]; 12];
    let m = mailbox(&kit, &twelve).await;
    let backlog = |ids: &[String]| crate::domain::outbound::Backlog {
        pending: ids
            .iter()
            .map(|id| crate::domain::outbound::Pending {
                id: uuid::Uuid::now_v7(),
                provider_message_id: id.clone(),
                add: Vec::new(),
                remove: vec!["INBOX".to_owned()],
                stamp: None,
                attempts: 0,
                created_at: kit.clock.now(),
            })
            .collect(),
        labels: ids
            .iter()
            .map(|id| (id.clone(), vec!["INBOX".to_owned(), "UNREAD".to_owned()]))
            .collect(),
        ..Default::default()
    };
    let google = kit.deps.google.clone().unwrap();
    let (access, _) = kit.google.grant();

    // As planned: the profile, one batch, the profile after.
    let planned = backlog(&m.ids);
    assert_eq!(
        crate::gmail::write::units(&planned, &MarkIds::default()),
        2 + 4 * 2 + 50,
        "reserved: both profiles, the history pages it may read, the batch"
    );
    let pushed =
        crate::gmail::write::push(&google.gmail(&access), &planned, &MarkIds::default(), None)
            .await;
    assert_eq!(pushed.spent, 1 + 50 + 1);

    // A batch Gmail refuses: the batch, then each message on its own.
    kit.google.refuse_changes(&m.ids[3]);
    let refused = backlog(&m.ids);
    let pushed =
        crate::gmail::write::push(&google.gmail(&access), &refused, &MarkIds::default(), None)
            .await;
    assert_eq!(pushed.spent, 1 + 50 + 12 * 5 + 1);
}

/// Watch the shared budget's settlements (an update that does not refill):
/// log each one's change of tokens to `settled`, or refuse it when `refuse`.
async fn watch_settlements(kit: &Kit, refuse: bool) {
    let body = if refuse {
        "RAISE EXCEPTION 'settlement refused for the test';"
    } else {
        "INSERT INTO public.settled VALUES (NEW.tokens - OLD.tokens);"
    };
    for sql in [
        "CREATE TABLE public.settled (delta float8 NOT NULL)".to_owned(),
        format!(
            "CREATE FUNCTION public.watch_settlement() RETURNS trigger LANGUAGE plpgsql
                 SECURITY DEFINER AS $$
             BEGIN
                 IF NEW.refilled_at = OLD.refilled_at THEN {body} END IF;
                 RETURN NEW;
             END $$"
        ),
        "CREATE TRIGGER watch_settlement BEFORE UPDATE ON connectors.provider_budgets
             FOR EACH ROW EXECUTE FUNCTION public.watch_settlement()"
            .to_owned(),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&kit.db)
            .await
            .unwrap();
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_gives_back_what_its_push_did_not_spend(db: PgPool) {
    // campbooks-cloud#252: twelve archives, one batch Gmail takes. The run
    // reserved the batch, both profiles and four history pages, and spent
    // less: the difference goes back, once.
    let kit = Kit::new(db).await;
    let twelve: Vec<&[&str]> = vec![&["INBOX"]; 12];
    let m = mailbox(&kit, &twelve).await;
    watch_settlements(&kit, false).await;
    let items: Vec<(&str, Value)> = m
        .ids
        .iter()
        .map(|id| (id.as_str(), json!({"kind": "archive"})))
        .collect();
    m.request(&kit, &items).await;
    let settled: Vec<f64> = sqlx::query_scalar("SELECT delta FROM public.settled")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert_eq!(settled.len(), 1, "{settled:?}");
    // Reserved 60 for its writes; spent the batch (50), a profile after it
    // and at least a profile or a history page before it.
    assert!(settled[0] > 0.0 && settled[0] <= 8.0, "{settled:?}");
    assert!(ends(&kit).await.iter().all(|e| e.0 == "applied"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_push_is_recorded_even_when_its_budget_cannot_be_settled(db: PgPool) {
    // What Gmail now holds is recorded before the budget is settled: a
    // settlement that fails never makes the run push the same changes again.
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"], &["INBOX"]]).await;
    watch_settlements(&kit, true).await;
    kit.google.clear_calls();
    let items: Vec<(&str, Value)> = m
        .ids
        .iter()
        .map(|id| (id.as_str(), json!({"kind": "archive"})))
        .collect();
    m.request(&kit, &items).await;
    assert_eq!(
        ends(&kit).await,
        [end("applied", None), end("applied", None)]
    );
    assert_eq!(kit.google.writes().len(), 2, "{:?}", kit.google.writes());
    for id in &m.ids {
        assert!(kit.google.labels_of(id).is_empty());
    }
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
    // One summary of the push; the quiet run made none.
    let made = tracked(&kit, "connectors.outbound_changes_made").await;
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].body["applied"], 2);
    assert_eq!(made[0].body["failed"], 1);
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
    kit.bind_planted_changes(m.ws).await;
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
    kit.bind_planted_changes(m.ws).await;
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
    kit.bind_planted_changes(m.ws).await;
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
    kit.bind_planted_changes(m.ws).await;
    let binding = &kit.state.binding;
    let (connection, mailbox) = kit.at_of(m.stream).await;
    let at = crate::domain::outbound::At {
        connection,
        mailbox: &mailbox,
    };
    let fresh = crate::domain::outbound::backlog(
        &kit.state.db,
        binding,
        m.ws,
        (m.stream, at),
        kit.clock.now(),
    )
    .await
    .unwrap();
    assert_eq!(own(fresh), None);
    let later = kit.clock.now() + TimeDelta::minutes(11);
    let stale =
        crate::domain::outbound::backlog(&kit.state.db, binding, m.ws, (m.stream, at), later)
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

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_change_request_changes_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"], &["INBOX"]]).await;
    kit.google.clear_calls();
    // Appended in the workspace, but no conversation shows these changes.
    m.publish(
        &kit,
        uuid::Uuid::now_v7(),
        &[
            (m.ids[0].as_str(), json!({"kind": "archive"})),
            (m.ids[1].as_str(), json!({"kind": "mark_unread"})),
        ],
    )
    .await;
    // Nothing recorded, and not even the history stamp asked for.
    assert!(outbound(&kit).await.is_empty());
    assert!(
        !kit.google
            .calls()
            .iter()
            .any(|call| call.contains("/profile")),
        "{:?}",
        kit.google.calls()
    );
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["INBOX", "UNREAD"])
    );
    assert_eq!(kit.google.labels_of(&m.ids[1]), labels(&["INBOX"]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_changes_a_conversation_shows_are_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "UNREAD"], &["INBOX", "UNREAD"]]).await;
    // The conversation shows the first message read; the second was smuggled
    // into the request.
    let conversation = uuid::Uuid::now_v7();
    kit.outbound.show(conversation, &[m.ids[0].as_str()]);
    m.publish(
        &kit,
        conversation,
        &[
            (m.ids[0].as_str(), json!({"kind": "mark_read"})),
            (m.ids[1].as_str(), json!({"kind": "mark_read"})),
        ],
    )
    .await;
    m.run(&kit).await;
    let rows = outbound(&kit).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, m.ids[0]);
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(
        kit.google.labels_of(&m.ids[1]),
        labels(&["INBOX", "UNREAD"])
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_and_spam_reach_gmail_and_come_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = mailbox(
        &kit,
        &[
            &["INBOX", "UNREAD"],
            &["CATEGORY_UPDATES"],
            &["SPAM", "UNREAD"],
            &["SENT"],
        ],
    )
    .await;
    let (inboxed, archived) = (&mailbox.ids[0], &mailbox.ids[1]);
    let (flagged, sent) = (&mailbox.ids[2], &mailbox.ids[3]);
    let before = changes(&kit, mailbox.ws).await.len();
    let step = |id: &str, change: Value| {
        let id = id.to_owned();
        let mailbox = &mailbox;
        let kit = &kit;
        async move {
            mailbox.request(kit, &[(id.as_str(), change)]).await;
        }
    };

    // Trash and back: Gmail's own calls, its labels kept.
    step(inboxed, json!({"kind": "trash"})).await;
    assert_eq!(
        kit.google.labels_of(inboxed),
        labels(&["INBOX", "TRASH", "UNREAD"])
    );
    step(inboxed, json!({"kind": "untrash", "inbound": true})).await;
    assert_eq!(kit.google.labels_of(inboxed), labels(&["INBOX", "UNREAD"]));
    let writes = kit.google.writes();
    assert!(writes.iter().any(|w| w.ends_with("/trash")), "{writes:?}");
    assert!(writes.iter().any(|w| w.ends_with("/untrash")), "{writes:?}");

    // Reported, then not spam: received mail back in the inbox, as Gmail's
    // own button does, also mail Gmail flagged itself.
    step(inboxed, json!({"kind": "spam"})).await;
    assert_eq!(kit.google.labels_of(inboxed), labels(&["SPAM", "UNREAD"]));
    step(inboxed, json!({"kind": "unspam", "inbound": true})).await;
    assert_eq!(kit.google.labels_of(inboxed), labels(&["INBOX", "UNREAD"]));
    step(flagged, json!({"kind": "unspam", "inbound": true})).await;
    assert_eq!(kit.google.labels_of(flagged), labels(&["INBOX", "UNREAD"]));
    // The member's own mail is never put in the inbox.
    step(sent, json!({"kind": "spam"})).await;
    step(sent, json!({"kind": "unspam", "inbound": false})).await;
    assert_eq!(kit.google.labels_of(sent), labels(&["SENT"]));

    // Trash and spam exclude each other.
    step(archived, json!({"kind": "spam"})).await;
    step(archived, json!({"kind": "trash"})).await;
    assert_eq!(
        kit.google.labels_of(archived),
        labels(&["CATEGORY_UPDATES", "TRASH"])
    );

    assert!(ends(&kit).await.iter().all(|e| *e == end("applied", None)));
    // The writes come back as nothing.
    mailbox.run(&kit).await;
    assert_eq!(changes(&kit, mailbox.ws).await.len(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_report_untrashes_only_mail_in_the_trash(db: PgPool) {
    let kit = Kit::new(db).await;
    let mailbox = mailbox(&kit, &[&["INBOX", "UNREAD"], &["TRASH"]]).await;
    let (inboxed, trashed) = (&mailbox.ids[0], &mailbox.ids[1]);

    // Not in the trash: a label change, no untrash.
    kit.google.clear_calls();
    mailbox
        .request(&kit, &[(inboxed.as_str(), json!({"kind": "spam"}))])
        .await;
    let writes = kit.google.writes();
    assert!(!writes.iter().any(|w| w.contains("/untrash")), "{writes:?}");
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(kit.google.labels_of(inboxed), labels(&["SPAM", "UNREAD"]));

    // In the trash: untrashed first, then reported.
    kit.google.clear_calls();
    mailbox
        .request(&kit, &[(trashed.as_str(), json!({"kind": "spam"}))])
        .await;
    let writes = kit.google.writes();
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert!(
        writes[0].contains(&format!("{trashed}/untrash")),
        "{writes:?}"
    );
    assert!(
        writes[1].contains(&format!("{trashed}/modify")),
        "{writes:?}"
    );
    assert_eq!(kit.google.labels_of(trashed), labels(&["SPAM"]));
    assert!(ends(&kit).await.iter().all(|e| *e == end("applied", None)));
}

/// campbooks-cloud#227: a run makes only the changes `request_changes`
/// recorded and bound. A change planted by SQL in the workspace, or a
/// member's change edited after it was recorded, is never made: it ends
/// `skipped` (`unverified`) and Gmail is not written.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_planted_or_edited_outbound_change_is_never_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"], &["INBOX"]]).await;
    m.pause(&kit, true).await;
    // An archive no member asked for, written as the runtime role.
    let mut tx = kit.state.db.begin_for(m.ws).await.unwrap();
    sqlx::query(
        "INSERT INTO connectors.outbound_changes
             (id, workspace_id, stream_id, request_id, provider_message_id, kind, remove_labels)
         VALUES ($1, $2, $3, $4, $5, 'archive', '{INBOX}')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(*m.ws.as_uuid())
    .bind(m.stream)
    .bind(uuid::Uuid::now_v7())
    .bind(&m.ids[0])
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // A member's star, then turned into an archive.
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "star"}))])
        .await;
    let mut tx = kit.state.db.begin_for(m.ws).await.unwrap();
    sqlx::query(
        "UPDATE connectors.outbound_changes
            SET kind = 'archive', add_labels = '{}', remove_labels = '{INBOX}'
          WHERE provider_message_id = $1",
    )
    .bind(&m.ids[1])
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    m.pause(&kit, false).await;
    kit.google.clear_calls();
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(kit.google.labels_of(&m.ids[1]), labels(&["INBOX"]));
    assert_eq!(
        ends(&kit).await,
        [
            end("skipped", Some("unverified")),
            end("skipped", Some("unverified"))
        ]
    );

    // A member's own change, as recorded, is made.
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "star"}))])
        .await;
    assert_eq!(
        kit.google.labels_of(&m.ids[1]),
        labels(&["INBOX", "STARRED"])
    );
}

/// campbooks-cloud#262: a change recorded under the old instance secret
/// key and still pending (its stream paused) is re-tagged while the old key
/// is the previous one, and made once only the new key is left.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_pending_change_retagged_after_a_rotation_is_made_without_the_old_key(db: PgPool) {
    use campbooks_kernel::{
        binding::Binding,
        crypto::{SECRET_KEY_LEN, SecretKey},
    };

    let mut kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "star"}))])
        .await;
    // The kit's key (`testing::binding`) is the old one.
    let old = SecretKey::from_bytes([0x5a; SECRET_KEY_LEN]);
    let new = SecretKey::from_bytes([0x6b; SECRET_KEY_LEN]);
    let rotated = Binding::new(&new.clone().with_previous(&old));

    let mut tx = kit.state.db.begin_for(m.ws).await.unwrap();
    let (retagged, left) = crate::domain::outbound::retag(&mut tx, &rotated, m.ws)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!((retagged, left), (1, 0));
    let mut tx = kit.state.db.begin_for(m.ws).await.unwrap();
    assert_eq!(
        crate::domain::outbound::retag(&mut tx, &rotated, m.ws)
            .await
            .unwrap(),
        (0, 0),
        "nothing left"
    );
    tx.commit().await.unwrap();

    // The previous key removed: the change verifies under the new key alone.
    kit.state = kit.state.clone().with_binding(Binding::new(&new));
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["INBOX", "STARRED"])
    );
    assert!(ends(&kit).await.iter().all(|e| *e == end("applied", None)));
}

/// The labels of `id` at Gmail, the Receipts mark named as such.
fn shown(kit: &Kit, id: &str) -> Vec<String> {
    let mark = kit.google.label_named("Campbooks/Receipts");
    let mut shown: Vec<String> = kit
        .google
        .labels_of(id)
        .into_iter()
        .map(|l| {
            if Some(&l) == mark.as_ref() {
                "Campbooks/Receipts".to_owned()
            } else {
                l
            }
        })
        .collect();
    shown.sort();
    shown
}

/// Gmail's `labels.create` calls seen.
fn creations(kit: &Kit) -> usize {
    kit.google
        .calls()
        .iter()
        .filter(|c| *c == "POST /gmail/v1/users/me/labels")
        .count()
}

/// The provider streams the batches since the `from`th carry for `id`.
async fn reported(kit: &Kit, ws: WorkspaceId, from: usize, id: &str) -> Vec<Value> {
    changes(kit, ws).await[from..]
        .iter()
        .filter(|c| c["message"]["provider_message_id"] == id)
        .map(|c| c["message"]["provider_stream"].clone())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn receipts_reach_gmail_through_its_mark(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_PROMOTIONS"]]).await;
    let id = m.ids[0].as_str();
    let start = changes(&kit, m.ws).await.len();
    kit.google.clear_calls();
    assert!(kit.google.label_named("Campbooks/Receipts").is_none());

    // The mark is created on first use: the Updates tab and the mark.
    m.request(
        &kit,
        &[(id, json!({"kind": "move_to_stream", "stream": "receipts"}))],
    )
    .await;
    assert_eq!(creations(&kit), 1);
    assert_eq!(
        shown(&kit, id),
        labels(&["CATEGORY_UPDATES", "Campbooks/Receipts", "INBOX"])
    );
    assert_eq!(ends(&kit).await, [end("applied", None)]);
    // Reported once, from the index; then nothing: no batch, no change.
    assert_eq!(reported(&kit, m.ws, start, id).await, [json!("receipts")]);
    let after = changes(&kit, m.ws).await.len();
    m.run(&kit).await;
    m.run(&kit).await;
    assert_eq!(changes(&kit, m.ws).await.len(), after);
    assert_eq!(outbound(&kit).await.len(), 1);
    // (Calls were cleared above: no download since.)
    assert_eq!(kit.google.raw_fetches(id), 0, "never downloaded again");

    // Out of Receipts: the mark and the Updates tab off, Primary on.
    m.request(
        &kit,
        &[(id, json!({"kind": "move_to_stream", "stream": "primary"}))],
    )
    .await;
    assert_eq!(shown(&kit, id), labels(&["CATEGORY_PERSONAL", "INBOX"]));

    // Back to Receipts: the mark is found, not made again.
    m.request(
        &kit,
        &[(id, json!({"kind": "move_to_stream", "stream": "receipts"}))],
    )
    .await;
    assert_eq!(creations(&kit), 1);
    assert_eq!(
        shown(&kit, id),
        labels(&["CATEGORY_UPDATES", "Campbooks/Receipts", "INBOX"])
    );
    assert!(ends(&kit).await.iter().all(|e| *e == end("applied", None)));
    assert_eq!(
        reported(&kit, m.ws, start, id).await,
        [json!("receipts"), json!("primary"), json!("receipts")]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stream_write_is_reported_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_PROMOTIONS"], &["INBOX"]]).await;
    let start = changes(&kit, m.ws).await.len();
    m.request(
        &kit,
        &[(
            m.ids[0].as_str(),
            json!({"kind": "move_to_stream", "stream": "primary"}),
        )],
    )
    .await;
    // A write that changes no stream is never reported.
    m.request(&kit, &[(m.ids[1].as_str(), json!({"kind": "star"}))])
        .await;
    let all = changes(&kit, m.ws).await;
    assert_eq!(all.len(), start + 1, "{:#?}", &all[start..]);
    let message = &all[start]["message"];
    assert_eq!(message["provider_message_id"], m.ids[0].as_str());
    assert_eq!(message["provider_stream"], "primary");
    assert!(message.get("stream_mark_stale").is_none());
    m.run(&kit).await;
    assert_eq!(changes(&kit, m.ws).await.len(), start + 1);
    assert_eq!(kit.google.raw_fetches(&m.ids[0]), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_revert_in_gmail_after_a_stream_write_comes_back(db: PgPool) {
    // Scout's sort is written (Promotions to Primary), then the member
    // drags the message back to Promotions in Gmail: the batches say
    // `primary`, then `newsletters`, the second with no stale mark.
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_PROMOTIONS"]]).await;
    let id = m.ids[0].as_str();
    let start = changes(&kit, m.ws).await.len();
    m.request(
        &kit,
        &[(id, json!({"kind": "move_to_stream", "stream": "primary"}))],
    )
    .await;
    kit.google
        .relabel(id, &["CATEGORY_PROMOTIONS"], &["CATEGORY_PERSONAL"]);
    m.run(&kit).await;
    assert_eq!(
        reported(&kit, m.ws, start, id).await,
        [json!("primary"), json!("newsletters")]
    );
    let last = changes(&kit, m.ws).await.pop().unwrap();
    assert!(last["message"].get("stream_mark_stale").is_none());
    assert_eq!(outbound(&kit).await.len(), 1, "nothing written back");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mark_gmail_already_has_is_adopted(db: PgPool) {
    let kit = Kit::new(db).await;
    // The member made the label in Gmail after the last label listing.
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    let made = kit.google.add_label("campbooks/receipts");
    m.request(
        &kit,
        &[(
            m.ids[0].as_str(),
            json!({"kind": "move_to_stream", "stream": "receipts"}),
        )],
    )
    .await;
    assert!(kit.google.labels_of(&m.ids[0]).contains(&made));
    assert_eq!(ends(&kit).await, [end("applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_already_in_place_makes_no_call(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_UPDATES"]]).await;
    kit.google.clear_calls();
    m.request(
        &kit,
        &[(
            m.ids[0].as_str(),
            json!({"kind": "move_to_stream", "stream": "updates"}),
        )],
    )
    .await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(ends(&kit).await, [end("skipped", Some("already_set"))]);
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["CATEGORY_UPDATES", "INBOX"])
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_gmail_made_since_the_last_read_is_not_taken_for_in_place(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX", "CATEGORY_UPDATES"]]).await;
    // Moved to Promotions in Gmail; Campbooks has not read it yet when a
    // move back to Updates is asked.
    kit.google
        .relabel(&m.ids[0], &["CATEGORY_PROMOTIONS"], &["CATEGORY_UPDATES"]);
    m.pause(&kit, true).await;
    m.request(
        &kit,
        &[(
            m.ids[0].as_str(),
            json!({"kind": "move_to_stream", "stream": "updates"}),
        )],
    )
    .await;
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(
        kit.google.labels_of(&m.ids[0]),
        labels(&["CATEGORY_UPDATES", "INBOX"])
    );
    assert_eq!(ends(&kit).await, [end("applied", None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn gmail_says_where_a_message_is_by_its_tab_or_its_mark(db: PgPool) {
    let kit = Kit::new(db).await;
    let mark = kit.google.add_label("Campbooks/Receipts");
    let m = mailbox(
        &kit,
        &[
            &["INBOX", "CATEGORY_UPDATES", mark.as_str(), "Label_7"],
            &["INBOX", "CATEGORY_UPDATES"],
        ],
    )
    .await;
    for batch in kit.batches(m.ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
    }
    let latest = |all: &[Value], id: &str| -> Value {
        all.iter()
            .rev()
            .find(|c| c["message"]["provider_message_id"] == id)
            .unwrap()["message"]
            .clone()
    };
    let all = changes(&kit, m.ws).await;
    // A mark is not a label: it says the stream.
    let receipt = latest(&all, &m.ids[0]);
    assert_eq!(
        receipt["labels"],
        json!([{"key": "Label_7", "name": "Clients"}])
    );
    assert_eq!(receipt["provider_stream"], "receipts");
    assert!(receipt.get("stream_mark_stale").is_none());
    assert_eq!(latest(&all, &m.ids[1])["provider_stream"], "updates");

    // Dragged to Promotions with its mark kept: the tab wins, the mark is
    // stale. Moved from Updates to Primary: Primary. Neither downloaded
    // again.
    kit.google
        .relabel(&m.ids[0], &["CATEGORY_PROMOTIONS"], &["CATEGORY_UPDATES"]);
    kit.google
        .relabel(&m.ids[1], &["CATEGORY_PERSONAL"], &["CATEGORY_UPDATES"]);
    let start = changes(&kit, m.ws).await.len();
    m.run(&kit).await;
    let all = changes(&kit, m.ws).await;
    for batch in kit.batches(m.ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
    }
    let dragged = latest(&all[start..], &m.ids[0]);
    assert_eq!(dragged["provider_stream"], "newsletters");
    assert_eq!(dragged["stream_mark_stale"], true);
    assert_eq!(dragged["labels"].as_array().unwrap().len(), 1);
    let tab = latest(&all[start..], &m.ids[1]);
    assert_eq!(tab["provider_stream"], "primary");
    assert!(tab.get("stream_mark_stale").is_none());
    for id in &m.ids {
        assert_eq!(kit.google.raw_fetches(id), 1, "downloaded once");
    }
}

/// Point `m`'s stream at another mailbox of its connection, as SQL in the
/// workspace could (campbooks-cloud#242).
async fn repoint(kit: &Kit, m: &Mailbox, mailbox: &str) {
    sqlx::query("UPDATE connectors.streams SET mailbox_key = $2 WHERE id = $1")
        .bind(m.stream)
        .bind(mailbox)
        .execute(&kit.db)
        .await
        .unwrap();
}

/// campbooks-cloud#242: a member's change is bound to where it is made, the
/// stream's connection and mailbox when it was recorded. The stream pointed
/// at another mailbox by SQL after that: the change ends `skipped`
/// (`unverified`) and nothing is written at the provider. A change recorded
/// once the stream points there is the member's, and is made.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_whose_stream_was_repointed_after_it_was_recorded_is_never_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"]]).await;
    m.pause(&kit, true).await;
    m.request(&kit, &[(m.ids[0].as_str(), json!({"kind": "archive"}))])
        .await;
    repoint(&kit, &m, "another-mailbox").await;
    m.pause(&kit, false).await;
    kit.google.clear_calls();
    m.run(&kit).await;
    assert!(kit.google.writes().is_empty(), "{:?}", kit.google.writes());
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));
    assert_eq!(ends(&kit).await, [end("skipped", Some("unverified"))]);
}

/// The member stars message `n` of `m` while its stream is paused, and the
/// change is bound as a version before destinations were bound did.
async fn star_as_before(kit: &Kit, m: &Mailbox, n: usize) {
    m.pause(kit, true).await;
    m.request(kit, &[(m.ids[n].as_str(), json!({"kind": "star"}))])
        .await;
    let ids: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM connectors.outbound_changes WHERE state = 'pending'")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    let mut tx = kit.state.db.begin_for(m.ws).await.unwrap();
    crate::domain::outbound::bind_before_destinations(&mut tx, &kit.state.binding, m.ws, &ids)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// A change recorded before destinations were bound (its MAC the `…/v1`
/// encoding) is made only while its stream goes where it went at that
/// deploy (`connectors.destinations_at_deploy`, which only the migration
/// writes): not without that record, and not once the stream was
/// repointed.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_recorded_before_destinations_is_made_only_where_its_stream_went_then(db: PgPool) {
    let kit = Kit::new(db).await;
    let m = mailbox(&kit, &[&["INBOX"], &["INBOX"], &["INBOX"]]).await;
    // No record of the deploy: refused.
    star_as_before(&kit, &m, 0).await;
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(kit.google.labels_of(&m.ids[0]), labels(&["INBOX"]));

    // Recorded at the deploy as it still is: made.
    let (connection, mailbox) = kit.at_of(m.stream).await;
    sqlx::query(
        "INSERT INTO connectors.destinations_at_deploy
             (stream_id, workspace_id, inbox_id, connection_id, mailbox_key)
         VALUES ($1, $2, $3::uuid, $4, $5)",
    )
    .bind(m.stream)
    .bind(*m.ws.as_uuid())
    .bind(&m.inbox)
    .bind(connection)
    .bind(&mailbox)
    .execute(&kit.db)
    .await
    .unwrap();
    star_as_before(&kit, &m, 1).await;
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(
        kit.google.labels_of(&m.ids[1]),
        labels(&["INBOX", "STARRED"])
    );

    // Repointed since the deploy: refused.
    star_as_before(&kit, &m, 2).await;
    repoint(&kit, &m, "another-mailbox").await;
    m.pause(&kit, false).await;
    m.run(&kit).await;
    assert_eq!(kit.google.labels_of(&m.ids[2]), labels(&["INBOX"]));
    assert_eq!(
        ends(&kit).await,
        [
            end("skipped", Some("unverified")),
            end("applied", None),
            end("skipped", Some("unverified")),
        ]
    );
}
