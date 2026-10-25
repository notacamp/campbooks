//! Needs that change with time alone (campbooks-cloud#275), over
//! `fixtures/needs.json`: the clock each refresh writes, the dispatcher in
//! the maintenance scope, and the workspace's wake job.

use campbooks_kernel::{
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde_json::Value;
use sqlx::PgPool;

use super::DispatchNeedClocks;
use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::{Kit, sorting_deps},
};

/// Sam's inbox holding `fixtures/needs.json`, raised under the testkit's
/// wide window (the fixtures' dates never drift out of it).
async fn sams_needs(kit: &Kit) -> (WorkspaceId, InboxId, String) {
    let workspace = WorkspaceId::new();
    let (token, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    (workspace, inbox, token)
}

/// The same module and database, settling needs under the shipped 14-day
/// window: the fixtures' mail is older than that by now or soon, and the
/// tests make it so by moving its sent time back.
async fn shipped_window(db: PgPool) -> Kit {
    let mut deps: ConversationsDeps = sorting_deps();
    deps.config.needs_window_days = 14;
    Kit::with_deps(db, deps).await
}

/// What the scheduler does every minute, then every job it causes.
async fn tick(kit: &Kit) {
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchNeedClocks, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// Move message `provider_message_id` back to `days` ago.
async fn sent_days_ago(kit: &Kit, provider_message_id: &str, days: i32) {
    sqlx::query(
        "UPDATE conversations.messages SET sent_at = now() - make_interval(days => $2)
          WHERE provider_message_id = $1",
    )
    .bind(provider_message_id)
    .bind(days)
    .execute(&kit.db)
    .await
    .unwrap();
}

/// Make the clock of `provider_message_id`'s conversation due now.
async fn due(kit: &Kit, provider_message_id: &str) {
    let conversation = kit.conversation_of(provider_message_id).await;
    let made = sqlx::query(
        "UPDATE conversations.need_clocks SET wake_at = now() - interval '1 minute'
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .execute(&kit.db)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(made, 1, "{provider_message_id} has no clock");
}

/// `(reason, trust, question, state, cleared_because)` of the need of
/// `provider_message_id`'s conversation.
async fn need(
    kit: &Kit,
    provider_message_id: &str,
) -> (String, String, String, String, Option<String>) {
    let conversation = kit.conversation_of(provider_message_id).await;
    sqlx::query_as(
        "SELECT reason, trust, question, state, cleared_because FROM conversations.needs
          WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

async fn clock(kit: &Kit, provider_message_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let conversation = kit.conversation_of(provider_message_id).await;
    sqlx::query_scalar(
        "SELECT wake_at FROM conversations.need_clocks WHERE conversation_id = $1::uuid",
    )
    .bind(&conversation)
    .fetch_optional(&kit.db)
    .await
    .unwrap()
}

async fn summary(kit: &Kit, token: &str, workspace: WorkspaceId, provider: &str) -> Value {
    let id = kit.conversation_of(provider).await;
    let list = kit
        .get(
            token,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    list.json["conversations"][0].clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn every_open_ask_gets_a_clock_and_a_bill_or_a_warning_none(db: PgPool) {
    let kit = Kit::new(db).await;
    sams_needs(&kit).await;
    // Marta's reply and Pedro's lease: when their mail leaves the window.
    assert!(clock(&kit, "needs-1").await.is_some());
    assert!(clock(&kit, "needs-2").await.is_some());
    // A bill never lapses nor goes stale; a warning keeps its own rules.
    assert_eq!(clock(&kit, "needs-3").await, None);
    assert_eq!(clock(&kit, "needs-4").await, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_old_undated_ask_becomes_still_to_do(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let before = kit.events("conversations.need_changed").await.len();

    // Three weeks on, under the shipped window, the clock is due.
    let shipped = shipped_window(db).await;
    sent_days_ago(&shipped, "needs-2", 21).await;
    due(&shipped, "needs-2").await;
    tick(&shipped).await;

    let (reason, trust, question, state, _) = need(&shipped, "needs-2").await;
    assert_eq!(
        (
            reason.as_str(),
            trust.as_str(),
            question.as_str(),
            state.as_str()
        ),
        ("review", "check", "still_to_do", "open")
    );
    // A question waits for the member: nothing more to wake for.
    assert_eq!(clock(&shipped, "needs-2").await, None);
    assert_eq!(
        shipped.events("conversations.need_changed").await.len(),
        before + 1
    );
    // Read through the first kit, whose identity provider issued Sam's token.
    let pedro = summary(&kit, &sam, workspace, "needs-2").await;
    assert_eq!(pedro["need"]["question"], "still_to_do");
    assert!(pedro["need"]["sent_at"].is_string());

    // Ticking again changes nothing.
    tick(&shipped).await;
    assert_eq!(
        shipped.events("conversations.need_changed").await.len(),
        before + 1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lapsed_ask_is_cleared_and_shows_as_expired(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = sams_needs(&kit).await;
    let message = kit.message_of("needs-1").await;
    // Scout read a lapse time; time went by since the need took it.
    sqlx::query(
        "INSERT INTO conversations.message_reads
             (message_id, workspace_id, layer, state, texts, lapses_at, dates_read_at, read_at)
         SELECT m.id, m.workspace_id, 'model', 'read', '{\"en\": \"Marta sent a link.\"}',
                now() - interval '5 minutes', now(), now()
           FROM conversations.messages m WHERE m.id = $1::uuid
         ON CONFLICT (message_id, layer) DO UPDATE
            SET lapses_at = EXCLUDED.lapses_at, dates_read_at = EXCLUDED.dates_read_at",
    )
    .bind(&message)
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE conversations.needs SET lapses_at = now() - interval '5 minutes'
          WHERE message_id = $1::uuid",
    )
    .bind(&message)
    .execute(&kit.db)
    .await
    .unwrap();

    // Before the clock wakes it: no need to act on, already Expired.
    let marta = summary(&kit, &sam, workspace, "needs-1").await;
    assert_eq!(marta["need"], Value::Null);
    assert!(marta["lapsed_need"]["lapsed_at"].is_string());

    due(&kit, "needs-1").await;
    tick(&kit).await;
    let (_, _, _, state, cleared) = need(&kit, "needs-1").await;
    assert_eq!(
        (state.as_str(), cleared.as_deref()),
        ("cleared", Some("lapsed"))
    );
    let marta = summary(&kit, &sam, workspace, "needs-1").await;
    assert_eq!(marta["need"], Value::Null);
    assert_eq!(marta["lapsed_need"]["reason"], "reply");
    let events = kit.events("conversations.need_changed").await;
    let last = events.last().unwrap();
    assert_eq!(last["cleared_because"], "lapsed");
    assert_eq!(last["need"], Value::Null);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_dispatcher_reads_only_due_clocks_in_the_maintenance_scope(db: PgPool) {
    let kit = Kit::new(db).await;
    sams_needs(&kit).await;
    // Nothing due: no job.
    tick(&kit).await;
    let woken: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.wake_needs'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(woken, 0);
    // Due: one job in the workspace, however many of its needs are due.
    due(&kit, "needs-1").await;
    due(&kit, "needs-2").await;
    tick(&kit).await;
    let woken: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT workspace_id FROM kernel.jobs WHERE kind = 'conversations.wake_needs'",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(woken.len(), 1, "{woken:?}");
    assert!(woken[0].is_some());
    // A transaction in no workspace and no scope sees no clock.
    let mut tx = kit.state.db.begin().await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.need_clocks")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 0);
    tx.rollback().await.unwrap();
}

/// The seed of the migration that added the facts `need_changed` carries
/// (campbooks-cloud#358), run again (as the migration's login).
async fn seed_need_facts(db: &PgPool) {
    let migration = std::fs::read_to_string(format!(
        "{}/../../../migrations/20261107000100_conversations__need_facts.sql",
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

/// campbooks-cloud#358: every open need announced before `need_changed`
/// carried whether it is a warning and whether a person sent it is
/// announced again with them, once, with no model asked.
#[sqlx::test(migrations = "../../../migrations")]
async fn open_needs_are_announced_again_with_the_new_facts(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _) = sams_needs(&kit).await;
    // As if announced before the facts existed.
    sqlx::query("UPDATE conversations.needs SET facts_version = 0 WHERE workspace_id = $1")
        .bind(workspace.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.needs WHERE workspace_id = $1 AND state = 'open'",
    )
    .bind(workspace.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(open > 0);
    let before = kit.events("conversations.need_changed").await.len();

    seed_need_facts(&kit.db).await;
    tick(&kit).await;
    let events = kit.events("conversations.need_changed").await;
    let again = &events[before..];
    assert_eq!(i64::try_from(again.len()).unwrap(), open, "{again:?}");
    assert!(
        again.iter().all(|e| e["need"]["from_person"].is_boolean()),
        "{again:?}"
    );
    assert_eq!(
        again
            .iter()
            .filter(|e| e["need"]["warning"] == true)
            .count(),
        1,
        "the phishing mail's warning"
    );

    // Run again: nothing more to say.
    seed_need_facts(&kit.db).await;
    tick(&kit).await;
    assert_eq!(
        kit.events("conversations.need_changed").await.len(),
        events.len()
    );
}
