//! The event log and job queue against a real Postgres (`#[sqlx::test]`
//! gives each test a fresh database with every migration applied).

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa_axum::router::OpenApiRouter;

use super::*;
use crate::{
    AppState, Module,
    events::{DomainEvent, Event, EventHandler, EventId, ReplayError, ReplayFrom},
    testing,
};

// --- fixtures ----------------------------------------------------------------

/// A job whose behaviour its payload scripts. It records each successful
/// run in `job_log` through the job transaction, so a failed attempt leaves
/// no row behind.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Probe {
    note: String,
    /// Fail (retryably) while the attempt number is at most this.
    fail_first: u32,
    fatal: bool,
    panic: bool,
    sleep_ms: u64,
}

impl Probe {
    fn note(note: &str) -> Self {
        Self {
            note: note.to_owned(),
            ..Self::default()
        }
    }
}

impl Job for Probe {
    const NAME: &'static str = "test.probe";
    const QUEUE: &'static str = "default";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        if self.sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.sleep_ms)).await;
        }
        assert!(!self.panic, "probe told to panic");
        let attempt = i32::try_from(ctx.attempt()).unwrap_or_default();
        let fail = self.fatal || ctx.attempt() <= self.fail_first;
        log(ctx, Self::NAME, &self.note, attempt).await?;
        if self.fatal {
            return Err(JobError::fatal("probe told to fail"));
        }
        if fail {
            return Err(JobError::retry("not yet"));
        }
        Ok(())
    }
}

/// Write a `job_log` row in the job's transaction, recording the workspace
/// setting the transaction carries.
async fn log(ctx: &mut JobContext, kind: &str, note: &str, attempt: i32) -> Result<(), JobError> {
    let tx = ctx.tx().await?;
    sqlx::query(
        "INSERT INTO job_log (kind, note, workspace, attempt)
         VALUES ($1, $2, nullif(current_setting('campbooks.workspace_id', true), ''), $3)",
    )
    .bind(kind)
    .bind(note)
    .bind(attempt)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pinged {
    workspace: Option<String>,
    n: u32,
}

impl DomainEvent for Pinged {
    const NAME: &'static str = "test.pinged";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        self.workspace.as_deref()
    }
}

struct CountA;

impl EventHandler for CountA {
    type Event = Pinged;
    const NAME: &'static str = "test.count_a";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<Pinged>) -> Result<(), JobError> {
        log(ctx, Self::NAME, &event.payload.n.to_string(), 1).await
    }
}

struct CountB;

impl EventHandler for CountB {
    type Event = Pinged;
    const NAME: &'static str = "test.count_b";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<Pinged>) -> Result<(), JobError> {
        log(ctx, Self::NAME, &event.payload.n.to_string(), 1).await
    }
}

fn module(background: Background) -> Module {
    Module {
        name: "test",
        scopes: &[],
        router: OpenApiRouter::new(),
        background,
    }
}

fn background() -> Background {
    Background::new()
        .job::<Probe>()
        .on_event::<CountA>()
        .on_event::<CountB>()
}

/// State over `db` with the test module registered and `tweak` applied to
/// the jobs config. Also creates the `job_log` table.
async fn state_with(db: PgPool, tweak: impl FnOnce(&mut crate::config::JobsConfig)) -> AppState {
    sqlx::query(
        "CREATE TABLE job_log (
             kind text NOT NULL, note text NOT NULL, workspace text,
             attempt integer NOT NULL, at timestamptz NOT NULL DEFAULT clock_timestamp())",
    )
    .execute(&db)
    .await
    .unwrap();
    let mut config = testing::config("postgres://test@localhost/test");
    tweak(&mut config.jobs);
    let registry = Registry::build(&[module(background())]).unwrap();
    AppState::new(db, config).with_registry(registry)
}

async fn state(db: PgPool) -> AppState {
    state_with(db, |_| {}).await
}

