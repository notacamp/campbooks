//! Scout's summary of the email behind a need, end to end over
//! `fixtures/needs.json` with a `complete` role on a mock server.

use axum::http::StatusCode;
use campbooks_ai::{AiConfig, Gateway, ProviderConfig, Role};
use campbooks_kernel::{
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::{Kit, sorting_deps},
};

/// An instruct model answering with `content`.
async fn complete(content: &Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "summary-test",
            "choices": [{"message": {"role": "assistant", "content": content.to_string()},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 300, "completion_tokens": 120}
        })))
        .mount(&server)
        .await;
    server
}

fn good() -> Value {
    json!({
        "en": "Marta asks you to confirm Thursday's walkthrough by Wednesday.",
        "pt-PT": "A Marta pede-lhe que confirme a visita de quinta-feira até quarta.",
        "es-ES": "Marta le pide que confirme la visita del jueves antes del miércoles.",
        "fr": "Marta vous demande de confirmer la visite de jeudi avant mercredi.",
    })
}

/// The module with a `complete` role on `server`, and the scripted decide
/// model raising the fixtures' needs.
async fn kit(db: PgPool, server: &MockServer) -> Kit {
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "test");
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Complete, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap();
    let deps = ConversationsDeps {
        ai: Some(gateway),
        ..sorting_deps()
    };
    Kit::with_deps(db, deps).await
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

