//! Scout's read of a message, end to end over `fixtures/needs.json` and a
//! few messages of our own, with a `complete` role on a mock server. The
//! scripted decide model flags the fixtures' mail as a good one would:
//! Marta's walkthrough and Pedro's lease have a deadline, the hosting
//! invoice asks for a payment, the look-alike mailbox warning is phishing,
//! and Jonas's lunch question asks for a reply and nothing else.

use axum::http::StatusCode;
use campbooks_ai::{AiConfig, Gateway, ProviderConfig, Role};
use campbooks_kernel::{
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use campbooks_kernel::{
    schedule::{TimeZoneFuture, TimeZones},
    tasks::{OpenTask as KernelTask, OpenTasks, OpenTasksFuture},
};

use super::*;
use crate::{
    ConversationsDeps,
    model::InboxRole,
    testkit::{Kit, Scripted, contract, dated, script, sorting_deps},
};

/// An instruct model answering with `content`.
async fn complete(content: &Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "read-test",
            "choices": [{"message": {"role": "assistant", "content": content.to_string()},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 300, "completion_tokens": 120}
        })))
        .mount(&server)
        .await;
    server
}

/// A summary in every language and nothing found in the dates.
fn good() -> Value {
    json!({
        "en": "Marta asks you to confirm Thursday's walkthrough by Wednesday.",
        "pt-PT": "A Marta pede-lhe que confirme a visita de quinta-feira até quarta.",
        "es-ES": "Marta le pide que confirme la visita del jueves antes del miércoles.",
        "fr": "Marta vous demande de confirmer la visite de jeudi avant mercredi.",
        "lapses_at": null, "due_on": null, "items": [], "asks": []
    })
}

/// The module with a `complete` role on `server`, and the scripted decide
/// model raising the fixtures' needs.
async fn kit(db: PgPool, server: &MockServer) -> Kit {
    kit_with(db, server, sorting_deps()).await
}

async fn kit_with(db: PgPool, server: &MockServer, deps: ConversationsDeps) -> Kit {
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "test");
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Complete, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap();
    Kit::with_deps(
        db,
        ConversationsDeps {
            ai: Some(gateway),
            ..deps
        },
    )
    .await
}