async fn enqueue_probe(state: &AppState, probe: &Probe, options: EnqueueOptions) -> Enqueued {
    let mut tx = state.db.begin().await.unwrap();
    let enqueued = enqueue(&mut tx, probe, options).await.unwrap();
    tx.commit().await.unwrap();
    enqueued
}

fn created(enqueued: Enqueued) -> JobId {
    match enqueued {
        Enqueued::Created(id) => id,
        Enqueued::Duplicate => panic!("expected a new job"),
    }
}

async fn publish(state: &AppState, event: &Pinged) -> EventId {
    let mut tx = state.db.begin().await.unwrap();
    let id = state.events.publish(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
    id
}

#[derive(Debug, sqlx::FromRow)]
struct JobRow {
    state: String,
    attempts: i32,
    run_at: DateTime<Utc>,
    last_error: Option<String>,
    locked_by: Option<String>,
    finished_at: Option<DateTime<Utc>>,
}

async fn job(db: &PgPool, id: JobId) -> JobRow {
    sqlx::query_as(
        "SELECT state, attempts, run_at, last_error, locked_by, finished_at
           FROM kernel.jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

/// `notes` in any order: jobs without an ordering key run concurrently.
async fn sorted_notes(db: &PgPool, kind: &str) -> Vec<String> {
    let mut notes = notes(db, kind).await;
    notes.sort();
    notes
}

async fn notes(db: &PgPool, kind: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT note FROM job_log WHERE kind = $1 ORDER BY at")
        .bind(kind)
        .fetch_all(db)
        .await
        .unwrap()
}

// --- events ------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn publish_is_transactional(db: PgPool) {
    let state = state(db.clone()).await;
    let event = Pinged {
        workspace: Some("ws-1".into()),
        n: 1,
    };

    let mut tx = db.begin().await.unwrap();
    state.events.publish(&mut tx, &event).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.events").await, 0);
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.jobs").await, 0);

    let id = publish(&state, &event).await;
    let (name, version, workspace): (String, i32, Option<String>) =
        sqlx::query_as("SELECT name, version, workspace_id FROM kernel.events WHERE id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        (name.as_str(), version, workspace.as_deref()),
        ("test.pinged", 1, Some("ws-1"))
    );
    // One delivery per consumer, in the event's workspace.
    let deliveries: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT kind, queue, workspace_id FROM kernel.jobs WHERE event_id = $1 ORDER BY kind",
    )
    .bind(id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        deliveries,
        vec![
            ("test.count_a".into(), "events".into(), Some("ws-1".into())),
            ("test.count_b".into(), "events".into(), Some("ws-1".into())),
        ]
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn fan_out_runs_every_consumer(db: PgPool) {
    let state = state(db.clone()).await;
    publish(
        &state,
        &Pinged {
            workspace: Some("ws-1".into()),
            n: 7,
        },
    )
    .await;

    let stats = Worker::new(state).run_until_idle().await;
    assert_eq!(stats.succeeded, 2);
    assert_eq!(notes(&db, "test.count_a").await, ["7"]);
    assert_eq!(notes(&db, "test.count_b").await, ["7"]);
    let workspaces: Vec<Option<String>> = sqlx::query_scalar("SELECT workspace FROM job_log")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(workspaces, [Some("ws-1".into()), Some("ws-1".into())]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn events_without_consumers_are_still_logged(db: PgPool) {
    let state = AppState::new(
        db.clone(),
        testing::config("postgres://test@localhost/test"),
    );
    publish(
        &state,
        &Pinged {
            workspace: None,
            n: 1,
        },
    )
    .await;
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.events").await, 1);
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.jobs").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn replay_reenqueues_one_consumer(db: PgPool) {
    let state = state(db.clone()).await;
    let mut ids = Vec::new();
    for n in 1..=3 {
        ids.push(publish(&state, &Pinged { workspace: None, n }).await);
    }
    let worker = Worker::new(state.clone());
    worker.run_until_idle().await;
    assert_eq!(sorted_notes(&db, "test.count_a").await, ["1", "2", "3"]);

    let events = &state.events;
    assert_eq!(
        events
            .replay(&db, "test.count_a", ReplayFrom::Beginning)
            .await
            .unwrap(),
        3
    );
    // Already pending: a second replay adds nothing.
    assert_eq!(
        events
            .replay(&db, "test.count_a", ReplayFrom::Beginning)
            .await
            .unwrap(),
        0
    );
    worker.run_until_idle().await;
    assert_eq!(
        sorted_notes(&db, "test.count_a").await,
        ["1", "1", "2", "2", "3", "3"]
    );
    assert_eq!(
        sorted_notes(&db, "test.count_b").await,
        ["1", "2", "3"],
        "other consumers untouched"
    );

    assert_eq!(
        events
            .replay(&db, "test.count_b", ReplayFrom::Event(ids[2]))
            .await
            .unwrap(),
        1
    );
    let future = Utc::now() + TimeDelta::hours(1);
    assert_eq!(
        events
            .replay(&db, "test.count_b", ReplayFrom::Time(future))
            .await
            .unwrap(),
        0
    );
    assert!(matches!(
        events
            .replay(&db, "test.nobody", ReplayFrom::Beginning)
            .await,
        Err(ReplayError::UnknownConsumer(_))
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn delivery_of_an_event_with_another_version_fails(db: PgPool) {
    let state = state(db.clone()).await;
    let id = publish(
        &state,
        &Pinged {
            workspace: None,
            n: 1,
        },
    )
    .await;
    sqlx::query("UPDATE kernel.events SET version = 2 WHERE id = $1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let stats = Worker::new(state).run_until_idle().await;
    assert_eq!(stats.failed, 2);
    let error: String = sqlx::query_scalar("SELECT last_error FROM kernel.jobs LIMIT 1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(error.contains("has version 2"), "{error}");
}

// --- running, retrying, failing ----------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn handler_transaction_carries_the_workspace(db: PgPool) {
    let state = state(db.clone()).await;
    enqueue_probe(
        &state,
        &Probe::note("tenant"),
        EnqueueOptions::for_workspace("ws-42"),
    )
    .await;
    enqueue_probe(&state, &Probe::note("instance"), EnqueueOptions::default()).await;
    Worker::new(state).run_until_idle().await;

    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT note, workspace FROM job_log ORDER BY note")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [
            ("instance".into(), None),
            ("tenant".into(), Some("ws-42".into()))
        ]
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn retry_backs_off_then_succeeds(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.retry_base_ms = 60_000).await;
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                fail_first: 1,
                ..Probe::note("flaky")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let worker = Worker::new(state);

    let before = Utc::now();
    let stats = worker.run_until_idle().await;
    assert_eq!((stats.claimed, stats.retried), (1, 1));
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("pending", 1));
    assert_eq!(row.last_error.as_deref(), Some("not yet"));
    assert!(row.locked_by.is_none());
    // First retry: base (60 s) with jitter in [50 %, 100 %].
    assert!(
        row.run_at >= before + TimeDelta::seconds(29),
        "{}",
        row.run_at
    );
    assert!(
        row.run_at <= Utc::now() + TimeDelta::seconds(61),
        "{}",
        row.run_at
    );
    assert!(
        notes(&db, Probe::NAME).await.is_empty(),
        "the failed attempt's writes rolled back"
    );

    sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE id = $1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    worker.run_until_idle().await;
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("succeeded", 2));
    assert!(row.finished_at.is_some());
    assert_eq!(notes(&db, Probe::NAME).await, ["flaky"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn fatal_errors_mark_the_job_dead(db: PgPool) {
    let state = state(db.clone()).await;
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                fatal: true,
                ..Probe::note("doomed")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let stats = Worker::new(state).run_until_idle().await;
    assert_eq!(stats.failed, 1);
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("failed", 1));
    assert_eq!(row.last_error.as_deref(), Some("probe told to fail"));
    assert!(row.finished_at.is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn running_out_of_attempts_marks_the_job_dead(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.retry_base_ms = 1).await;
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                fail_first: 99,
                ..Probe::note("never")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let worker = Worker::new(state);
    for _ in 0..Probe::MAX_ATTEMPTS {
        sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE id = $1")
            .bind(id)
            .execute(&db)
            .await
            .unwrap();
        worker.run_until_idle().await;
    }
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("failed", 3));
    assert_eq!(worker.stats().retried, 2);
    assert_eq!(worker.stats().failed, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_panicking_handler_is_retried(db: PgPool) {
    let state = state(db.clone()).await;
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                panic: true,
                ..Probe::note("boom")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    Worker::new(state).run_until_idle().await;
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("pending", 1));
    assert_eq!(row.last_error.as_deref(), Some("the handler panicked"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_kinds_fail_for_good(db: PgPool) {
    let state = state(db.clone()).await;
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, max_attempts)
         VALUES ('default', 'test.removed', 'null', 5)",
    )
    .execute(&db)
    .await
    .unwrap();
    Worker::new(state).run_until_idle().await;
    let (state, error): (String, String) =
        sqlx::query_as("SELECT state, last_error FROM kernel.jobs")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(state, "failed");
    assert!(error.contains("no handler is registered"), "{error}");
}

// --- leases, uniqueness, ordering --------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn expired_leases_are_reclaimed(db: PgPool) {
    let state = state(db.clone()).await;
    let id =
        created(enqueue_probe(&state, &Probe::note("orphan"), EnqueueOptions::default()).await);
    let crashed = Worker::new(state.clone());
    let claimed = crashed.claim_for_test("default", 1).await.unwrap();
    assert_eq!(claimed, [(id, None, 1)]);
    assert_eq!(job(&db, id).await.locked_by.as_deref(), Some(crashed.id()));

    // Its worker died: nobody renews the lease.
    sqlx::query("UPDATE kernel.jobs SET locked_until = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    let survivor = Worker::new(state);
    survivor.upkeep().await;
    let row = job(&db, id).await;
    assert_eq!(row.state, "pending");
    assert!(row.last_error.unwrap().contains("lease expired"));

    survivor.run_until_idle().await;
    let row = job(&db, id).await;
    assert_eq!((row.state.as_str(), row.attempts), ("succeeded", 2));
    // The original holder can no longer complete it.
    assert!(!crashed.complete_for_test(id, 1).await.unwrap());
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_workers_never_claim_the_same_job(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.per_workspace = 10_000).await;
    flood(&state, "ws", 200).await;
    let (a, b) = (Worker::new(state.clone()), Worker::new(state));
    let mut claimed = Vec::new();
    for _ in 0..4 {
        let (x, y) = tokio::join!(
            a.claim_for_test("default", 30),
            b.claim_for_test("default", 30)
        );
        claimed.extend(x.unwrap().into_iter().map(|(id, _, _)| id));
        claimed.extend(y.unwrap().into_iter().map(|(id, _, _)| id));
    }
    let total = claimed.len();
    claimed.sort_unstable();
    claimed.dedup();
    assert_eq!(claimed.len(), total, "a job was claimed twice");
    assert_eq!(total, 200);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_worker_that_lost_the_lease_rolls_its_work_back(db: PgPool) {
    let state = state(db.clone()).await;
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                sleep_ms: 500,
                ..Probe::note("stale")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let worker = Worker::new(state);
    let running = tokio::spawn({
        let worker = worker.clone();
        async move { worker.run_until_idle().await }
    });
    wait_for_state(&db, id, "running").await;
    // Another worker took the job over (as after a lease expiry).
    sqlx::query(
        "UPDATE kernel.jobs SET locked_by = 'other', attempts = attempts + 1 WHERE id = $1",
    )
    .bind(id)
    .execute(&db)
    .await
    .unwrap();

    let stats = running.await.unwrap();
    assert_eq!((stats.succeeded, stats.lost), (0, 1));
    assert!(
        notes(&db, Probe::NAME).await.is_empty(),
        "its writes rolled back"
    );
    assert_eq!(job(&db, id).await.locked_by.as_deref(), Some("other"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unique_keys_dedupe_unfinished_jobs(db: PgPool) {
    let state = state(db.clone()).await;
    let options = EnqueueOptions::default().unique("mailbox-1");
    let first = enqueue_probe(&state, &Probe::note("a"), options.clone()).await;
    assert!(matches!(first, Enqueued::Created(_)));
    assert_eq!(
        enqueue_probe(&state, &Probe::note("b"), options.clone()).await,
        Enqueued::Duplicate
    );
    // Another key is another job.
    assert!(matches!(
        enqueue_probe(
            &state,
            &Probe::note("c"),
            EnqueueOptions::default().unique("mailbox-2")
        )
        .await,
        Enqueued::Created(_)
    ));

    Worker::new(state.clone()).run_until_idle().await;
    // Once finished, the key is free again.
    assert!(matches!(
        enqueue_probe(&state, &Probe::note("d"), options).await,
        Enqueued::Created(_)
    ));
    assert_eq!(notes(&db, Probe::NAME).await.len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn ordering_keys_run_one_at_a_time_in_order(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.concurrency = 8).await;
    let mut ordered = Vec::new();
    for n in 1..=3 {
        ordered.push(created(
            enqueue_probe(
                &state,
                &Probe::note(&format!("t{n}")),
                EnqueueOptions::default().ordered_by("thread-1"),
            )
            .await,
        ));
    }
    let other =
        created(enqueue_probe(&state, &Probe::note("other"), EnqueueOptions::default()).await);

    let worker = Worker::new(state.clone());
    let claimed: Vec<JobId> = worker
        .claim_for_test("default", 10)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(claimed.len(), 2, "{claimed:?}");
    assert!(claimed.contains(&ordered[0]) && claimed.contains(&other));
    // While the first is running, the rest of its key waits.
    assert!(
        worker
            .claim_for_test("default", 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(worker.complete_for_test(ordered[0], 1).await.unwrap());
    let next = worker.claim_for_test("default", 10).await.unwrap();
    assert_eq!(next, [(ordered[1], None, 1)]);
    assert!(worker.complete_for_test(ordered[1], 1).await.unwrap());

    // With many slots, a run still executes the key's jobs in enqueue order.
    for n in 4..=8 {
        enqueue_probe(
            &state,
            &Probe::note(&format!("t{n}")),
            EnqueueOptions::default().ordered_by("thread-1"),
        )
        .await;
    }
    worker.run_until_idle().await;
    assert_eq!(
        notes(&db, Probe::NAME).await,
        ["t3", "t4", "t5", "t6", "t7", "t8"]
    );
}

// --- fairness ----------------------------------------------------------------

async fn flood(state: &AppState, workspace: &str, jobs: usize) {
    let mut tx = state.db.begin().await.unwrap();
    for n in 0..jobs {
        enqueue(
            &mut tx,
            &Probe::note(&format!("{workspace}-{n}")),
            EnqueueOptions::for_workspace(workspace),
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_busy_workspace_does_not_block_another_beyond_one_claim(db: PgPool) {
    // No per-workspace cap, one job per claim: only the claim order is fair.
    let state = state_with(db.clone(), |jobs| jobs.per_workspace = 10_000).await;
    flood(&state, "busy", 1_000).await;
    flood(&state, "quiet", 1).await;
    let worker = Worker::new(state);

    let first = worker.claim_for_test("default", 1).await.unwrap();
    let (id, workspace, attempt) = first[0].clone();
    assert!(worker.complete_for_test(id, attempt).await.unwrap());
    let second = worker.claim_for_test("default", 1).await.unwrap();
    let served = [workspace, second[0].1.clone()];
    assert!(
        served.contains(&Some("quiet".to_owned())),
        "the quiet workspace waited more than one claim: {served:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn one_claim_takes_every_workspace_s_first_job_first(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.per_workspace = 10_000).await;
    flood(&state, "busy", 1_000).await;
    flood(&state, "quiet", 1).await;
    let claimed = Worker::new(state)
        .claim_for_test("default", 2)
        .await
        .unwrap();
    let mut workspaces: Vec<_> = claimed.into_iter().map(|(_, w, _)| w).collect();
    workspaces.sort();
    assert_eq!(workspaces, [Some("busy".into()), Some("quiet".into())]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn per_workspace_limit_caps_running_jobs(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.per_workspace = 2).await;
    flood(&state, "busy", 10).await;
    flood(&state, "quiet", 3).await;
    let worker = Worker::new(state);
    let claimed = worker.claim_for_test("default", 100).await.unwrap();
    let busy = claimed
        .iter()
        .filter(|(_, w, _)| w.as_deref() == Some("busy"))
        .count();
    assert_eq!((claimed.len(), busy), (4, 2));
    // Both workspaces are at their cap until something finishes.
    assert!(
        worker
            .claim_for_test("default", 100)
            .await
            .unwrap()
            .is_empty()
    );
}

// --- schedules ---------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn schedules_enqueue_each_tick_once_across_workers(db: PgPool) {
    let state = state(db.clone()).await;
    let a = Worker::new(state.clone());
    let b = Worker::new(state);
    for _ in 0..3 {
        tokio::join!(a.upkeep(), b.upkeep());
    }
    let prunes = "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.prune_jobs'";
    assert_eq!(count(&db, prunes).await, 1);

    a.run_until_idle().await;
    tokio::join!(a.upkeep(), b.upkeep());
    assert_eq!(count(&db, prunes).await, 1, "same tick: not enqueued again");
    let done =
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.prune_jobs' AND state = 'succeeded'";
    assert_eq!(count(&db, done).await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn prune_deletes_old_succeeded_jobs_only(db: PgPool) {
    let state = state(db.clone()).await;
    for note in ["old", "recent"] {
        enqueue_probe(&state, &Probe::note(note), EnqueueOptions::default()).await;
    }
    enqueue_probe(
        &state,
        &Probe {
            fatal: true,
            ..Probe::note("dead")
        },
        EnqueueOptions::default(),
    )
    .await;
    let worker = Worker::new(state.clone());
    worker.run_until_idle().await;
    sqlx::query(
        "UPDATE kernel.jobs SET finished_at = now() - interval '30 days'
          WHERE payload->>'note' IN ('old', 'dead')",
    )
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    enqueue(&mut tx, &PruneJobs, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    worker.run_until_idle().await;

    let left: Vec<String> = sqlx::query_scalar(
        "SELECT payload->>'note' FROM kernel.jobs WHERE kind = 'test.probe' ORDER BY 1",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(left, ["dead", "recent"]);
}

// --- the run loop ------------------------------------------------------------

async fn wait_for_state(db: &PgPool, id: JobId, wanted: &str) {
    for _ in 0..200 {
        if job(db, id).await.state == wanted {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} never reached `{wanted}`: {:?}", job(db, id).await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn enqueue_notifications_wake_an_idle_worker(db: PgPool) {
    // A poll interval far longer than the test: only NOTIFY can wake it.
    let state = state_with(db.clone(), |jobs| jobs.poll_interval_ms = 600_000).await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(Worker::new(state.clone()).run(async {
        let _ = stopped.await;
    }));
    // Wait until the worker's listener connection has subscribed.
    for _ in 0..200 {
        let listening = count(
            &db,
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND query ILIKE 'listen%'",
        )
        .await;
        if listening > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let enqueued_at = std::time::Instant::now();
    let id = created(enqueue_probe(&state, &Probe::note("wake"), EnqueueOptions::default()).await);
    wait_for_state(&db, id, "succeeded").await;
    // Well before the worker's next timed wake-up (upkeep, at 5 s).
    assert!(
        enqueued_at.elapsed() < Duration::from_secs(2),
        "{:?}",
        enqueued_at.elapsed()
    );
    stop.send(()).unwrap();
    // The probe, and the kernel's prune scheduled at start-up.
    assert_eq!(worker.await.unwrap().succeeded, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn shutdown_finishes_quick_jobs_and_releases_slow_ones(db: PgPool) {
    let state = state_with(db.clone(), |jobs| {
        jobs.shutdown_grace_seconds = 1;
        jobs.poll_interval_ms = 50;
    })
    .await;
    let quick = created(
        enqueue_probe(
            &state,
            &Probe {
                sleep_ms: 300,
                ..Probe::note("quick")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let slow = created(
        enqueue_probe(
            &state,
            &Probe {
                sleep_ms: 60_000,
                ..Probe::note("slow")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(Worker::new(state).run(async {
        let _ = stopped.await;
    }));
    wait_for_state(&db, quick, "running").await;
    wait_for_state(&db, slow, "running").await;

    stop.send(()).unwrap();
    let stats = worker.await.unwrap();
    // The quick probe and the kernel's prune scheduled at start-up.
    assert_eq!((stats.succeeded, stats.released), (2, 1));
    assert_eq!(job(&db, quick).await.state, "succeeded");
    let slow = job(&db, slow).await;
    assert_eq!((slow.state.as_str(), slow.attempts), ("pending", 0));
    assert!(slow.locked_by.is_none());
}

// --- registrations -----------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Misnamed;

impl Job for Misnamed {
    const NAME: &'static str = "other.misnamed";
    const QUEUE: &'static str = "Bad Queue";
    const MAX_ATTEMPTS: u32 = 0;

    async fn run(self, _ctx: &mut JobContext) -> Result<(), JobError> {
        Ok(())
    }
}

#[test]
fn registry_collects_kernel_and_module_work() {
    let registry = Registry::build(&[module(background())]).unwrap();
    assert_eq!(
        registry.kind_names(),
        [
            "kernel.prune_jobs",
            "test.count_a",
            "test.count_b",
            "test.probe"
        ]
    );
    assert_eq!(registry.schedule_names(), ["kernel.prune_jobs"]);
    assert_eq!(registry.queues(), ["default", "events", "maintenance"]);
    let consumers: Vec<_> = registry
        .consumers_of("test.pinged")
        .iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(consumers, ["test.count_a", "test.count_b"]);
    assert_eq!(
        registry.consumer("test.count_b").map(|c| c.event),
        Some("test.pinged")
    );
}

#[test]
fn registry_rejects_bad_registrations() {
    let err = Registry::build(&[module(
        Background::new()
            .job::<Probe>()
            .job::<Probe>()
            .job::<Misnamed>()
            .schedule::<PruneJobs>(Duration::from_millis(10)),
    )])
    .unwrap_err();
    let all = err.0.join("\n");
    assert!(all.contains("`test.probe` is registered twice"), "{all}");
    assert!(
        all.contains("`other.misnamed`: must be `test.<name>`"),
        "{all}"
    );
    assert!(all.contains("queue `Bad Queue`"), "{all}");
    assert!(all.contains("MAX_ATTEMPTS must be at least 1"), "{all}");
    assert!(all.contains("`kernel.prune_jobs`"), "{all}");
}

#[test]
fn registry_takes_work_owned_outside_modules() {
    #[derive(Debug, Default, Serialize, Deserialize)]
    struct Sweep;
    impl Job for Sweep {
        const NAME: &'static str = "web.sweep";
        const QUEUE: &'static str = "maintenance";
        async fn run(self, _ctx: &mut JobContext) -> Result<(), JobError> {
            Ok(())
        }
    }
    let web = Background::new().schedule::<Sweep>(Duration::from_mins(15));
    let registry = Registry::build_with(&[], &[("web", &web)]).unwrap();
    assert_eq!(
        registry.schedule_names(),
        ["kernel.prune_jobs", "web.sweep"]
    );

    let err = Registry::build_with(&[], &[("other", &web)]).unwrap_err();
    assert!(err.0.join("\n").contains("must be `other.<name>`"), "{err}");
}

#[test]
fn errors_are_truncated() {
    let long = "x".repeat(MAX_ERROR_LEN + 10);
    assert_eq!(truncate_error(&long).chars().count(), MAX_ERROR_LEN + 1);
    assert_eq!(truncate_error("short"), "short");
}