/// `(state, attempts)` of every stored summary, by provider message id.
async fn stored(kit: &Kit) -> Vec<(String, String, i16)> {
    sqlx::query_as(
        "SELECT m.provider_message_id, s.state, s.attempts
           FROM conversations.need_summaries s
           JOIN conversations.messages m ON m.id = s.message_id ORDER BY 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

/// Open needs that get a summary: all but the phishing warnings.
async fn open_needs(kit: &Kit) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM conversations.needs
          WHERE state = 'open' AND question <> 'suspicious'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

async fn requeue(kit: &Kit, workspace: WorkspaceId, provider_message_id: &str) {
    let message = kit.message_of(provider_message_id).await.parse().unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    let _ = jobs::enqueue_to_in::<super::SummarizeNeed>(
        &mut tx,
        &workspace.to_string(),
        &super::SummarizeNeedPayload { message },
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

#[sqlx::test(migrations = "../../../migrations")]
async fn a_need_that_opens_gets_one_summary_in_every_language(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;

    let needs = open_needs(&kit).await;
    assert!(needs >= 3, "the fixtures raise needs");
    let rows = stored(&kit).await;
    // One call and one row per message that raised a need, all written.
    assert_eq!(i64::try_from(rows.len()).unwrap(), needs);
    assert_eq!(calls(&server).await, rows.len());
    assert!(
        rows.iter()
            .all(|(_, state, attempts)| state == "written" && *attempts == 1)
    );
    // Announced once each, with ids only.
    let events = kit.events("conversations.need_summarised").await;
    assert_eq!(events.len(), rows.len());
    assert!(events[0].get("texts").is_none() && events[0].get("summary").is_none());

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
async fn a_phishing_warning_gets_no_summary(db: PgPool) {
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
    // Even a summary written before the need became a warning stays hidden.
    let message = kit.message_of(&phishing[0]).await;
    sqlx::query(
        "INSERT INTO conversations.need_summaries (message_id, workspace_id, state, texts, attempts)
         VALUES ($1::uuid, $2, 'written', $3, 1)",
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
async fn a_summary_is_written_once_and_never_again(db: PgPool) {
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
}

#[sqlx::test(migrations = "../../../migrations")]
async fn opening_an_item_never_asks_the_model(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;
    let before = calls(&server).await;
    for _ in 0..3 {
        detail(&kit, &sam, workspace, "needs-1", "en").await;
    }
    assert_eq!(calls(&server).await, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_who_may_not_read_the_inbox_gets_no_summary(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, _, leo) = sams_needs(&kit).await;
    let id = kit.conversation_of("needs-1").await;
    let response = kit
        .get(&leo, &format!("/api/v1/w/{workspace}/conversations/{id}"))
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(!response.json.to_string().contains("walkthrough"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unusable_answer_is_recorded_and_tried_once_more_then_never(db: PgPool) {
    let server = complete(&json!({"en": " ", "pt-PT": "", "es-ES": "", "fr": ""})).await;
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
    // Nothing is shown; the email itself still is.
    let shown = detail(&kit, &sam, workspace, "needs-1", "en").await;
    assert_eq!(shown["need_summary"], Value::Null);
    assert!(!shown["messages"].as_array().unwrap().is_empty());

    // One retry later, then no more.
    requeue(&kit, workspace, "needs-1").await;
    assert_eq!(calls(&server).await, first + 1);
    requeue(&kit, workspace, "needs-1").await;
    assert_eq!(calls(&server).await, first + 1);
    let again = stored(&kit).await;
    assert_eq!(again.iter().find(|r| r.0 == "needs-1").unwrap().2, 2);
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
    let written: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.need_summaries WHERE state = 'written'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(written, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_complete_role_nothing_is_queued_and_the_email_still_shows(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.summarize_need'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    assert!(stored(&kit).await.is_empty());
    let shown = detail(&kit, &sam, workspace, "needs-1", "en").await;
    assert_eq!(shown["need_summary"], Value::Null);
    assert!(!shown["messages"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_backfill_summarises_the_open_needs_that_have_none(db: PgPool) {
    // Needs raised before this shipped: no summary, no job.
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (_, inbox, ..) = sams_needs(&kit).await;
    sqlx::query("DELETE FROM conversations.need_summaries")
        .execute(&kit.db)
        .await
        .unwrap();
    assert!(stored(&kit).await.is_empty());
    let before = calls(&server).await;

    // The upgrade: the migration queues every inbox for the needs backfill,
    // and the scheduler's tick runs it.
    sqlx::query(
        "INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
         SELECT id, workspace_id FROM conversations.inboxes WHERE id = $1
         ON CONFLICT (inbox_id) DO UPDATE SET state = 'pending', after_id = NULL, refreshed = 0,
                                              finished_at = NULL",
    )
    .bind(inbox)
    .execute(&kit.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(
        &mut tx,
        &crate::features::backfill_needs::DispatchNeedsBackfills,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;

    let rows = stored(&kit).await;
    assert_eq!(i64::try_from(rows.len()).unwrap(), open_needs(&kit).await);
    assert!(rows.iter().all(|(_, state, _)| state == "written"));
    let after = calls(&server).await;
    assert_eq!(after - before, rows.len());

    // A second run asks for nothing again.
    sqlx::query("UPDATE conversations.needs_backfills SET state = 'pending', after_id = NULL")
        .execute(&kit.db)
        .await
        .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(
        &mut tx,
        &crate::features::backfill_needs::DispatchNeedsBackfills,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert_eq!(calls(&server).await, after);
}

/// The summary with dates read from the email (campbooks-cloud#275): a lapse
/// time twenty minutes after Marta's mail, and a due date of 16 October.
fn dated() -> Value {
    let mut answer = good();
    answer["lapses_at"] = json!("2026-10-12T17:00:00Z");
    answer["due_on"] = json!("2026-10-16");
    answer
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
    Option<chrono::DateTime<chrono::Utc>>,
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
async fn the_dates_scout_reads_reach_the_need(db: PgPool) {
    let server = complete(&dated()).await;
    let kit = kit(db, &server).await;
    let (workspace, _, sam, _) = sams_needs(&kit).await;

    // Marta's ask lapsed twenty minutes after she sent it: cleared, and the
    // inbox says it expired.
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
    let events = kit.events("conversations.need_changed").await;
    assert!(
        events
            .iter()
            .any(|e| e["conversation"] == id.as_str() && e["cleared_because"] == "lapsed"),
        "{events:?}"
    );

    // The bill was sent after that time, so no lapse time is kept (and a
    // bill never lapses); its due date is.
    let (state, _, due_on, lapses_at) = need_of(&kit, "needs-3").await;
    assert_eq!(state, "open");
    assert_eq!(due_on, chrono::NaiveDate::from_ymd_opt(2026, 10, 16));
    assert_eq!(lapses_at, None);
    let opened = events
        .iter()
        .rev()
        .find(|e| e["need"]["reason"] == "pay")
        .unwrap();
    assert_eq!(opened["need"]["due_on"], "2026-10-16");

    // Every summary says its dates were read.
    let unread: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.need_summaries WHERE dates_read_at IS NULL",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(unread, 0);
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
        &crate::features::backfill_needs::DispatchNeedsBackfills,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_summary_from_before_the_dates_is_read_once_more(db: PgPool) {
    let server = complete(&good()).await;
    let kit = kit(db, &server).await;
    let (_, inbox, ..) = sams_needs(&kit).await;
    // Summaries written before dates were read.
    sqlx::query("UPDATE conversations.need_summaries SET dates_read_at = NULL")
        .execute(&kit.db)
        .await
        .unwrap();
    let written = stored(&kit).await;
    let before = calls(&server).await;

    run_backfill(&kit, inbox).await;
    let after = calls(&server).await;
    assert_eq!(after - before, written.len(), "each asked once more");
    let unread: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.need_summaries WHERE dates_read_at IS NULL",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(unread, 0);
    assert!(
        stored(&kit)
            .await
            .iter()
            .all(|(_, state, _)| state == "written")
    );

    // Never again.
    run_backfill(&kit, inbox).await;
    assert_eq!(calls(&server).await, after);
}
