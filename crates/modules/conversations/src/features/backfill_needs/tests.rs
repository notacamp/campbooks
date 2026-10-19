//! The needs backfill over mail stored "before attention": the fixtures are
//! ingested, then their needs and need events are deleted, which is what a
//! mailbox synced before the attention module looks like, and the
//! migration's seed lists the inboxes again. And the upgrade to the facts
//! (ADR 0018): the old rules' answers stop counting, the needs they raised
//! are withdrawn, and recent mail no checked model answered is read again.

use std::sync::Arc;

use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, Role,
    decide::{
        AnswerValue, Question,
        signals::email::{Envelope, triage},
        strategy::{BoxFuture, Decisions, Strategy},
    },
};
use campbooks_kernel::{
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde_json::Value;
use sqlx::PgPool;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::DispatchNeedsBackfills;
use crate::{
    ConversationsDeps, dev,
    domain::{
        decide::Source,
        policy::{Policy, Switch},
    },
    model::{InboxId, InboxRole},
    testkit::{Gated, Kit, Scripted, fixture, script, sorting_deps},
};

/// Rules that must never be asked: the backfill reads stored answers only.
struct Unasked;

impl Strategy for Unasked {
    fn name(&self) -> &'static str {
        "unasked"
    }

    fn decide<'a>(
        &'a self,
        _: &'a Envelope,
        _: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, campbooks_ai::Error>> {
        panic!("the needs backfill asked a strategy");
    }
}

/// The module as the upgraded server composes it, except that asking the
/// rules panics and the `decide` model is a server that must receive no
/// request (checked when it is dropped); its answers check has not run.
async fn upgraded(db: &PgPool, deps: ConversationsDeps) -> (Kit, MockServer) {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let provider = ProviderConfig::new(
        format!("{}/v1", server.uri()).parse().unwrap(),
        "convaiinnovations/laya-multilingual",
    )
    .with_protocol(Protocol::Laya);
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Decide, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap();
    let deps = ConversationsDeps {
        ai: Some(gateway),
        model: None,
        ..deps
    }
    .with_rules(Arc::new(Unasked));
    (Kit::with_deps(db.clone(), deps).await, server)
}

/// The test settings without a model: mail stored before the upgrade, or
/// while no trusted model answered.
fn without_model() -> ConversationsDeps {
    ConversationsDeps {
        model: None,
        ..sorting_deps()
    }
}

/// Sam's inbox in a new workspace, holding the recorded batches `names`.
async fn mailbox(kit: &Kit, names: &[&str]) -> (WorkspaceId, InboxId) {
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    for name in names {
        kit.ingest(workspace, inbox, name).await;
    }
    (workspace, inbox)
}

/// Make `workspace` look as if its mail was stored before attention: no
/// needs and no need events.
async fn before_attention(db: &PgPool, workspace: WorkspaceId) {
    sqlx::query("DELETE FROM conversations.needs WHERE workspace_id = $1")
        .bind(workspace.as_uuid())
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM kernel.events WHERE name = 'conversations.need_changed' AND workspace_id = $1",
    )
    .bind(workspace.to_string())
    .execute(db)
    .await
    .unwrap();
}

/// The seed of the migration that added the backfill, run again (as the
/// migration's login).
async fn seed(db: &PgPool) {
    let migration = std::fs::read_to_string(format!(
        "{}/../../../migrations/20261020050000_conversations__needs_backfill.sql",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let start = migration.find("-- seed begin").unwrap();
    let end = migration.find("-- seed end").unwrap();
    // The migration's own text, from the repository.
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration[start..end].to_owned()))
        .execute(db)
        .await
        .unwrap();
}

/// The migration that retired the content rules (ADR 0018), run again in
/// full (as the migration's login).
async fn upgrade(db: &PgPool) {
    let migration = std::fs::read_to_string(format!(
        "{}/../../../migrations/20261020170000_conversations__facts_and_model.sql",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    // The migration's own text, from the repository.
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration))
        .execute(db)
        .await
        .unwrap();
}

