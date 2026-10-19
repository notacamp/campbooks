//! Waiting on someone and Nudge (campbooks-cloud#293), end to end: Joao asks
//! Ana Costa to send Sam the floor plan, copying Sam's inbox.

use std::sync::Arc;

use axum::http::StatusCode;
use campbooks_ai::decide::{AnswerValue, signals::moves::question::NEXT_MOVE};
use campbooks_kernel::{
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    domain::needs::{NeedSettings, after_working_days},
    features::wake_needs::DispatchNeedClocks,
    testkit::{
        ANA, JOAO, Kit, SAM, Scripted, contract, joao_asks_ana, mail, script, someone_else_moves,
        waiting_deps,
    },
};

/// Results a receiving provider gives mail whose From is forged.
const FORGED: &str =
    "mx.example; dkim=fail header.i=@studio.example; dmarc=fail header.from=studio.example";

/// [`mail`] with another `subject` and, when given, the receiving
/// provider's `results`.
fn mail_with(base: Value, subject: &str, results: Option<&str>) -> Value {
    let mut value = base;
    let message = &mut value["changes"][0]["message"];
    message["subject"] = json!(subject);
    if let Some(results) = results {
        message["headers"] = json!({"authentication_results": results});
    }
    value
}

async fn summary(kit: &Kit, token: &str, workspace: WorkspaceId) -> Value {
    let id = kit.conversation_of("joao-1").await;
    let list = kit
        .get(
            token,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    list.json["conversations"][0].clone()
}

/// `(reason, trust, state, cleared_because, nudges)` of Joao's
/// conversation's need.
async fn need(kit: &Kit) -> (String, String, String, Option<String>, i32) {
    let conversation = kit.conversation_of("joao-1").await;
    sqlx::query_as(
        "SELECT reason, trust, state, cleared_because, nudges FROM conversations.needs
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

async fn sent_at(kit: &Kit, provider_message_id: &str) -> DateTime<Utc> {
    sqlx::query_scalar("SELECT sent_at FROM conversations.messages WHERE provider_message_id = $1")
        .bind(provider_message_id)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

/// Joao's message sent `days` days ago, its clock due now, and a tick of
/// the scheduler.
async fn days_later(kit: &Kit, days: i32) {
    sqlx::query(
        "UPDATE conversations.messages SET sent_at = now() - make_interval(days => $1)
          WHERE provider_message_id = 'joao-1'",
    )
    .bind(days)
    .execute(&kit.db)
    .await
    .unwrap();
    let conversation = kit.conversation_of("joao-1").await;
    sqlx::query(
        "UPDATE conversations.need_clocks SET wake_at = now() - interval '1 minute'
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .execute(&kit.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchNeedClocks, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn ana_was_asked_to_write_so_the_conversation_waits_on_ana(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;

    let (reason, trust, state, _, nudges) = need(&kit).await;
    assert_eq!(
        (reason.as_str(), trust.as_str(), state.as_str(), nudges),
        ("waiting", "check", "open", 0)
    );
    let joao = summary(&kit, &sam, workspace).await;
    let need_view = &joao["need"];
    assert_eq!(need_view["reason"], "waiting");
    assert_eq!(need_view["question"], "next_move");
    assert_eq!(need_view["waiting_on"]["name"], ANA.0);
    assert_eq!(need_view["waiting_on"]["address"], ANA.1);
    assert_eq!(need_view["nudges"], 0);
    // Due when Scout nudges: three working days after the message.
    let nudge_at = after_working_days(sent_at(&kit, "joao-1").await, 3);
    assert_eq!(
        need_view["due_on"],
        json!(nudge_at.date_naive().to_string())
    );
    // The clock wakes it then.
    let conversation = kit.conversation_of("joao-1").await;
    let wake: DateTime<Utc> = sqlx::query_scalar(
        "SELECT wake_at FROM conversations.need_clocks WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(wake, nudge_at);

    // The event follows the contract and names nobody.
    let (schemas, index) = contract("conversations.need_changed.v1.schema.json");
    let events = kit.events("conversations.need_changed").await;
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["need"]["reason"], "waiting");
    assert_eq!(events[0]["need"]["trust"], "check");
    for event in &events {
        if let Err(error) = schemas.validate(event, index) {
            panic!("{event} breaks the contract: {error}");
        }
        let text = event.to_string();
        for personal in ["Ana", "ana@", "Joao", "joao@", "floor plan"] {
            assert!(!text.contains(personal), "{personal} in {text}");
        }
    }
    // The stored answer keeps the option it named, as it was asked.
    let options: Value = sqlx::query_scalar(
        "SELECT options FROM conversations.decisions WHERE question = 'awaited'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(options[0]["label"], "p1");
    assert_eq!(
        options[0]["description"],
        format!("{} <{}> (To)", ANA.0, ANA.1)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn archiving_leaves_the_wait_and_the_clock_turns_it_into_a_nudge(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;
    let id = kit.conversation_of("joao-1").await;
    let archived = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/archive"),
            None,
        )
        .await;
    assert!(archived.status.is_success(), "{:?}", archived.json);
    assert_eq!(need(&kit).await.0, "waiting");

    // Six days on (three working days, whatever the weekday), and Ana has
    // not written: a Nudge, overdue.
    days_later(&kit, 6).await;
    let (reason, _, state, _, _) = need(&kit).await;
    assert_eq!((reason.as_str(), state.as_str()), ("nudge", "open"));
    let events = kit.events("conversations.need_changed").await;
    assert_eq!(events.last().unwrap()["need"]["reason"], "nudge");
    let joao = summary(&kit, &sam, workspace).await;
    assert_eq!(joao["need"]["reason"], "nudge");
    assert_eq!(joao["need"]["overdue"], true);
    // Nothing more to wake for: a Nudge stays until it is answered.
    let conversation = kit.conversation_of("joao-1").await;
    let clock: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT wake_at FROM conversations.need_clocks WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_optional(&kit.db)
    .await
    .unwrap();
    assert_eq!(clock, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn writing_to_ana_arms_the_wait_again_and_ana_writing_ends_it(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, inbox, sam) = joao_asks_ana(&kit).await;
    days_later(&kit, 6).await;
    assert_eq!(need(&kit).await.0, "nudge");

    // Sam nudges Ana, in the same thread, Joao copied: waiting again.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("sam-1", "thr-floor-plan", SAM, &[ANA], &[JOAO], 0),
    )
    .await;
    let (reason, _, state, _, nudges) = need(&kit).await;
    assert_eq!(
        (reason.as_str(), state.as_str(), nudges),
        ("waiting", "open", 1)
    );
    let nudge_at = after_working_days(sent_at(&kit, "sam-1").await, 3);
    let joao = summary(&kit, &sam, workspace).await;
    assert_eq!(joao["need"]["nudges"], 1);
    assert_eq!(
        joao["need"]["due_on"],
        json!(nudge_at.date_naive().to_string())
    );
    // A reply to Joao alone is not a nudge.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("sam-2", "thr-floor-plan", SAM, &[JOAO], &[], 0),
    )
    .await;
    assert_eq!(need(&kit).await.4, 1);

    // Ana writes in a new thread: heard back.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("ana-1", "thr-new", ANA, &[SAM], &[], 0),
    )
    .await;
    let (_, _, state, cleared, _) = need(&kit).await;
    assert_eq!(
        (state.as_str(), cleared.as_deref()),
        ("cleared", Some("heard_back"))
    );
    let joao = summary(&kit, &sam, workspace).await;
    assert_eq!(joao["need"], Value::Null);
    let events = kit.events("conversations.need_changed").await;
    let last = events.last().unwrap();
    assert_eq!(last["cleared_because"], "heard_back");
    assert_eq!(last["need"], Value::Null);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_from_someone_else_or_another_inbox_is_not_hearing_back(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, inbox, _) = joao_asks_ana(&kit).await;
    // Ana writes to another inbox of the workspace.
    let (_, eve) = kit.member("eve", workspace);
    let other = kit
        .inbox(
            workspace,
            "eve@example.com",
            &[(eve, crate::model::InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        other,
        &mail(
            "ana-elsewhere",
            "thr-eve",
            ANA,
            &[("Eve", "eve@example.com")],
            &[],
            0,
        ),
    )
    .await;
    assert_eq!(need(&kit).await.2, "open");
    // Mail sent before Joao's: not hearing back either.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("ana-before", "thr-old", ANA, &[SAM], &[], 120),
    )
    .await;
    assert_eq!(need(&kit).await.2, "open");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn heard_from_them_and_not_waiting(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;
    let id = kit.conversation_of("joao-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}/need");
    let seen = kit.message_of("joao-1").await;

    // Heard from them (Done): cleared, nothing filed.
    let done = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "done", "message_id": seen})),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert_eq!(done.json["need"], Value::Null);
    let back = kit.post(&sam, &format!("{base}/reopen"), None).await;
    assert_eq!(back.json["need"]["reason"], "waiting");

    // Not waiting: a correction of the model's move answer.
    let not = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": seen})),
        )
        .await;
    assert_eq!(not.status, StatusCode::OK, "{:?}", not.json);
    let (question, from, to): (String, String, String) =
        sqlx::query_as("SELECT question, from_value, to_value FROM conversations.corrections")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        (question.as_str(), from.as_str(), to.as_str()),
        ("next_move", "someone_else", "not_waiting")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_newer_message_that_waits_on_nobody_ends_the_wait_as_superseded(db: PgPool) {
    // Joao's "never mind, I sent it myself": nobody has the next move.
    let mut nobody = script("primary", false, false, false, 0.0, false);
    nobody.push((NEXT_MOVE, AnswerValue::Choice("nobody".to_owned()), 0.91));
    let model = Scripted::model([(JOAO.1, someone_else_moves())]).followup(
        JOAO.1,
        "Re: Floor plan",
        nobody,
    );
    let kit = Kit::with_deps(db, waiting_deps().with_model(Arc::new(model))).await;
    let (workspace, inbox, _) = joao_asks_ana(&kit).await;
    assert_eq!(need(&kit).await.0, "waiting");

    kit.ingest_value(
        workspace,
        inbox,
        &mail_with(
            mail("joao-2", "thr-floor-plan", JOAO, &[ANA], &[SAM], 0),
            "Re: Floor plan",
            None,
        ),
    )
    .await;
    let (_, _, state, cleared, _) = need(&kit).await;
    assert_eq!(
        (state.as_str(), cleared.as_deref()),
        ("cleared", Some("superseded"))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_from_ana_that_fails_its_sender_checks_is_not_hearing_back(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, inbox, _) = joao_asks_ana(&kit).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail_with(
            mail("ana-forged", "thr-forged", ANA, &[SAM], &[], 0),
            "Floor plan",
            Some(FORGED),
        ),
    )
    .await;
    assert_eq!(need(&kit).await.2, "open");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_to_ana_long_after_does_not_arm_the_wait_again(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, inbox, _) = joao_asks_ana(&kit).await;
    // Joao asked 90 days ago: a nudge since.
    days_later(&kit, 90).await;
    assert_eq!(need(&kit).await.0, "nudge");
    // Sam writes to Ana now, past the 60 days the scan reads: not a nudge.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("sam-late", "thr-floor-plan", SAM, &[ANA], &[JOAO], 0),
    )
    .await;
    let (reason, _, _, _, nudges) = need(&kit).await;
    assert_eq!((reason.as_str(), nudges), ("nudge", 0));
}

/// Joao's conversation's open wait, made one on `address`.
async fn waiting_on(kit: &Kit, address: &str) {
    let conversation = kit.conversation_of("joao-1").await;
    sqlx::query(
        "UPDATE conversations.needs SET awaited_address = $2, awaited_name = 'Sam'
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .bind(address)
    .execute(&kit.db)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_wait_on_the_inbox_itself_is_never_nudged(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;
    let id = kit.conversation_of("joao-1").await;
    let message = kit.message_of("joao-1").await;
    // A wait stored on the inbox's own address, however it got there.
    waiting_on(&kit, &SAM.1.to_uppercase()).await;
    let draft = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(json!({"kind": "nudge", "message_id": message})),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CONFLICT, "{:?}", draft.json);
    assert_eq!(draft.json["code"], "not_waiting");
    let written = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/nudge/draft"),
            Some(json!({})),
        )
        .await;
    assert_eq!(written.status, StatusCode::CONFLICT, "{:?}", written.json);
    assert_eq!(written.json["code"], "not_waiting");
    let drafts: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.drafts")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(drafts, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_nudge_goes_where_the_server_addresses_it(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;
    let message = kit.message_of("joao-1").await;
    let draft = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(json!({
                "kind": "nudge",
                "message_id": message,
                "to": [{"name": "Eve", "address": "eve@elsewhere.example"}],
                "cc": [{"name": "Mallory", "address": "mallory@elsewhere.example"}],
                "body": "Hi Ana",
            })),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{:?}", draft.json);
    let addresses = |field: &str| -> Vec<String> {
        draft.json[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["address"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(addresses("to"), [ANA.1]);
    assert_eq!(addresses("cc"), [JOAO.1]);
    assert_eq!(draft.json["body"], "Hi Ana");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_waits_switched_off_an_open_wait_keeps_its_course(db: PgPool) {
    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, _, _) = joao_asks_ana(&kit).await;
    assert_eq!(need(&kit).await.2, "open");
    let conversation = kit.conversation_of("joao-1").await;
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    crate::domain::needs::refresh(
        &mut tx,
        &kit.state.events,
        &workspace.to_string(),
        conversation.parse().unwrap(),
        &NeedSettings::new(36_500, &waiting_deps().policy).with_waits(false),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (reason, _, state, _, _) = need(&kit).await;
    assert_eq!((reason.as_str(), state.as_str()), ("waiting", "open"));
}