/// Sam's inbox holding `fixtures/needs.json`, and Leo, who is not in it.
async fn sams_needs(kit: &Kit) -> (WorkspaceId, InboxId, String, String) {
    let workspace = WorkspaceId::new();
    let (sam_token, sam) = kit.member("sam", workspace);
    let (leo_token, _) = kit.member("leo", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    (workspace, inbox, sam_token, leo_token)
}

async fn calls(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

/// `(provider message id, state, attempts)` of every model read.
async fn stored(kit: &Kit) -> Vec<(String, String, i16)> {
    sqlx::query_as(
        "SELECT m.provider_message_id, s.state, s.attempts
           FROM conversations.message_reads s
           JOIN conversations.messages m ON m.id = s.message_id
          WHERE s.layer = 'model' ORDER BY 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

/// The fixtures' mail the decide pass flags and that raises a need.
const FLAGGED: [&str; 3] = ["needs-1", "needs-2", "needs-3"];

async fn requeue(kit: &Kit, workspace: WorkspaceId, provider_message_id: &str) {
    let message = kit.message_of(provider_message_id).await.parse().unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    let _ = jobs::enqueue_to_in::<ReadMessage>(
        &mut tx,
        &workspace.to_string(),
        &ReadMessagePayload { message },
        EnqueueOptions::for_workspace(workspace.to_string()),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

async fn detail(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    provider: &str,
    lang: &str,
) -> Value {
    let id = kit.conversation_of(provider).await;
    let response = kit
        .get_in(
            token,
            &format!("/api/v1/w/{workspace}/conversations/{id}"),
            lang,
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
    response.json
}

/// `…/conversations/{c}/messages/{m}/read` of a fixture message.
async fn read_path(kit: &Kit, workspace: WorkspaceId, provider: &str) -> String {
    let conversation = kit.conversation_of(provider).await;
    let message = kit.message_of(provider).await;
    format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/read")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_flagged_need_gets_one_read_with_its_summary_in_every_language(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;

    // One call and one row per flagged message that raised a need, all read.
    let rows = stored(&kit).await;
    let ids: Vec<&str> = rows.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, FLAGGED);
    assert_eq!(calls(&server).await, rows.len());
    assert!(
        rows.iter()
            .all(|(_, state, attempts)| state == "read" && *attempts == 1)
    );
    // Announced once each, with ids only.
    let summarised = kit.events("conversations.need_summarised").await;
    assert_eq!(summarised.len(), rows.len());
    assert!(summarised[0].get("texts").is_none() && summarised[0].get("summary").is_none());
    let (schemas, index) = contract("conversations.message_read.v1.schema.json");
    let reads = kit.events("conversations.message_read").await;
    assert_eq!(reads.len(), rows.len());
    for event in &reads {
        if let Err(error) = schemas.validate(event, index) {
            panic!("{event} breaks the contract: {error}");
        }
        assert_eq!(event["layer"], "model");
        assert_eq!(event["summary_ready"], true);
        assert!(!event.to_string().contains("walkthrough"));
    }

    // Each caller reads their own language; English when theirs is missing.
    let english = detail(&kit, &sam, workspace, "needs-1", "en").await;
    assert_eq!(
        english["need_summary"],
        "Marta asks you to confirm Thursday's walkthrough by Wednesday."
    );
    let portuguese = detail(&kit, &sam, workspace, "needs-1", "pt-PT,pt;q=0.9").await;
    assert_eq!(
        portuguese["need_summary"],
        "A Marta pede-lhe que confirme a visita de quinta-feira até quarta."
    );
    let japanese = detail(&kit, &sam, workspace, "needs-1", "ja").await;
    assert_eq!(japanese["need_summary"], english["need_summary"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_plain_question_raises_no_read_and_opening_it_queues_none(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;

    // Jonas asks for a reply and nothing else: a Reply need, no flag.
    let need: String = sqlx::query_scalar(
        "SELECT reason FROM conversations.needs n JOIN conversations.messages m
             ON m.id = n.message_id WHERE m.provider_message_id = 'needs-5'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(need, "reply");
    assert!(!stored(&kit).await.iter().any(|(id, ..)| id == "needs-5"));
    let before = calls(&server).await;

    let path = read_path(&kit, workspace, "needs-5").await;
    let opened = kit.post(&sam, &path, None).await;
    assert_eq!(opened.status, StatusCode::OK, "{:?}", opened.json);
    assert_eq!(opened.json["outcome"], "not_flagged");
    kit.run_jobs().await;
    assert_eq!(calls(&server).await, before);
    let view = kit.get(&sam, &path).await;
    assert_eq!(view.json["status"], "none");
    assert_eq!(view.json["flagged"], false);
}

/// A booking-like email nobody needs to answer: the clinic's reminder, sent
/// an hour ago, in Updates.
fn clinic(text: &str) -> Value {
    json!({"changes": [{"op": "upsert", "message": {
        "provider_message_id": "clinic-1",
        "thread_key": "thr-clinic",
        "from": {"name": "Clínica Sol", "address": "recepcao@clinica-sol.example"},
        "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
        "subject": "A sua consulta",
        "sent_at": (Utc::now() - chrono::TimeDelta::hours(1)).to_rfc3339(),
        "direction": "inbound",
        "text": text,
        "mailbox": {"in_inbox": true},
        "read": false, "starred": false
    }}]})
}

/// The deps whose scripted decide model reads the clinic's reminder as a
/// dated thing of kind `appointment`, 0.9 sure.
fn clinic_deps() -> ConversationsDeps {
    sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(
        "recepcao@clinica-sol.example",
        dated(
            script("updates", false, false, false, 0.0, false),
            "appointment",
            0.9,
        ),
    )])))
}

/// `date` as a day of next month, far enough ahead to be in the future
/// whenever the test runs.
fn in_a_month() -> (String, String) {
    let day = (Utc::now() + chrono::TimeDelta::days(30)).date_naive();
    (
        day.format("%Y-%m-%d").to_string(),
        day.format("%d/%m/%Y").to_string(),
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_flagged_booking_is_read_when_a_member_opens_it(db: PgPool) {
    let (iso, written) = in_a_month();
    let quote = format!("A sua consulta é a {written} às 15:00");
    let answer = json!({
        "en": "", "pt-PT": "", "es-ES": "", "fr": "", "lapses_at": null, "due_on": null,
        "items": [{
            "kind": "appointment", "title": "Consulta na Clínica Sol",
            "starts": format!("{iso}T15:00:00+00:00"), "ends": format!("{iso}T15:30:00+00:00"),
            "all_day": false, "time_zone": "Europe/Lisbon", "place": "Rua do Sol 12",
            "quote": quote, "updates": null, "cancels": null
        }],
        "asks": []
    });
    let server = complete(&answer).await;
    let kit = kit_with(db, &server, clinic_deps()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &clinic(&format!("Bom dia. {quote}. Até breve.")),
    )
    .await;

    // No need, so no read on arrival; the pass only flagged it.
    assert!(stored(&kit).await.is_empty());
    assert_eq!(calls(&server).await, 0);
    let path = read_path(&kit, workspace, "clinic-1").await;
    let before = kit.get(&sam, &path).await;
    assert_eq!(before.status, StatusCode::OK, "{:?}", before.json);
    assert_eq!(before.json["status"], "none");
    assert_eq!(before.json["flagged"], true);
    assert_eq!(before.json["can_read"], true);
    // A model may read mail here, so no fallback kind is offered.
    assert!(before.json.get("dated_kind").is_none());

    // Opening it queues the read, once.
    let opened = kit.post(&sam, &path, None).await;
    assert_eq!(opened.json["outcome"], "queued");
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.message_reads WHERE state = 'queued'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    let again = kit.post(&sam, &path, None).await;
    assert_eq!(again.json["outcome"], "already_queued");
    kit.run_jobs().await;
    assert_eq!(
        calls(&server).await,
        1,
        "one call however often it is opened"
    );
    let rows = stored(&kit).await;
    assert_eq!(rows, [("clinic-1".to_owned(), "read".to_owned(), 1)]);

    // What was read, with its text, for the member.
    let after = kit.get(&sam, &path).await;
    assert_eq!(after.json["status"], "done");
    assert_eq!(after.json["layers"], json!(["model"]));
    let item = &after.json["items"][0];
    assert_eq!(item["kind"], "appointment");
    assert_eq!(item["title"], "Consulta na Clínica Sol");
    assert_eq!(item["place"], "Rua do Sol 12");
    assert_eq!(item["quote"], quote);
    assert_eq!(item["time_zone"], "Europe/Lisbon");
    assert_eq!(item["starts"], format!("{iso}T15:00:00+00:00"));
    let message = kit.message_of("clinic-1").await;
    assert_eq!(item["key"], format!("{message}:model:item:1"));
    let reopened = kit.post(&sam, &path, None).await;
    assert_eq!(reopened.json["outcome"], "read");
    kit.run_jobs().await;
    assert_eq!(calls(&server).await, 1);

    // The event names the item by key, kind and time, and no word of it.
    let events = kit.events("conversations.message_read").await;
    assert_eq!(events.len(), 1);
    let (schemas, index) = contract("conversations.message_read.v1.schema.json");
    schemas.validate(&events[0], index).unwrap();
    assert_eq!(events[0]["items"][0]["kind"], "appointment");
    assert_eq!(events[0]["summary_ready"], false);
    assert_eq!(events[0]["owner"], json!(sam_id));
    let text = events[0].to_string();
    for word in ["Consulta", "Rua do Sol", "Clínica", "15:00 "] {
        assert!(!text.contains(word), "{word} in {text}");
    }
    assert!(kit.events("conversations.need_summarised").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn opening_a_flagged_message_tracks_the_request_and_the_read_by_counts(db: PgPool) {
    let (iso, written) = in_a_month();
    let quote = format!("A sua consulta é a {written} às 15:00");
    let answer = json!({
        "en": "", "pt-PT": "", "es-ES": "", "fr": "", "lapses_at": null, "due_on": null,
        "items": [{
            "kind": "appointment", "title": "Consulta na Clínica Sol",
            "starts": format!("{iso}T15:00:00+00:00"), "ends": format!("{iso}T15:30:00+00:00"),
            "all_day": false, "time_zone": "Europe/Lisbon", "place": "Rua do Sol 12",
            "quote": quote, "updates": null, "cancels": null
        }],
        "asks": []
    });
    let server = complete(&answer).await;
    let kit = kit_with(db, &server, clinic_deps()).await.tracking();
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &clinic(&format!("Bom dia. {quote}. Até breve.")),
    )
    .await;
    let path = read_path(&kit, workspace, "clinic-1").await;
    let message = kit.message_of("clinic-1").await;
    let conversation = kit.conversation_of("clinic-1").await;

    assert_eq!(kit.post(&sam, &path, None).await.json["outcome"], "queued");
    // Queued already: nothing more is tracked.
    assert_eq!(
        kit.post(&sam, &path, None).await.json["outcome"],
        "already_queued"
    );
    let requested =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.message_read_requested")
            .await;
    assert_eq!(requested.len(), 1);
    assert_eq!(requested[0].version, 1);
    assert_eq!(
        requested[0].envelope["actor"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(
        requested[0].envelope["operation"],
        "conversations.request_message_read"
    );
    assert_eq!(requested[0].body["message_id"], message.as_str());
    assert_eq!(requested[0].body["conversation_id"], conversation.as_str());

    kit.run_jobs().await;
    let reads =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.message_read_completed")
            .await;
    let read: Vec<_> = reads
        .iter()
        .filter(|event| event.body["layer"] == "model")
        .collect();
    assert_eq!(read.len(), 1, "{reads:?}");
    let event = read[0];
    assert_eq!(event.envelope["actor"]["type"], "system");
    assert_eq!(
        event.envelope["on_behalf_of"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(event.body["message_id"], message.as_str());
    assert_eq!(event.body["outcome"], "read");
    assert_eq!(event.body["trigger"], "open");
    assert_eq!(event.body["item_count"], 1);
    assert_eq!(event.body["ask_count"], 0);
    assert_eq!(event.body["summary_ready"], false);
    let text = event.body.to_string();
    for word in ["Consulta", "Rua do Sol", "Clínica", "15:00"] {
        assert!(!text.contains(word), "{word} in {text}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_phishing_warning_gets_no_read(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;
    let phishing: Vec<String> = sqlx::query_scalar(
        "SELECT m.provider_message_id FROM conversations.needs n
           JOIN conversations.messages m ON m.id = n.message_id
          WHERE n.state = 'open' AND n.question = 'suspicious'",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert!(
        !phishing.is_empty(),
        "the fixtures raise a phishing warning"
    );
    // Nothing asked of the model or stored for it: a summary would retell
    // the scam in Scout's voice.
    let rows = stored(&kit).await;
    assert!(
        rows.iter().all(|(id, _, _)| !phishing.contains(id)),
        "{rows:?}"
    );
    assert_eq!(calls(&server).await, rows.len());
    // Opening it queues nothing either.
    let path = read_path(&kit, workspace, &phishing[0]).await;
    let opened = kit.post(&sam, &path, None).await;
    assert_eq!(opened.json["outcome"], "not_flagged");
    // Even a summary written before the need became a warning stays hidden.
    let message = kit.message_of(&phishing[0]).await;
    sqlx::query(
        "INSERT INTO conversations.message_reads
             (message_id, workspace_id, layer, state, texts, attempts, read_at)
         VALUES ($1::uuid, $2, 'model', 'read', $3, 1, now())",
    )
    .bind(&message)
    .bind(workspace.as_uuid())
    .bind(good())
    .execute(&kit.db)
    .await
    .unwrap();
    let shown = detail(&kit, &sam, workspace, &phishing[0], "en").await;
    assert!(shown["need_summary"].is_null(), "{shown:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_read_is_written_once_and_never_again(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, inbox, ..) = sams_needs(&kit).await;
    let before = calls(&server).await;
    assert!(before > 0);

    // The same message queued again, and its need worked out again, asks
    // nothing and writes nothing.
    requeue(&kit, workspace, "needs-1").await;
    kit.ingest(workspace, inbox, "needs").await;
    assert_eq!(calls(&server).await, before);
    assert_eq!(
        kit.events("conversations.need_summarised").await.len(),
        before
    );
    assert_eq!(kit.events("conversations.message_read").await.len(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn opening_an_item_never_asks_the_model(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;
    let before = calls(&server).await;
    let path = read_path(&kit, workspace, "needs-1").await;
    for _ in 0..3 {
        detail(&kit, &sam, workspace, "needs-1", "en").await;
        let view = kit.get(&sam, &path).await;
        assert_eq!(view.status, StatusCode::OK);
        assert_eq!(view.json["status"], "done");
    }
    assert_eq!(calls(&server).await, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_who_may_not_read_the_inbox_gets_no_read(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, _, leo) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-1").await;
    let response = kit
        .get(&leo, &format!("/api/v1/w/{workspace}/conversations/{id}"))
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(!response.json.to_string().contains("walkthrough"));
    let path = read_path(&kit, workspace, "needs-1").await;
    let read = kit.get(&leo, &path).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
    let asked = kit.post(&leo, &path, None).await;
    assert_eq!(asked.status, StatusCode::NOT_FOUND);
    assert!(!asked.json.to_string().contains("outcome"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unusable_answer_is_recorded_and_tried_once_more_then_never(db: PgPool) {
    let server = complete(&json!({"en": " ", "pt-PT": "", "es-ES": "", "fr": "",
        "lapses_at": null, "due_on": null, "items": [], "asks": []}))
    .await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;
    let first = calls(&server).await;
    let rows = stored(&kit).await;
    assert_eq!(rows.len(), first);
    assert!(
        rows.iter()
            .all(|(_, state, attempts)| state == "failed" && *attempts == 1)
    );
    assert!(kit.events("conversations.need_summarised").await.is_empty());
    assert!(kit.events("conversations.message_read").await.is_empty());
    // Nothing is shown; the email itself still is.
    let shown = detail(&kit, &sam, workspace, "needs-1", "en").await;
    assert_eq!(shown["need_summary"], Value::Null);
    assert!(!shown["messages"].as_array().unwrap().is_empty());
    // It may still be tried once more, so the reader does not say it failed.
    assert!(shown.get("need_read_failed").is_none());

    // One retry later, then no more.
    requeue(&kit, workspace, "needs-1").await;
    assert_eq!(calls(&server).await, first + 1);
    requeue(&kit, workspace, "needs-1").await;
    assert_eq!(calls(&server).await, first + 1);
    let again = stored(&kit).await;
    assert_eq!(again.iter().find(|r| r.0 == "needs-1").unwrap().2, 2);
    let path = read_path(&kit, workspace, "needs-1").await;
    let asked = kit.post(&sam, &path, None).await;
    assert_eq!(asked.json["outcome"], "failed");
    // Both calls are spent: the reader says Scout could not read it.
    let shown = detail(&kit, &sam, workspace, "needs-1", "en").await;
    assert_eq!(shown["need_read_failed"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_failing_provider_writes_nothing(db: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("secret mail words"))
        .mount(&server)
        .await;
    let kit = kit(db, &server).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    let batch = kit.batch(workspace, inbox, "needs").await;
    crate::dev::submit(&kit.state, workspace, &batch)
        .await
        .unwrap();
    // Jobs may fail and retry: only that nothing was written matters here.
    let _ = campbooks_kernel::jobs::Worker::new(kit.state.clone())
        .run_until_idle()
        .await;
    let read: i64 =
        sqlx::query_scalar("SELECT count(*) FROM conversations.message_reads WHERE state = 'read'")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(read, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_complete_role_nothing_is_queued_and_the_email_still_shows(db: PgPool) {
    let kit = Kit::with_deps(db, clinic_deps()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &clinic("Bom dia. A sua consulta é amanhã."),
    )
    .await;
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.read_message'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    assert!(stored(&kit).await.is_empty());
    let shown = detail(&kit, &sam, workspace, "clinic-1", "en").await;
    assert_eq!(shown["need_summary"], Value::Null);
    assert!(!shown["messages"].as_array().unwrap().is_empty());

    // Opening it reads nothing; the reader gets the kind the pass named, to
    // offer "This email has an appointment · Add".
    let path = read_path(&kit, workspace, "clinic-1").await;
    let view = kit.get(&sam, &path).await;
    assert_eq!(view.json["flagged"], true);
    assert_eq!(view.json["dated_kind"], "appointment");
    assert_eq!(view.json["status"], "none");
    let asked = kit.post(&sam, &path, None).await;
    assert_eq!(asked.json["outcome"], "unavailable");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM conversations.message_reads")
            .fetch_one(&kit.db)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_dated_things_off_opening_a_message_queues_nothing(db: PgPool) {
    let server = complete(&good()).await;
    let mut deps = clinic_deps();
    deps.config.dated.enabled = false;
    let kit = kit_with(db, &server, deps).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &clinic("Bom dia. A sua consulta é amanhã."),
    )
    .await;
    let path = read_path(&kit, workspace, "clinic-1").await;
    // Not flagged by a dated thing any more.
    let view = kit.get(&sam, &path).await;
    assert_eq!(view.json["flagged"], false);
    let asked = kit.post(&sam, &path, None).await;
    assert_eq!(asked.json["outcome"], "unavailable");
    kit.run_jobs().await;
    assert_eq!(calls(&server).await, 0);
}

/// What the model was given for each call so far: the user message, parsed.
async fn inputs(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            let chat: Value = request.body_json().unwrap();
            serde_json::from_str(chat["messages"][1]["content"].as_str().unwrap()).unwrap()
        })
        .collect()
}

/// The module that owns tasks, as far as this test needs it: the hosting
/// company's bill is already a task.
#[derive(Debug)]
struct HostingBill;

impl OpenTasks for HostingBill {
    fn open_tasks<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        _workspace: WorkspaceId,
        _inbox: sqlx::types::Uuid,
        _conversation: sqlx::types::Uuid,
        sender_address: &'a str,
    ) -> OpenTasksFuture<'a> {
        Box::pin(async move {
            Ok(if sender_address == "billing@acme-hosting.example" {
                vec![KernelTask {
                    reference: "task-17".into(),
                    title: "Pay the hosting bill".into(),
                    due_on: None,
                    due_at: None,
                }]
            } else {
                Vec::new()
            })
        })
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bill_that_repeats_an_open_task_is_told_about_it_and_may_name_it(db: PgPool) {
    let answer = json!({
        "en": "A bill.", "pt-PT": "Uma fatura.", "es-ES": "Una factura.", "fr": "Une facture.",
        "lapses_at": null, "due_on": null, "items": [],
        "asks": [{"title": "Pay Acme Hosting", "quote": "Payment due by 30 October",
                  "due": "2026-10-30", "verb": "pay", "amount_minor": null,
                  "amount_currency": null, "same_as": "task-17"}]
    });
    let server = complete(&answer).await;
    let kit = kit(db, &server)
        .await
        .with_open_tasks(std::sync::Arc::new(HostingBill));
    sams_needs(&kit).await;

    // The model is shown the task for the hosting company's bill, and for
    // nobody else's.
    let given = inputs(&server).await;
    assert_eq!(given.len(), 3);
    for input in &given {
        let bill = input["email"]["from"] == "Acme Hosting";
        assert_eq!(
            input["open_tasks"].as_array().unwrap().len(),
            usize::from(bill)
        );
        if bill {
            assert_eq!(input["open_tasks"][0]["task_ref"], "task-17");
            assert_eq!(input["open_tasks"][0]["title"], "Pay the hosting bill");
        }
    }
    // Its ask says it is the same one, and the event carries only the
    // reference.
    let asks: Value = sqlx::query_scalar(
        "SELECT s.asks FROM conversations.message_reads s
           JOIN conversations.messages m ON m.id = s.message_id
          WHERE m.provider_message_id = 'needs-3' AND s.layer = 'model'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(asks[0]["same_as"], "task-17");
    let events = kit.events("conversations.message_read").await;
    let bill = events
        .iter()
        .find(|e| !e["asks"].as_array().unwrap().is_empty())
        .unwrap();
    assert_eq!(bill["asks"][0]["same_as"], "task-17");
    assert!(!bill.to_string().contains("hosting bill"));
}

/// Members' time zones as the schedule module would answer: Sam is in Lisbon.
#[derive(Debug)]
struct Lisbon;

impl TimeZones for Lisbon {
    fn time_zone<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        _workspace: WorkspaceId,
        _member: campbooks_kernel::tenancy::UserId,
    ) -> TimeZoneFuture<'a> {
        Box::pin(async { Ok(Some("Europe/Lisbon".to_owned())) })
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_model_is_told_the_zone_of_the_inboxes_owner_and_utc_for_a_shared_one(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server)
        .await
        .with_time_zones(std::sync::Arc::new(Lisbon));
    // Sam's own inbox.
    sams_needs(&kit).await;
    let own = inputs(&server).await;
    assert!(!own.is_empty());
    assert!(
        own.iter()
            .all(|input| input["time_zone"] == "Europe/Lisbon")
    );

    // An inbox Sam and Leo both read has no single owner.
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam2", workspace);
    let (_, leo) = kit.member("leo2", workspace);
    let shared = kit
        .inbox(
            workspace,
            "team@example.com",
            &[(sam, InboxRole::Manager), (leo, InboxRole::Reader)],
        )
        .await;
    let before = calls(&server).await;
    kit.ingest(workspace, shared, "needs").await;
    let all = inputs(&server).await;
    assert!(all.len() > before);
    assert!(
        all[before..]
            .iter()
            .all(|input| input["time_zone"] == "UTC")
    );
    // Nothing from a shared inbox names an owner.
    let events = kit.events("conversations.message_read").await;
    assert!(events[before..].iter().all(|e| e.get("owner").is_none()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_takes_the_due_date_scout_reads_and_a_do_takes_its_ask_s(db: PgPool) {
    let mut answer = good();
    answer["due_on"] = json!("2026-10-14");
    let server = complete(&answer).await;
    let kit = kit(db, &server).await;
    sams_needs(&kit).await;
    // Marta's reply is due when the read says; Pedro's lease (a `do`, whose
    // date comes from its ask) and the bill (a `pay`) take nothing from it.
    let (_, _, due_on, _) = need_of(&kit, "needs-1").await;
    assert_eq!(due_on, chrono::NaiveDate::from_ymd_opt(2026, 10, 14));
    assert_eq!(need_of(&kit, "needs-2").await.2, None);
    assert_eq!(need_of(&kit, "needs-3").await.2, None);
}

/// The upgrade's needs backfill of `inbox`, as the migration queues it and
/// the scheduler's tick runs it.
async fn run_backfill(kit: &Kit, inbox: InboxId) {
    sqlx::query(
        "INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
         SELECT id, workspace_id FROM conversations.inboxes WHERE id = $1
         ON CONFLICT (inbox_id) DO UPDATE SET state = 'pending', after_id = NULL,
                                              refreshed = 0, finished_at = NULL",
    )
    .bind(inbox)
    .execute(&kit.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(
        &mut tx,
        &crate::features::backfill_needs::DispatchNeedsBackfills::default(),
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_backfill_reads_the_flagged_open_needs_that_have_none(db: PgPool) {
    // Needs raised before this shipped: no read, no job.
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (_, inbox, ..) = sams_needs(&kit).await;
    sqlx::query("DELETE FROM conversations.message_reads")
        .execute(&kit.db)
        .await
        .unwrap();
    assert!(stored(&kit).await.is_empty());
    let before = calls(&server).await;

    run_backfill(&kit, inbox).await;

    // Only the flagged ones: Jonas's plain question and the warning get none.
    let rows = stored(&kit).await;
    let ids: Vec<&str> = rows.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, FLAGGED);
    assert!(rows.iter().all(|(_, state, _)| state == "read"));
    let after = calls(&server).await;
    assert_eq!(after - before, rows.len());

    // A second run asks for nothing again.
    run_backfill(&kit, inbox).await;
    assert_eq!(calls(&server).await, after);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_read_from_before_items_and_asks_is_read_once_more(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (_, inbox, ..) = sams_needs(&kit).await;
    // Summaries written before items and asks were read.
    sqlx::query("UPDATE conversations.message_reads SET read_at = NULL")
        .execute(&kit.db)
        .await
        .unwrap();
    let written = stored(&kit).await;
    let before = calls(&server).await;

    run_backfill(&kit, inbox).await;
    let after = calls(&server).await;
    assert_eq!(after - before, written.len(), "each asked once more");
    let unread: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.message_reads WHERE read_at IS NULL",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(unread, 0);
    assert!(
        stored(&kit)
            .await
            .iter()
            .all(|(_, state, _)| state == "read")
    );

    // Never again.
    run_backfill(&kit, inbox).await;
    assert_eq!(calls(&server).await, after);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_read_with_no_usable_answer_keeps_its_old_summary_and_is_never_asked_again(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (_, inbox, ..) = sams_needs(&kit).await;
    sqlx::query("UPDATE conversations.message_reads SET read_at = NULL")
        .execute(&kit.db)
        .await
        .unwrap();
    // The model now gives nothing usable.
    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "read-test",
            "choices": [{"message": {"role": "assistant", "content": "not json"},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 300, "completion_tokens": 3}
        })))
        .mount(&server)
        .await;
    run_backfill(&kit, inbox).await;
    let after = calls(&server).await;
    assert!(after > 0);
    let kept: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.message_reads
          WHERE state = 'read' AND read_at IS NOT NULL AND texts <> '{}'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(kept, 3, "the summaries stay, and are marked read");
    run_backfill(&kit, inbox).await;
    assert_eq!(calls(&server).await, after);
}

/// `(state, cleared_because, due_on, lapses_at)` of the need of
/// `provider_message_id`'s conversation.
async fn need_of(
    kit: &Kit,
    provider_message_id: &str,
) -> (
    String,
    Option<String>,
    Option<chrono::NaiveDate>,
    Option<DateTime<Utc>>,
) {
    let conversation = kit.conversation_of(provider_message_id).await;
    sqlx::query_as(
        "SELECT state, cleared_because, due_on, lapses_at FROM conversations.needs
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_lapse_time_scout_reads_reaches_the_need(db: PgPool) {
    // A lapse time twenty minutes after Marta's mail.
    let mut answer = good();
    answer["lapses_at"] = json!("2026-10-12T17:00:00Z");
    let server = complete(&answer).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;

    // Marta's ask lapsed: cleared, and the inbox says it expired.
    let (state, cleared, _, lapses_at) = need_of(&kit, "needs-1").await;
    assert_eq!(
        (state.as_str(), cleared.as_deref()),
        ("cleared", Some("lapsed"))
    );
    assert_eq!(lapses_at, Some("2026-10-12T17:00:00Z".parse().unwrap()));
    let id = kit.conversation_of("needs-1").await;
    let list = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    let marta = &list.json["conversations"][0];
    assert_eq!(marta["need"], Value::Null);
    assert_eq!(marta["lapsed_need"]["lapsed_at"], "2026-10-12T17:00:00Z");
    // The bill was sent after that time, so no lapse time is kept (and a
    // bill never lapses).
    let (state, _, _, lapses_at) = need_of(&kit, "needs-3").await;
    assert_eq!(state, "open");
    assert_eq!(lapses_at, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_asks_scout_reads_give_do_and_pay_needs_their_due_date(db: PgPool) {
    // One answer for every flagged message: each keeps only the ask whose
    // quote is in its own words, so a made-up quote costs that ask only.
    let answer = json!({
        "en": "A request.", "pt-PT": "Um pedido.", "es-ES": "Una petición.", "fr": "Une demande.",
        "lapses_at": null, "due_on": null, "items": [],
        "asks": [
            {"title": "Sign and return the lease", "quote": "send it back by Friday 16 October",
             "due": "2026-10-16", "verb": "do", "amount_minor": null, "amount_currency": null,
             "same_as": null},
            {"title": "Pay Acme Hosting", "quote": "Payment due by 30 October",
             "due": "2026-10-30", "verb": "pay", "amount_minor": 4900, "amount_currency": "eur",
             "same_as": null},
            {"title": "Something made up", "quote": "wire the money to the new account",
             "due": "2026-10-20", "verb": "pay", "amount_minor": null, "amount_currency": null,
             "same_as": null}
        ]
    });
    let server = complete(&answer).await;
    let kit = kit(db, &server).await;
    sams_needs(&kit).await;

    // Pedro's lease: a `do` need, due when the first `do` ask says.
    let (state, _, due_on, _) = need_of(&kit, "needs-2").await;
    assert_eq!(state, "open");
    assert_eq!(due_on, chrono::NaiveDate::from_ymd_opt(2026, 10, 16));
    // The invoice: a `pay` need, due when the `pay` ask says.
    let (state, _, due_on, _) = need_of(&kit, "needs-3").await;
    assert_eq!(state, "open");
    assert_eq!(due_on, chrono::NaiveDate::from_ymd_opt(2026, 10, 30));
    let opened = kit.events("conversations.need_changed").await;
    let pay = opened
        .iter()
        .rev()
        .find(|e| e["need"]["reason"] == "pay")
        .unwrap();
    assert_eq!(pay["need"]["due_on"], "2026-10-30");

    // What was kept per message: the invoice has its one ask, with the amount
    // whose digits are in the email; Pedro's has the lease; the made-up one
    // is nowhere.
    let asks: Vec<(String, Value)> = sqlx::query_as(
        "SELECT m.provider_message_id, s.asks FROM conversations.message_reads s
           JOIN conversations.messages m ON m.id = s.message_id
          WHERE s.layer = 'model' ORDER BY 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    let by = |id: &str| &asks.iter().find(|(m, _)| m == id).unwrap().1;
    assert_eq!(by("needs-1").as_array().unwrap().len(), 0);
    assert_eq!(by("needs-2").as_array().unwrap().len(), 1);
    assert_eq!(by("needs-3")[0]["amount"]["minor"], 4900);
    assert_eq!(by("needs-3")[0]["amount"]["currency"], "EUR");
    let published = kit.events("conversations.message_read").await;
    let bill = published
        .iter()
        .find(|e| e["asks"][0]["verb"] == "pay")
        .unwrap();
    assert_eq!(bill["asks"][0]["has_amount"], true);
    assert!(!bill.to_string().contains("Acme"));
}

#[test]
fn a_message_is_read_only_when_flagged_asked_for_and_not_done() {
    let row = |state: Option<&str>, read: bool, trigger: Option<&str>, need: bool| Loaded {
        sender: Value::Null,
        subject: String::new(),
        bodies: Value::Null,
        sent_at: Utc::now(),
        conversation_id: ConversationId::new(),
        inbox_id: InboxId::new(),
        state: state.map(ToOwned::to_owned),
        attempts: state.map(|_| 1),
        read_at: read.then(Utc::now),
        triggered_by: trigger.map(ToOwned::to_owned),
        need_reason: need.then(|| "reply".to_owned()),
        need_question: need.then(|| "needs_reply".to_owned()),
    };
    let on = DatedSettings::default();
    // A need opened on a flagged message.
    let plan_of = |row: &Loaded, flagged, covered| super::plan(row, flagged, covered, on);
    assert!(plan_of(&row(Some("queued"), false, Some("need"), true), true, false).is_ok());
    // Not flagged, a warning, already read, used both calls.
    assert_eq!(
        plan_of(
            &row(Some("queued"), false, Some("need"), true),
            false,
            false
        ),
        Err(Skip::NotFlagged)
    );
    let mut warning = row(Some("queued"), false, Some("need"), true);
    warning.need_question = Some("suspicious".into());
    assert_eq!(plan_of(&warning, true, false), Err(Skip::Phishing));
    assert_eq!(
        plan_of(&row(Some("read"), true, Some("need"), true), true, false),
        Err(Skip::Done)
    );
    let mut twice = row(Some("failed"), false, Some("need"), true);
    twice.attempts = Some(2);
    assert_eq!(plan_of(&twice, true, false), Err(Skip::Done));
    let mut once = row(Some("failed"), false, Some("need"), true);
    once.attempts = Some(1);
    assert!(plan_of(&once, true, false).is_ok());
    // Asked for by a need that is gone, or by an open that is switched off.
    assert_eq!(
        plan_of(
            &row(Some("queued"), false, Some("need"), false),
            true,
            false
        ),
        Err(Skip::NothingToAsk)
    );
    let off = DatedSettings {
        enabled: false,
        ..on
    };
    assert_eq!(
        super::plan(
            &row(Some("queued"), false, Some("open"), false),
            true,
            false,
            off
        ),
        Err(Skip::NothingToAsk)
    );
    // Opened by a member with no need: the dates are all there is to ask.
    let opened = plan_of(
        &row(Some("queued"), false, Some("open"), false),
        true,
        false,
    )
    .unwrap();
    assert!(opened.dates && !opened.need_open && opened.trigger == Trigger::Open);
    // Covered by the sender's data: only an open need's summary is left.
    assert_eq!(
        plan_of(&row(Some("queued"), false, Some("open"), false), true, true),
        Err(Skip::NothingToAsk)
    );
    let summary_only =
        plan_of(&row(Some("queued"), false, Some("need"), true), true, true).unwrap();
    assert!(!summary_only.dates && summary_only.need_open);
    // A read from before items and asks is read again for an open need only.
    let legacy = plan_of(&row(Some("read"), false, None, true), true, false).unwrap();
    assert!(legacy.rereading);
    assert_eq!(
        plan_of(&row(Some("read"), false, None, false), true, false),
        Err(Skip::NothingToAsk)
    );
}

#[test]
fn the_sender_s_data_covers_what_the_pass_named() {
    let structured = (
        json!([{"kind": "travel"}, {"kind": "stay"}]),
        json!([{"verb": "pay"}]),
    );
    let flag = |kind: Option<&str>| Flagged {
        by_dated_thing: true,
        kind: kind.map(ToOwned::to_owned),
    };
    // Nothing structured: not covered.
    assert!(!covers(None, Some(&flag(Some("travel")))));
    // A kind among the items, none named, or none: covered.
    assert!(covers(Some(&structured), Some(&flag(Some("stay")))));
    assert!(covers(Some(&structured), Some(&flag(None))));
    assert!(covers(Some(&structured), Some(&flag(Some("none")))));
    // A kind the data did not find: not covered.
    assert!(!covers(Some(&structured), Some(&flag(Some("show")))));
    // A deadline is covered by a payment found, not by items alone.
    assert!(covers(Some(&structured), Some(&flag(Some("deadline")))));
    let only_items = (json!([{"kind": "travel"}]), json!([]));
    assert!(!covers(Some(&only_items), Some(&flag(Some("deadline")))));
}