/// What the scheduler does every minute, then every job it causes.
async fn tick(kit: &Kit) {
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchNeedsBackfills, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// `(provider message id of the need, reason, state, cause)` of
/// `workspace`'s needs.
async fn needs_of(db: &PgPool, workspace: WorkspaceId) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        "SELECT m.provider_message_id, n.reason, n.state, coalesce(n.cleared_because, '')
           FROM conversations.needs n JOIN conversations.messages m ON m.id = n.message_id
          WHERE n.workspace_id = $1 ORDER BY 1",
    )
    .bind(workspace.as_uuid())
    .fetch_all(db)
    .await
    .unwrap()
}

async fn need_events(db: &PgPool, workspace: WorkspaceId) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT payload FROM kernel.events
          WHERE name = 'conversations.need_changed' AND workspace_id = $1 ORDER BY id",
    )
    .bind(workspace.to_string())
    .fetch_all(db)
    .await
    .unwrap()
}

fn row(message: &str, reason: &str, state: &str, cause: &str) -> (String, String, String, String) {
    (message.into(), reason.into(), state.into(), cause.into())
}

#[sqlx::test(migrations = "../../../migrations")]
async fn old_answered_mail_gets_its_needs_without_asking_anyone(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (old, _) = mailbox(&kit, &["needs"]).await;
    // Another workspace, its needs raised live: the backfill must leave it
    // as it is.
    let (current, _) = mailbox(&kit, &["needs"]).await;
    let current_needs = needs_of(&db, current).await;
    let current_events = need_events(&db, current).await.len();
    before_attention(&db, old).await;
    assert!(needs_of(&db, old).await.is_empty());

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;

    assert_eq!(
        needs_of(&db, old).await,
        [
            row("needs-1", "reply", "open", ""),
            row("needs-2", "do", "open", ""),
            row("needs-3", "pay", "open", ""),
            row("needs-4", "review", "open", ""),
            row("needs-5", "reply", "open", ""),
        ]
    );
    // Published like live needs, so attention projects them, with no due
    // date until one is extracted.
    let events = need_events(&db, old).await;
    assert_eq!(events.len(), 5, "{events:?}");
    assert!(events.iter().all(|e| e["workspace"] == old.to_string()));
    let marta: String = sqlx::query_scalar(
        "SELECT conversation_id::text FROM conversations.messages
          WHERE provider_message_id = 'needs-1' AND workspace_id = $1",
    )
    .bind(old.as_uuid())
    .fetch_one(&db)
    .await
    .unwrap();
    let due = events
        .iter()
        .find(|e| e["conversation"] == marta.as_str())
        .unwrap();
    assert_eq!(due["need"]["due_on"], Value::Null);
    // The mail had checked model answers: the model was not asked again.
    let asked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.decide_backlog'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(asked, 0);
    // The other workspace is untouched.
    assert_eq!(needs_of(&db, current).await, current_needs);
    assert_eq!(need_events(&db, current).await.len(), current_events);
    let done: Vec<String> = sqlx::query_scalar("SELECT state FROM conversations.needs_backfills")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(done, ["done", "done"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_older_than_the_needs_window_raises_nothing(db: PgPool) {
    let mut deps = sorting_deps();
    deps.config.needs_window_days = crate::DEFAULT_NEEDS_WINDOW_DAYS;
    let kit = Kit::with_deps(db.clone(), deps.clone()).await;
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    // The same mail, sent six years ago.
    let old: Value = serde_json::from_str(&fixture("needs").replace("\"2026-", "\"2020-")).unwrap();
    let batch = dev::record(&kit.state, workspace, inbox, &old)
        .await
        .unwrap();
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;
    before_attention(&db, workspace).await;

    let (kit, _model) = upgraded(&db, deps).await;
    seed(&db).await;
    tick(&kit).await;
    assert!(needs_of(&db, workspace).await.is_empty());
    assert!(need_events(&db, workspace).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn replied_archived_or_trashed_mail_is_not_a_reply(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    // Sam answered Jonas from his phone before the upgrade.
    let (workspace, _) = mailbox(&kit, &["needs", "needs-replied"]).await;
    // Marta's question (reply) and Pedro's lease (do) archived, the bill
    // trashed, all before the upgrade.
    for (provider_id, column) in [
        ("needs-1", "archived_at = now()"),
        ("needs-2", "archived_at = now()"),
        ("needs-3", "trashed = true"),
    ] {
        // `column` is one of the literals above.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE conversations.conversations SET {column} WHERE id = $1::uuid"
        )))
        .bind(kit.conversation_of(provider_id).await)
        .execute(&db)
        .await
        .unwrap();
    }
    before_attention(&db, workspace).await;

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    // Archive keeps an ask until Done; the phishing warning shows; nothing
    // else is raised, and nothing that never opened is announced cleared.
    assert_eq!(
        needs_of(&db, workspace).await,
        [
            row("needs-2", "do", "open", ""),
            row("needs-4", "review", "open", ""),
        ]
    );
    assert_eq!(need_events(&db, workspace).await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn running_it_twice_changes_nothing(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    let revisions = || async {
        sqlx::query_as::<_, (String, i64, String)>(
            "SELECT conversation_id::text, revision, state FROM conversations.needs ORDER BY 1",
        )
        .fetch_all(&db)
        .await
        .unwrap()
    };
    let first = revisions().await;
    let events = need_events(&db, workspace).await.len();

    // The seed again (what a later migration runs to start over) restarts
    // the inbox's backfill without a second row; it finds every need as it
    // is.
    seed(&db).await;
    let (rows, state): (i64, String) = sqlx::query_as(
        "SELECT count(*), min(state) FROM conversations.needs_backfills WHERE inbox_id = $1",
    )
    .bind(inbox)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((rows, state.as_str()), (1, "pending"));
    tick(&kit).await;
    tick(&kit).await;
    assert_eq!(revisions().await, first);
    assert_eq!(need_events(&db, workspace).await.len(), events);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_large_inbox_is_done_a_page_at_a_time(db: PgPool) {
    let asks = Scripted::model((0..230).map(|i| {
        (
            format!("p{i}@people.example"),
            script("primary", true, false, false, 1.0, false),
        )
    }));
    let kit = Kit::with_deps(db.clone(), sorting_deps().with_model(Arc::new(asks))).await;
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    // 230 people each asking Sam a question: three pages.
    let changes: Vec<Value> = (0..230)
        .map(|i| {
            serde_json::json!({"op": "upsert", "message": {
                "provider_message_id": format!("ask-{i}"),
                "thread_key": format!("thr-ask-{i}"),
                "rfc_message_id": format!("ask-{i}@people.example"),
                "from": {"name": format!("Person {i}"), "address": format!("p{i}@people.example")},
                "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                "subject": format!("Question {i}"),
                "sent_at": "2026-10-12T09:00:00Z",
                "direction": "inbound",
                "text": "Hi Sam,\n\nCould you send me the plan?\n\nThanks",
                "mailbox": {"in_inbox": true},
                "read": false, "starred": false
            }})
        })
        .collect();
    let batch = dev::record(
        &kit.state,
        workspace,
        inbox,
        &serde_json::json!({ "changes": changes }),
    )
    .await
    .unwrap();
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;
    before_attention(&db, workspace).await;

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.needs WHERE state = 'open' AND reason = 'reply'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(open, 230);
    let pages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.backfill_needs'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pages, 3);
    let (state, refreshed): (String, i64) =
        sqlx::query_as("SELECT state, refreshed FROM conversations.needs_backfills")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((state.as_str(), refreshed), ("done", 230));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_inbox_that_goes_takes_its_backfill_with_it(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    seed(&db).await;
    sqlx::query("DELETE FROM conversations.inboxes WHERE id = $1")
        .bind(inbox)
        .execute(&db)
        .await
        .unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.needs_backfills")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    tick(&kit).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_open_question_its_answers_no_longer_support_is_withdrawn(db: PgPool) {
    // A code host's notification from long ago, outside the needs window,
    // that an earlier version put to Sam as a question.
    let mut deps = sorting_deps();
    deps.config.needs_window_days = crate::DEFAULT_NEEDS_WINDOW_DAYS;
    deps.config.decide_backfill_days = crate::DEFAULT_DECIDE_BACKFILL_DAYS;
    let kit = Kit::with_deps(db.clone(), deps.clone()).await;
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    let old: Value =
        serde_json::from_str(&fixture("github-comment").replace("\"2026-", "\"2020-")).unwrap();
    let batch = dev::record(&kit.state, workspace, inbox, &old)
        .await
        .unwrap();
    dev::submit(&kit.state, workspace, &batch).await.unwrap();
    kit.run_jobs().await;
    let message = kit.message_of("github-comment-1").await;
    sqlx::query(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question, state,
              latest_inbound_id)
         SELECT conversation_id, workspace_id, id, 'review', 'check', 'model', 'asks_action',
                'open', id
           FROM conversations.messages WHERE id = $1::uuid",
    )
    .bind(&message)
    .execute(&db)
    .await
    .unwrap();

    let (kit, _model) = upgraded(&db, deps).await;
    seed(&db).await;
    tick(&kit).await;
    assert_eq!(
        needs_of(&db, workspace).await,
        [row("github-comment-1", "review", "cleared", "superseded")]
    );
    let events = need_events(&db, workspace).await;
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["cleared_because"], "superseded");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bill_raised_before_the_money_question_stays_until_it_is_asked(db: PgPool) {
    // A bill an earlier version raised as Pay, on recent mail never asked
    // the money question; the upgraded model cannot be asked yet (its
    // answers check has not run), as when the daily AI cap is reached.
    let kit = Kit::new(db.clone()).await;
    let (workspace, _) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    let bill = kit.message_of("needs-3").await;
    sqlx::query(
        "DELETE FROM conversations.decisions WHERE message_id = $1::uuid AND question = 'money'",
    )
    .bind(&bill)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question, state,
              latest_inbound_id)
         SELECT conversation_id, workspace_id, id, 'pay', 'check', 'model', 'asks_action',
                'open', id
           FROM conversations.messages WHERE id = $1::uuid",
    )
    .bind(&bill)
    .execute(&db)
    .await
    .unwrap();

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    let bills: Vec<_> = needs_of(&db, workspace)
        .await
        .into_iter()
        .filter(|(message, ..)| message == "needs-3")
        .collect();
    assert_eq!(bills, [row("needs-3", "pay", "open", "")]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_conversation_that_cannot_be_worked_out_is_skipped(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    // The bill's stored reasons no longer decode: its refresh fails.
    sqlx::query(
        "UPDATE conversations.decisions SET reasons = '\"not a list\"'
          WHERE message_id = (SELECT id FROM conversations.messages
                               WHERE provider_message_id = 'needs-3')",
    )
    .execute(&db)
    .await
    .unwrap();

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    let raised: Vec<String> = needs_of(&db, workspace)
        .await
        .into_iter()
        .map(|(message, ..)| message)
        .collect();
    assert_eq!(raised, ["needs-1", "needs-2", "needs-4", "needs-5"]);
    let state: String = sqlx::query_scalar("SELECT state FROM conversations.needs_backfills")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(state, "done");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_running_backfill_whose_job_is_gone_is_queued_again(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    seed(&db).await;
    // Started by a tick whose page job was then lost.
    sqlx::query("UPDATE conversations.needs_backfills SET state = 'running'")
        .execute(&db)
        .await
        .unwrap();
    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    tick(&kit).await;
    assert_eq!(needs_of(&db, workspace).await.len(), 5);
    // One page for one small inbox: no job was queued twice.
    let pages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.backfill_needs'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pages, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_backfill_row_keeps_its_inbox_and_workspace(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _) = mailbox(&kit, &["needs"]).await;
    seed(&db).await;
    // A transaction that sets nothing sees no row (campbooks-cloud#129).
    let mut tx = kit.state.db.begin().await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.needs_backfills")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 0);
    tx.rollback().await.unwrap();
    // The dispatcher's context (the maintenance scope) may only start rows.
    let mut tx = campbooks_kernel::db::begin_maintenance(&kit.state.db)
        .await
        .unwrap();
    let moved = sqlx::query(
        "UPDATE conversations.needs_backfills SET state = 'running', workspace_id = $1",
    )
    .bind(WorkspaceId::new().as_uuid())
    .execute(&mut *tx)
    .await;
    assert!(moved.is_err(), "moved to another workspace");
    tx.rollback().await.unwrap();
    let mut tx = campbooks_kernel::db::begin_maintenance(&kit.state.db)
        .await
        .unwrap();
    let finished = sqlx::query("UPDATE conversations.needs_backfills SET state = 'done'")
        .execute(&mut *tx)
        .await;
    assert!(finished.is_err(), "the dispatcher may only start rows");
    tx.rollback().await.unwrap();
    // Nor may the workspace's own jobs move a row.
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace.to_string())
        .await
        .unwrap();
    let moved =
        sqlx::query("UPDATE conversations.needs_backfills SET inbox_id = gen_random_uuid()")
            .execute(&mut *tx)
            .await;
    assert!(moved.is_err(), "moved to another inbox");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_kept_read_only_inbox_is_left_as_it_is(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, inbox) = mailbox(&kit, &["needs"]).await;
    before_attention(&db, workspace).await;
    // Its connection was removed and the mail kept, read-only; one need
    // from before stays as it was.
    sqlx::query("UPDATE conversations.inboxes SET detached_at = now() WHERE id = $1")
        .bind(inbox)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question, state,
              latest_inbound_id)
         SELECT conversation_id, workspace_id, id, 'review', 'check', 'model', 'asks_action',
                'open', id
           FROM conversations.messages WHERE provider_message_id = 'needs-5'",
    )
    .execute(&db)
    .await
    .unwrap();

    let (kit, _model) = upgraded(&db, sorting_deps()).await;
    seed(&db).await;
    tick(&kit).await;
    assert_eq!(
        needs_of(&db, workspace).await,
        [row("needs-5", "review", "open", "")]
    );
    assert!(need_events(&db, workspace).await.is_empty());
    let state: String = sqlx::query_scalar("SELECT state FROM conversations.needs_backfills")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(state, "done");

    // Nor does a refresh of one of its conversations change anything.
    let conversation = kit.conversation_of("needs-5").await;
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    crate::domain::needs::refresh(
        &mut tx,
        &kit.state.events,
        &workspace.to_string(),
        conversation.parse().unwrap(),
        &crate::domain::needs::NeedSettings::new(
            36_500,
            &crate::domain::policy::Policy::baseline(),
        ),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(need_events(&db, workspace).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_upgrade_withdraws_the_needs_the_old_rules_raised(db: PgPool) {
    // The retired rules, as they read a shipping notice: a reply needed by
    // a date, sure enough to act.
    let old_rules = Scripted::rules([(
        "suivi@colis-rapide.example",
        vec![
            (triage::NEEDS_REPLY, AnswerValue::Noul(true), 0.95),
            (triage::HAS_DEADLINE, AnswerValue::Noul(true), 0.95),
        ],
    )]);
    let deps = without_model().with_rules(Arc::new(old_rules)).with_policy(
        Policy::baseline()
            .with(triage::NEEDS_REPLY, Source::Rules, Switch::ActAbove(0.9))
            .with(triage::HAS_DEADLINE, Source::Rules, Switch::ActAbove(0.9)),
    );
    let kit = Kit::with_deps(db.clone(), deps).await;
    let (workspace, _) = mailbox(&kit, &["fr"]).await;
    assert_eq!(
        needs_of(&db, workspace).await,
        [row("fr-1", "reply", "open", "")]
    );
    // Stored about the envelope the old rules read.
    sqlx::query("UPDATE conversations.decisions SET state_version = 2 WHERE source = 'rules'")
        .execute(&db)
        .await
        .unwrap();

    upgrade(&db).await;
    let acted: Vec<bool> =
        sqlx::query_scalar("SELECT acted FROM conversations.decisions WHERE source = 'rules'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(acted, [false, false], "kept as a record, no longer acting");
    let queued: Vec<String> = sqlx::query_scalar("SELECT state FROM conversations.needs_backfills")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(queued, ["pending"]);

    let (kit, _model) = upgraded(&db, without_model()).await;
    tick(&kit).await;
    assert_eq!(
        needs_of(&db, workspace).await,
        [row("fr-1", "reply", "cleared", "superseded")]
    );
    let events = need_events(&db, workspace).await;
    assert_eq!(events.last().unwrap()["cleared_because"], "superseded");

    // Running it again changes nothing more.
    let before = need_events(&db, workspace).await.len();
    upgrade(&db).await;
    tick(&kit).await;
    assert_eq!(need_events(&db, workspace).await.len(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn recent_mail_no_checked_model_answered_is_read_again_once(db: PgPool) {
    // Stored before the upgrade, while no model was trusted: the facts'
    // answers only.
    let model = Gated::closed(Scripted::fixtures());
    let kit = Kit::with_deps(db.clone(), sorting_deps().with_model(model.clone())).await;
    let (workspace, _) = mailbox(&kit, &["needs"]).await;
    assert!(needs_of(&db, workspace).await.is_empty());

    // Upgraded, with a trusted model.
    upgrade(&db).await;
    model.open();
    tick(&kit).await;
    // The model was asked about each conversation's newest inbound message,
    // once: it sorted them and raised their needs.
    let asked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.decide_backlog'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(asked, 5);
    assert_eq!(
        needs_of(&db, workspace).await,
        [
            row("needs-1", "reply", "open", ""),
            row("needs-2", "do", "open", ""),
            row("needs-3", "pay", "open", ""),
            row("needs-4", "review", "open", ""),
            row("needs-5", "reply", "open", ""),
        ]
    );
    assert_eq!(
        kit.stream_of("needs-3").await,
        ("receipts".to_owned(), "model".to_owned())
    );

    // Again: every message has a checked answer now, so nothing is asked.
    upgrade(&db).await;
    tick(&kit).await;
    let asked_again: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.decide_backlog'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(asked_again, 5);
}

/// Only the maintenance scope reads the backfills across workspaces: the
/// kernel scope (which reaches the kernel's tables) sees none of them, and
/// neither does a transaction that set nothing (campbooks-cloud#196).
#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_maintenance_scope_reads_the_backfills(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    mailbox(&kit, &["needs"]).await;
    mailbox(&kit, &["needs"]).await;
    seed(&db).await;
    let count = "SELECT count(*) FROM conversations.needs_backfills";
    let owner: i64 = sqlx::query_scalar(count).fetch_one(&db).await.unwrap();
    assert_eq!(owner, 2);
    let kernel = campbooks_kernel::testing::kernel_scope_pool(&db).await;
    let seen: i64 = sqlx::query_scalar(count).fetch_one(&kernel).await.unwrap();
    assert_eq!(seen, 0, "the kernel scope");
    let seen: i64 = sqlx::query_scalar(count)
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(seen, 0, "nothing set");
    let mut tx = campbooks_kernel::db::begin_maintenance(&kit.state.db)
        .await
        .unwrap();
    let seen: i64 = sqlx::query_scalar(count).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(seen, 2, "the maintenance scope");
    tx.rollback().await.unwrap();
}

/// The dispatcher's page jobs are each enqueued in their inbox's workspace,
/// and leave its transaction working in none (campbooks-cloud#207).
#[sqlx::test(migrations = "../../../migrations")]
async fn the_dispatcher_leaves_no_workspace_set(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let first = mailbox(&kit, &["needs"]).await;
    let second = mailbox(&kit, &["needs"]).await;
    let running: Vec<(WorkspaceId, super::Page)> = [first, second]
        .into_iter()
        .map(|(workspace, inbox)| (workspace, super::Page { inbox, after: None }))
        .collect();
    let mut tx = campbooks_kernel::db::begin_maintenance(&kit.state.db)
        .await
        .unwrap();
    let queued = super::queue_pages(&mut tx, &running).await.unwrap();
    assert_eq!(queued, 2);
    let workspace: Option<String> =
        sqlx::query_scalar("SELECT nullif(current_setting('campbooks.workspace_id', true), '')")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(workspace, None);
    tx.rollback().await.unwrap();
}

/// A strategy that remembers which questions it was asked, answering as
/// `inner` does.
struct Recording {
    inner: Scripted,
    asked: std::sync::Mutex<Vec<Vec<String>>>,
}

impl Strategy for Recording {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, campbooks_ai::Error>> {
        self.asked
            .lock()
            .unwrap()
            .push(questions.iter().map(|q| q.id.clone()).collect());
        self.inner.decide(envelope, questions)
    }
}

/// Fail with what failed: each failed job's kind and error.
async fn failed_jobs(db: &PgPool) {
    let failed: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT kind, last_error FROM kernel.jobs WHERE state = 'failed'")
            .fetch_all(db)
            .await
            .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
}

/// [`tick`], saying which job failed, if one did.
async fn tick_reporting(kit: &Kit) {
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchNeedsBackfills, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    campbooks_kernel::jobs::Worker::new(kit.state.clone())
        .run_until_idle()
        .await;
    failed_jobs(&kit.db).await;
}

/// campbooks-cloud#293: mail stored before whose move it is was asked, with
/// no open need, is asked only the move questions once on upgrade, and may
/// then wait on someone; running it again asks nothing more.
#[sqlx::test(migrations = "../../../migrations")]
async fn the_upgrade_asks_recent_mail_whose_move_it_is_once(db: PgPool) {
    use crate::testkit::{JOAO, joao_asks_ana, waiting_deps};

    let mut joao = script("primary", false, false, false, 0.0, false);
    joao.push((
        "next_move",
        AnswerValue::Choice("someone_else".to_owned()),
        0.91,
    ));
    joao.push(("awaited", AnswerValue::Choice("p1".to_owned()), 0.9));
    let recording = Arc::new(Recording {
        inner: Scripted::model([(JOAO.1, joao)]),
        asked: std::sync::Mutex::new(Vec::new()),
    });
    let kit = Kit::with_deps(db.clone(), waiting_deps().with_model(recording.clone())).await;
    let (workspace, _, _) = joao_asks_ana(&kit).await;
    // Make it look as it did before the upgrade (the catalogue off): the
    // triage answers only, no need and no need event.
    for statement in [
        "DELETE FROM conversations.decisions WHERE question IN ('next_move', 'awaited')",
        "DELETE FROM conversations.needs",
        "DELETE FROM kernel.events WHERE name = 'conversations.need_changed'",
    ] {
        sqlx::query(statement).execute(&db).await.unwrap();
    }
    assert!(needs_of(&db, workspace).await.is_empty());
    recording.asked.lock().unwrap().clear();

    // The upgraded server: the migration's seed, a tick.
    let migration = std::fs::read_to_string(format!(
        "{}/../../../migrations/20261023090000_conversations__needs_waiting.sql",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let start = migration.find("-- seed begin").unwrap();
    let end = migration.find("-- seed end").unwrap();
    // The migration's own text, from the repository.
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration[start..end].to_owned()))
        .execute(&db)
        .await
        .unwrap();
    tick_reporting(&kit).await;

    // Asked once, the move questions only.
    let asked = recording.asked.lock().unwrap().clone();
    assert_eq!(asked, [vec!["next_move".to_owned(), "awaited".to_owned()]]);
    assert_eq!(
        needs_of(&db, workspace).await,
        [(
            "joao-1".to_owned(),
            "waiting".to_owned(),
            "open".to_owned(),
            String::new()
        )]
    );
    // The triage answers stay as they were: one model answer each.
    let replies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.decisions
          WHERE question = 'needs_reply' AND source = 'model'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(replies, 1);

    // Again: nothing more is asked.
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration[start..end].to_owned()))
        .execute(&db)
        .await
        .unwrap();
    tick_reporting(&kit).await;
    assert_eq!(recording.asked.lock().unwrap().len(), 1);
}
