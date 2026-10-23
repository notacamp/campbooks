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
    events::{
        DomainEvent, Event, EventCatalogue, EventConsumer, EventHandler, EventId, ReplayError,
        ReplayFrom,
    },
    testing,
};

// --- fixtures ----------------------------------------------------------------

/// A job whose behaviour its payload scripts. It records each successful
/// run in `job_log` through the job transaction, so a failed attempt leaves
/// no row behind.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "a test fixture scripted by its payload: each flag is one behaviour"
)]
struct Probe {
    note: String,
    /// Fail (retryably) while the attempt number is at most this.
    fail_first: u32,
    fatal: bool,
    panic: bool,
    sleep_ms: u64,
    /// Ask Postgres to read this text as a number (an error that quotes it).
    #[serde(default)]
    as_number: Option<String>,
    /// Fail with a message quoting this text, as a careless handler would
    /// quote a provider's reply.
    #[serde(default)]
    echo: Option<String>,
    /// Work in the kernel scope (`kernel_tx`), recording whether it was on.
    #[serde(default)]
    kernel: bool,
    /// Record the statement timeout of the job's transaction.
    #[serde(default)]
    limits: bool,
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
        if let Some(text) = &self.as_number {
            sqlx::query("SELECT $1::text::int")
                .bind(text)
                .fetch_one(ctx.pool())
                .await?;
        }
        if let Some(reply) = &self.echo {
            return Err(JobError::fatal(format!("provider answered 400: {reply}")));
        }
        if self.limits {
            let tx = ctx.tx().await?;
            let limit: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
                .fetch_one(&mut **tx)
                .await?;
            log(ctx, Self::NAME, &format!("statement_timeout={limit}"), 1).await?;
            return Ok(());
        }
        if self.kernel {
            let tx = ctx.kernel_tx().await?;
            let on: bool = sqlx::query_scalar(
                "SELECT coalesce(current_setting('campbooks.kernel_scope', true), '') = 'on'",
            )
            .fetch_one(&mut **tx)
            .await?;
            log(ctx, Self::NAME, &format!("kernel={on}"), 1).await?;
            return Ok(());
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct Pinged {
    workspace: Option<String>,
    n: u32,
}

impl DomainEvent for Pinged {
    const NAME: &'static str = "test.pinged";
    const VERSION: u16 = 1;
    // Its instance-wide ones may come from a workspace's transaction (the
    // tests below list them, and their deliveries, for row-level security).
    const PUBLISHED_IN_WORKSPACES: bool = true;

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

/// An event something waits on (`SETTLES`), whose one consumer always
/// fails.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct Departed {
    who: String,
}

impl DomainEvent for Departed {
    const NAME: &'static str = "test.departed";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;
    const SETTLES: bool = true;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

struct ForgetFails;

/// How a dead [`ForgetFails`] delivery is counted and logged.
const FORGETTING: Watch = Watch {
    code: "test_forgetting",
    died: "test forgetting delivery died",
};

impl EventHandler for ForgetFails {
    type Event = Departed;
    const NAME: &'static str = "test.forget_fails";
    const QUEUE: &'static str = "events";
    const WATCH: Option<Watch> = Some(FORGETTING);

    async fn handle(_ctx: &mut JobContext, _event: Event<Departed>) -> Result<(), JobError> {
        Err(JobError::fatal("cannot forget"))
    }
}

fn module(background: Background) -> Module {
    Module {
        name: "test",
        scopes: &[],
        router: OpenApiRouter::new(),
        admin_router: None,
        background,
        resources: crate::authz::Resources::default(),
    }
}

fn background() -> Background {
    Background::new()
        .job::<Probe>()
        .on_event::<CountA>()
        .on_event::<CountB>()
        .on_event::<ForgetFails>()
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
    if let Some(workspace) = &options.workspace {
        crate::db::set_workspace(&mut tx, workspace).await.unwrap();
    }
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
    if let Some(workspace) = crate::DomainEvent::workspace(event) {
        crate::db::set_workspace(&mut tx, workspace).await.unwrap();
    }
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
    crate::db::set_workspace(&mut tx, "ws-1").await.unwrap();
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
    // Already pending: a second replay adds nothing, read in batches of
    // two or not (campbooks-cloud#254).
    assert_eq!(
        events
            .replay(&db, "test.count_a", ReplayFrom::Beginning)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        events
            .replay_in_batches(&db, "test.count_a", ReplayFrom::Beginning, 2)
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
    worker.run_until_idle().await;
    // Every event, a batch of two and then one, each enqueued once.
    assert_eq!(
        events
            .replay_in_batches(&db, "test.count_b", ReplayFrom::Beginning, 2)
            .await
            .unwrap(),
        3
    );
    worker.run_until_idle().await;
    assert_eq!(
        sorted_notes(&db, "test.count_b").await,
        ["1", "1", "2", "2", "3", "3", "3"]
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

// --- handlers that own their dependencies -------------------------------------

/// A job handler composed with a prefix it writes in front of every note.
struct Stamp {
    prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StampPayload {
    note: String,
}

impl JobHandler for Stamp {
    type Payload = StampPayload;
    const NAME: &'static str = "test.stamp";
    const QUEUE: &'static str = "default";

    async fn run(&self, ctx: &mut JobContext, payload: StampPayload) -> Result<(), JobError> {
        let note = format!("{}{}", self.prefix, payload.note);
        log(ctx, Self::NAME, &note, 1).await
    }
}

/// An event consumer composed with a prefix.
struct StampPings {
    prefix: String,
}

impl EventConsumer for StampPings {
    type Event = Pinged;
    const NAME: &'static str = "test.stamp_pings";
    const QUEUE: &'static str = "events";

    async fn handle(&self, ctx: &mut JobContext, event: Event<Pinged>) -> Result<(), JobError> {
        let note = format!("{}{}", self.prefix, event.payload.n);
        log(ctx, Self::NAME, &note, 1).await
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn handlers_run_with_the_dependencies_they_were_composed_with(db: PgPool) {
    let base = state(db.clone()).await;
    let registry = Registry::build(&[module(
        background()
            .publishes::<Pinged>()
            .handler(Stamp {
                prefix: "job:".into(),
            })
            .consumer(StampPings {
                prefix: "event:".into(),
            }),
    )])
    .unwrap();
    assert!(registry.consumer("test.stamp_pings").is_some());
    let state = base.with_registry(registry);

    let mut tx = state.db.begin().await.unwrap();
    enqueue_to::<Stamp>(
        &mut tx,
        &StampPayload { note: "a".into() },
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    publish(
        &state,
        &Pinged {
            workspace: None,
            n: 7,
        },
    )
    .await;
    Worker::new(state).run_until_idle().await;

    assert_eq!(notes(&db, "test.stamp").await, ["job:a"]);
    assert_eq!(notes(&db, "test.stamp_pings").await, ["event:7"]);
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

/// Options in `workspace` (`None`: instance-wide).
fn in_workspace(workspace: Option<&str>) -> EnqueueOptions {
    EnqueueOptions {
        workspace: workspace.map(str::to_owned),
        ..EnqueueOptions::default()
    }
}

/// Unique keys are per workspace (campbooks-cloud#209): two workspaces
/// enqueue the same kind and key and both run, while within one workspace,
/// and among instance-wide jobs, the key still dedupes.
#[sqlx::test(migrations = "../../migrations")]
async fn unique_keys_are_per_workspace(db: PgPool) {
    let state = state(db.clone()).await;
    let key = "connectors.sync_stream:s-1";
    for (note, workspace) in [
        ("ana", Some("ana")),
        ("bob", Some("bob")),
        ("instance", None),
    ] {
        let options = in_workspace(workspace).unique(key);
        let first = enqueue_probe(&state, &Probe::note(note), options.clone()).await;
        assert!(matches!(first, Enqueued::Created(_)), "{note}: {first:?}");
        assert_eq!(
            enqueue_probe(&state, &Probe::note(&format!("{note} again")), options).await,
            Enqueued::Duplicate,
            "{note}: the key still dedupes there"
        );
    }
    let stats = Worker::new(state.clone()).run_until_idle().await;
    assert_eq!(stats.succeeded, 3, "{stats:?}");
    assert_eq!(
        sorted_notes(&db, Probe::NAME).await,
        ["ana", "bob", "instance"]
    );
}

/// Ordering keys are per workspace (campbooks-cloud#209): jobs of two
/// workspaces, and instance-wide ones, sharing an ordering key do not wait
/// for each other, at claim or at enqueue, while a workspace's own jobs
/// still run one at a time, in order.
#[sqlx::test(migrations = "../../migrations")]
async fn ordering_keys_are_per_workspace(db: PgPool) {
    let state = state_with(db.clone(), |jobs| jobs.concurrency = 8).await;
    let ordered = |workspace: Option<&str>| in_workspace(workspace).ordered_by("thread-1");
    let mut ana = Vec::new();
    for n in 1..=2 {
        ana.push(created(
            enqueue_probe(
                &state,
                &Probe::note(&format!("ana-{n}")),
                ordered(Some("ana")),
            )
            .await,
        ));
    }
    let bob = created(enqueue_probe(&state, &Probe::note("bob"), ordered(Some("bob"))).await);
    let instance = created(enqueue_probe(&state, &Probe::note("instance"), ordered(None)).await);

    let worker = Worker::new(state.clone());
    let claimed: Vec<JobId> = worker
        .claim_for_test("default", 10)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(claimed.len(), 3, "{claimed:?}");
    for id in [ana[0], bob, instance] {
        assert!(
            claimed.contains(&id),
            "{id} waited for another workspace's job"
        );
    }
    // Ana's second job still waits for her first.
    assert!(
        worker
            .claim_for_test("default", 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(worker.complete_for_test(ana[0], 1).await.unwrap());
    assert_eq!(
        worker.claim_for_test("default", 10).await.unwrap(),
        [(ana[1], Some("ana".to_owned()), 1)]
    );

    // An enqueuer of the key in an open transaction of ana's does not hold
    // up bob's enqueuer of the same key.
    let mut held = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut held, "ana").await.unwrap();
    enqueue(&mut held, &Probe::note("ana-3"), ordered(Some("ana")))
        .await
        .unwrap();
    let bob_again = tokio::time::timeout(
        Duration::from_secs(10),
        enqueue_probe(&state, &Probe::note("bob-2"), ordered(Some("bob"))),
    )
    .await;
    assert!(
        matches!(bob_again, Ok(Enqueued::Created(_))),
        "bob's enqueue waited for ana's transaction: {bob_again:?}"
    );
    held.rollback().await.unwrap();
}

// --- fairness ----------------------------------------------------------------

async fn flood(state: &AppState, workspace: &str, jobs: usize) {
    let mut tx = crate::db::begin_in(&state.db, workspace).await.unwrap();
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

/// A tick transaction that stalled for more than a period between its start
/// and its upsert counts the tick by the statement's clock, not the
/// transaction's start: it never moves a newer last tick back
/// (campbooks-cloud#211).
#[sqlx::test(migrations = "../../migrations")]
async fn a_stalled_tick_never_moves_the_last_tick_back(db: PgPool) {
    #[derive(Debug, Default, Serialize, Deserialize)]
    struct Tick;
    impl Job for Tick {
        const NAME: &'static str = "test.tick";
        const QUEUE: &'static str = "maintenance";
        async fn run(self, _ctx: &mut JobContext) -> Result<(), JobError> {
            Ok(())
        }
    }
    const EVERY: Duration = Duration::from_secs(1);
    let registry = Registry::build(&[module(Background::new().schedule::<Tick>(EVERY))]).unwrap();
    let state = AppState::new(
        db.clone(),
        testing::config("postgres://test@localhost/test"),
    )
    .with_registry(registry);
    let (stalled, live) = (Worker::new(state.clone()), Worker::new(state.clone()));
    let last = || async {
        sqlx::query_scalar::<_, i64>("SELECT last_tick FROM kernel.schedules WHERE name = $1")
            .bind(Tick::NAME)
            .fetch_optional(&db)
            .await
            .unwrap()
    };

    // One worker's tick transaction starts, then stalls past two periods
    // while another worker ticks.
    let mut tx = crate::db::begin_kernel(&state.db).await.unwrap();
    sqlx::query("SELECT now()").execute(&mut *tx).await.unwrap();
    tokio::time::sleep(EVERY * 5 / 2).await;
    live.upkeep().await;
    let newer = last().await.expect("the live worker ticked");

    // The stalled one wakes: by the clock this is the same tick, so it
    // records nothing and enqueues nothing.
    let recorded = stalled.tick_in_for_test(&mut tx, Tick::NAME).await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        recorded.is_none_or(|tick| tick >= newer),
        "moved back from {newer} to {recorded:?}"
    );
    assert!(last().await.unwrap() >= newer);
}

#[sqlx::test(migrations = "../../migrations")]
async fn prune_deletes_finished_jobs_past_their_retention(db: PgPool) {
    let state = state(db.clone()).await;
    for note in ["old", "recent"] {
        enqueue_probe(&state, &Probe::note(note), EnqueueOptions::default()).await;
    }
    for note in ["dead", "long-dead"] {
        enqueue_probe(
            &state,
            &Probe {
                fatal: true,
                ..Probe::note(note)
            },
            EnqueueOptions::default(),
        )
        .await;
    }
    let worker = Worker::new(state.clone());
    worker.run_until_idle().await;
    // Succeeded jobs go after 168 hours, failed ones after 30 days.
    sqlx::query(
        "UPDATE kernel.jobs SET finished_at = now() - CASE payload->>'note'
             WHEN 'old' THEN interval '8 days'
             WHEN 'dead' THEN interval '20 days'
             WHEN 'long-dead' THEN interval '31 days' END
          WHERE payload->>'note' IN ('old', 'dead', 'long-dead')",
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

#[sqlx::test(migrations = "../../migrations")]
async fn a_database_error_leaves_no_content_in_last_error(db: PgPool) {
    let state = state(db.clone()).await;
    let mail = "Subject: Q4 plan from maya@northwind-studio.example";
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                as_number: Some(mail.to_owned()),
                ..Probe::note("db")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    Worker::new(state).run_until_idle().await;
    let error = job(&db, id).await.last_error.unwrap();
    assert_eq!(error, "database error 22P02");
    assert!(!error.contains("maya"), "{error}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_handler_quoting_mail_in_its_error_leaves_no_content_in_last_error(db: PgPool) {
    let state = state(db.clone()).await;
    let reply = "{\"error\":\"invalid\",\"echo\":\"Subject: Q4 plan\"} from \
                 Maya <maya@northwind-studio.example>, see \
                 https://mail.example.com/v1/messages/Q4-plan?token=abc\n\
                 Hi Sam,\nthe Q4 plan is attached.";
    let id = created(
        enqueue_probe(
            &state,
            &Probe {
                echo: Some(reply.to_owned()),
                ..Probe::note("echo")
            },
            EnqueueOptions::default(),
        )
        .await,
    );
    Worker::new(state).run_until_idle().await;
    let error = job(&db, id).await.last_error.unwrap();
    assert_eq!(
        error,
        r#"provider answered 400: {"…":"…","…":"…"} from Maya <[address]>, see https://mail.example.com/… […]"#
    );
    for leaked in ["Q4", "maya@", "northwind", "Hi Sam", "attached", "token"] {
        assert!(!error.contains(leaked), "{leaked} in {error}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn prune_events_keeps_recent_and_undelivered_events(db: PgPool) {
    let state = state(db.clone()).await;
    // Three old events, one of them still waiting for its consumer.
    let mut ids = Vec::new();
    for (n, age) in [(1, 120), (2, 120), (3, 10)] {
        let id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO kernel.events (name, version, workspace_id, payload, occurred_at)
             VALUES ('test.happened', 1, 'ws', jsonb_build_object('n', $1::int),
                     now() - make_interval(days => $2))
             RETURNING id",
        )
        .bind(n)
        .bind(age)
        .fetch_one(&db)
        .await
        .unwrap();
        ids.push(id);
    }
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, event_id, max_attempts, run_at)
         VALUES ('events', 'test.later', '{}', $1, 1, now() + interval '1 day')",
    )
    .bind(ids[1])
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    enqueue(&mut tx, &PruneEvents, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state).run_until_idle().await;

    let left: Vec<i32> = sqlx::query_scalar(
        "SELECT (payload->>'n')::int FROM kernel.events WHERE name = 'test.happened' ORDER BY 1",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(left, [2, 3], "the 90-day-old delivered event is gone");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_purged_workspace_loses_its_finished_jobs_and_events(db: PgPool) {
    let state = state(db.clone()).await;
    for workspace in ["gone", "kept"] {
        enqueue_probe(
            &state,
            &Probe::note(workspace),
            EnqueueOptions::for_workspace(workspace),
        )
        .await;
        sqlx::query(
            "INSERT INTO kernel.events (name, version, workspace_id, payload)
             VALUES ('test.happened', 1, $1, '{}')",
        )
        .bind(workspace)
        .execute(&db)
        .await
        .unwrap();
    }
    Worker::new(state.clone()).run_until_idle().await;
    // A job of the purged workspace still to run (another module's purge).
    enqueue_probe(
        &state,
        &Probe::note("still-queued"),
        EnqueueOptions::for_workspace("gone").run_at(Utc::now() + TimeDelta::days(1)),
    )
    .await;

    let mut tx = crate::db::begin_in(&db, "gone").await.unwrap();
    let (jobs, events) = purge_workspace(&mut tx, "gone", 10).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!((jobs, events), (1, 1));
    let left: Vec<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, payload->>'note' FROM kernel.jobs
          WHERE kind = 'test.probe' ORDER BY 1, 2",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        left,
        [
            ("gone".to_owned(), "still-queued".to_owned()),
            ("kept".to_owned(), "kept".to_owned())
        ]
    );
    let events: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM kernel.events WHERE name = 'test.happened'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(events, ["kept"]);
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

/// Wait until every kernel schedule enqueued at start-up has succeeded, so a
/// worker's stats are not racing them.
async fn wait_for_kernel_schedules(db: &PgPool) {
    let schedules = i64::try_from(Registry::build(&[]).unwrap().schedule_names().len()).unwrap();
    for _ in 0..200 {
        let done = count(
            db,
            "SELECT count(*) FROM kernel.jobs WHERE kind LIKE 'kernel.%' AND state = 'succeeded'",
        )
        .await;
        if done >= schedules {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the kernel's start-up schedules never all succeeded");
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
    wait_for_kernel_schedules(&db).await;
    stop.send(()).unwrap();
    // The probe, and the kernel's prunes, blob sweep, purged-workspace
    // sweep and rekey dispatch scheduled at start-up.
    assert_eq!(worker.await.unwrap().succeeded, 9);
}

#[sqlx::test(migrations = "../../migrations")]
async fn shutdown_finishes_quick_jobs_and_releases_slow_ones(db: PgPool) {
    let state = state_with(db.clone(), |jobs| {
        jobs.shutdown_grace_seconds = 1;
        jobs.poll_interval_ms = 50;
        // Room for both probes and every start-up schedule at once (all
        // instance-wide, so one fairness bucket).
        jobs.concurrency = 10;
        jobs.per_workspace = 10;
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
    wait_for_kernel_schedules(&db).await;

    stop.send(()).unwrap();
    let stats = worker.await.unwrap();
    // The quick probe and the kernel's prunes, blob sweep, purged-workspace
    // sweep and rekey dispatch scheduled at start-up.
    assert_eq!((stats.succeeded, stats.released), (9, 1));
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
            "kernel.dispatch_rekeys",
            "kernel.prune_changes",
            "kernel.prune_events",
            "kernel.prune_idempotency",
            "kernel.prune_jobs",
            "kernel.prune_rate_limits",
            "kernel.prune_tracked_events",
            "kernel.sequence_changes",
            "kernel.sweep_blobs",
            "kernel.sweep_purged_workspaces",
            "test.count_a",
            "test.count_b",
            "test.forget_fails",
            "test.probe"
        ]
    );
    assert_eq!(registry.settling_events(), ["test.departed"]);
    assert_eq!(
        registry.schedule_names(),
        [
            "kernel.prune_jobs",
            "kernel.prune_events",
            "kernel.prune_tracked_events",
            "kernel.sweep_blobs",
            "kernel.prune_idempotency",
            "kernel.prune_rate_limits",
            "kernel.prune_changes",
            "kernel.sweep_purged_workspaces",
            "kernel.dispatch_rekeys"
        ]
    );
    assert_eq!(
        registry.queues(),
        ["changes", "default", "events", "maintenance"]
    );
    let consumers: Vec<_> = registry
        .consumers_of("test.pinged")
        .iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(consumers, ["test.count_a", "test.count_b"]);
    assert_eq!(
        registry
            .consumer("test.count_b")
            .map(|c| (c.event, c.event_version)),
        Some(("test.pinged", 1))
    );
}

/// An instance-wide event with a nested payload type.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct Rotated {
    /// Which key.
    key: KeyInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct KeyInfo {
    kid: String,
}

impl DomainEvent for Rotated {
    const NAME: &'static str = "test.rotated";
    const VERSION: u16 = 3;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct Foreign;

impl DomainEvent for Foreign {
    const NAME: &'static str = "other.foreign";
    const VERSION: u16 = 0;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

#[test]
fn published_events_form_the_catalogue() {
    let registry = Registry::build(&[module(
        background().publishes::<Rotated>().publishes::<Pinged>(),
    )])
    .unwrap();
    let names: Vec<_> = registry.published().iter().map(|e| e.name).collect();
    assert_eq!(names, ["test.rotated", "test.pinged"]);

    let catalogue = serde_json::to_value(EventCatalogue::new(&registry)).unwrap();
    let events = catalogue["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    // Sorted by name.
    assert_eq!(events[0]["name"], "test.pinged");
    assert_eq!(events[0]["module"], "test");
    assert_eq!(events[0]["version"], 1);
    assert_eq!(events[0]["workspaceScoped"], true);
    assert_eq!(events[0]["payload"]["type"], "object");
    assert!(events[0]["payload"]["properties"]["n"].is_object());
    assert_eq!(events[1]["name"], "test.rotated");
    assert_eq!(events[1]["version"], 3);
    assert_eq!(events[1]["workspaceScoped"], false);
    assert_eq!(
        events[1]["payload"]["properties"]["key"]["$ref"],
        "#/components/schemas/KeyInfo"
    );
    assert_eq!(
        catalogue["components"]["schemas"]["KeyInfo"]["properties"]["kid"]["type"],
        "string"
    );
}

#[test]
fn published_events_are_owned_versioned_and_unique() {
    let err = Registry::build(&[module(
        Background::new()
            .publishes::<Pinged>()
            .publishes::<Pinged>()
            .publishes::<Foreign>(),
    )])
    .unwrap_err();
    let all = err.0.join("\n");
    assert!(all.contains("`test.pinged` is declared twice"), "{all}");
    assert!(
        all.contains("published event `other.foreign`: must be `test.<name>`"),
        "{all}"
    );
    assert!(
        all.contains("`other.foreign`: VERSION must be at least 1"),
        "{all}"
    );
}

/// The kinds that run instance-wide only (campbooks-cloud#211): every
/// schedule's and the deliveries of an instance-wide event, not a job or a
/// workspace event's consumer.
#[test]
fn registry_marks_the_instance_only_kinds() {
    let registry = Registry::build(&[module(background())]).unwrap();
    assert_eq!(
        registry.instance_only_kinds(),
        [
            "kernel.dispatch_rekeys",
            "kernel.prune_changes",
            "kernel.prune_events",
            "kernel.prune_idempotency",
            "kernel.prune_jobs",
            "kernel.prune_rate_limits",
            "kernel.prune_tracked_events",
            "kernel.sweep_blobs",
            "kernel.sweep_purged_workspaces",
            "test.forget_fails",
        ]
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

/// Another consumer watched under [`FORGETTING`]'s code, with another line.
struct ForgetLoudly;

impl EventHandler for ForgetLoudly {
    type Event = Departed;
    const NAME: &'static str = "test.forget_loudly";
    const QUEUE: &'static str = "events";
    const WATCH: Option<Watch> = Some(Watch {
        code: "test_forgetting",
        died: "another line",
    });

    async fn handle(_ctx: &mut JobContext, _event: Event<Departed>) -> Result<(), JobError> {
        Ok(())
    }
}

/// A watch whose code is no code.
struct ForgetShouting;

impl EventHandler for ForgetShouting {
    type Event = Departed;
    const NAME: &'static str = "test.forget_shouting";
    const QUEUE: &'static str = "events";
    const WATCH: Option<Watch> = Some(Watch {
        code: "Not A Code",
        died: "",
    });

    async fn handle(_ctx: &mut JobContext, _event: Event<Departed>) -> Result<(), JobError> {
        Ok(())
    }
}

#[test]
fn registry_lists_watched_kinds_and_rejects_watches_that_disagree() {
    let registry = Registry::build(&[module(background())]).unwrap();
    assert_eq!(registry.watch("test.forget_fails"), Some(FORGETTING));
    assert_eq!(registry.watch("test.count_a"), None);
    assert_eq!(
        registry.watched().into_iter().collect::<Vec<_>>(),
        [("test_forgetting", vec!["test.forget_fails"])]
    );

    let err = Registry::build(&[module(
        background()
            .on_event::<ForgetLoudly>()
            .on_event::<ForgetShouting>(),
    )])
    .unwrap_err();
    let all = err.0.join("\n");
    assert!(
        all.contains("`test.forget_loudly`: watch `test_forgetting` logs another line than `test.forget_fails`'s"),
        "{all}"
    );
    assert!(all.contains("watch code `Not A Code`"), "{all}");
    assert!(
        all.contains("watch `Not A Code` needs the line logged when one dies"),
        "{all}"
    );
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
    assert!(registry.schedule_names().contains(&"web.sweep"));
    assert!(registry.schedule_names().contains(&"kernel.prune_jobs"));

    let err = Registry::build_with(&[], &[("other", &web)]).unwrap_err();
    assert!(err.0.join("\n").contains("must be `other.<name>`"), "{err}");
}

#[test]
fn errors_are_truncated() {
    let long = "x".repeat(MAX_ERROR_LEN + 10);
    assert_eq!(truncate_error(&long).chars().count(), MAX_ERROR_LEN + 1);
    assert_eq!(truncate_error("short"), "short");
}

// --- workspace isolation of the kernel's tables (campbooks-cloud#117) ---------

/// An event of workspace `workspace`, with one change for the feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Touched {
    workspace: String,
}

impl DomainEvent for Touched {
    const NAME: &'static str = "test.touched";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<crate::changes::Change> {
        vec![crate::changes::Change::upsert(crate::RecordRef::new(
            "test.thing",
            "t-1",
        ))]
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_workspace_transaction_sees_and_writes_only_its_kernel_rows(db: PgPool) {
    use crate::{db::BeginFor as _, tenancy::WorkspaceId};

    let pool = crate::testing::app_role_pool(&db).await;
    let state = crate::testing::state(pool.clone());
    let (ana, bea) = (WorkspaceId::new(), WorkspaceId::new());
    for workspace in [ana, bea] {
        let mut tx = pool.begin_for(workspace).await.unwrap();
        state
            .events
            .publish(
                &mut tx,
                &Touched {
                    workspace: workspace.to_string(),
                },
            )
            .await
            .unwrap();
        crate::jobs::enqueue(
            &mut tx,
            &Probe::default(),
            EnqueueOptions::for_workspace(workspace.to_string()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // In Ana's transaction, the event log, the queue and the feed show
    // Ana's rows only.
    let mut tx = pool.begin_for(ana).await.unwrap();
    for (table, sql) in [
        ("events", "SELECT DISTINCT workspace_id FROM kernel.events"),
        ("jobs", "SELECT DISTINCT workspace_id FROM kernel.jobs"),
        (
            "changes",
            "SELECT DISTINCT workspace_id FROM kernel.changes",
        ),
    ] {
        let workspaces: Vec<Option<String>> =
            sqlx::query_scalar(sql).fetch_all(&mut *tx).await.unwrap();
        assert!(
            workspaces
                .iter()
                .all(|w| w.as_deref().is_none_or(|w| w == ana.to_string())),
            "{table}: {workspaces:?}"
        );
        assert!(!workspaces.is_empty(), "{table}");
    }
    // Publishing Bea's event from Ana's transaction is refused.
    let refused = state
        .events
        .publish(
            &mut tx,
            &Touched {
                workspace: bea.to_string(),
            },
        )
        .await;
    assert!(
        matches!(refused, Err(sqlx::Error::InvalidArgument(_))),
        "{refused:?}"
    );
    tx.rollback().await.unwrap();
    // And row-level security refuses a forged row even without the check.
    let mut tx = pool.begin_for(ana).await.unwrap();
    let forged = sqlx::query(
        "INSERT INTO kernel.changes (workspace_id, event_id, record_type, record_id, op)
         VALUES ($1, kernel.uuid_v7(), 'test.thing', 't-2', 'upsert')",
    )
    .bind(bea.to_string())
    .execute(&mut *tx)
    .await;
    assert!(forged.is_err(), "{forged:?}");
    tx.rollback().await.unwrap();

    // A transaction that set nothing sees neither; the kernel's own
    // machinery (the kernel scope) sees both.
    let none: i64 = sqlx::query_scalar("SELECT count(DISTINCT workspace_id) FROM kernel.changes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(none, 0);
    let mut tx = crate::db::begin_kernel(&pool).await.unwrap();
    let both: i64 = sqlx::query_scalar("SELECT count(DISTINCT workspace_id) FROM kernel.changes")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(both, 2);
    tx.rollback().await.unwrap();
}

// --- administration ----------------------------------------------------------

mod administration {
    use super::*;
    use crate::{
        admin::{AdminGrant, PermissionSet},
        jobs::admin::{self, Discard, FailedFilter, Retry},
    };

    fn grant() -> AdminGrant {
        AdminGrant::new("ops".into(), PermissionSet::from_names(["super"]))
    }

    fn doomed(note: &str) -> Probe {
        Probe {
            fatal: true,
            ..Probe::note(note)
        }
    }

    /// Enqueue `probe`, let a worker run it to its death, return its id.
    async fn die(state: &AppState, probe: &Probe, options: EnqueueOptions) -> JobId {
        let id = created(enqueue_probe(state, probe, options).await);
        Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(job(&state.db, id).await.state, "failed");
        id
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_summary_counts_the_dead_jobs_of_watched_kinds(db: PgPool) {
        let state = state(db).await;
        let watched = |summary: &admin::QueueSummary| {
            summary
                .watched
                .iter()
                .map(|w| (w.code.clone(), w.kinds.clone(), w.failed))
                .collect::<Vec<_>>()
        };
        let summary = admin::summary(&state.db, state.events.registry(), &grant())
            .await
            .unwrap();
        // Listed before anything died, so a panel can say all is well.
        assert_eq!(
            watched(&summary),
            [(
                "test_forgetting".to_owned(),
                vec!["test.forget_fails".to_owned()],
                0
            )]
        );
        // A probe that dies is not watched: not counted there.
        die(&state, &doomed("unwatched"), EnqueueOptions::default()).await;
        let mut tx = state.db.begin().await.unwrap();
        state
            .events
            .publish(&mut tx, &Departed { who: "sam".into() })
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 1, "{stats:?}");
        let summary = admin::summary(&state.db, state.events.registry(), &grant())
            .await
            .unwrap();
        assert_eq!(watched(&summary)[0].2, 1);
    }

    /// Make a dead job runnable: the cause was fixed.
    async fn fix(db: &PgPool, id: JobId) {
        sqlx::query("UPDATE kernel.jobs SET payload = $2 WHERE id = $1")
            .bind(id)
            .bind(serde_json::to_value(Probe::note("fixed")).unwrap())
            .execute(db)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn summary_counts_by_queue_and_kind_without_payloads(db: PgPool) {
        let state = state(db.clone()).await;
        die(&state, &doomed("secret-note"), EnqueueOptions::default()).await;
        enqueue_probe(&state, &Probe::note("ready"), EnqueueOptions::default()).await;
        enqueue_probe(
            &state,
            &Probe::note("later"),
            EnqueueOptions {
                run_at: Some(Utc::now() + TimeDelta::hours(1)),
                ..EnqueueOptions::default()
            },
        )
        .await;
        enqueue_probe(&state, &Probe::note("done"), EnqueueOptions::default()).await;
        sqlx::query(
            "UPDATE kernel.jobs SET state = 'succeeded', finished_at = now()
              WHERE payload->>'note' = 'done'",
        )
        .execute(&db)
        .await
        .unwrap();
        // A running job past its lease.
        let lost =
            created(enqueue_probe(&state, &Probe::note("lost"), EnqueueOptions::default()).await);
        sqlx::query(
            "UPDATE kernel.jobs SET state = 'running', locked_by = 'w',
                    locked_until = now() - interval '1 minute', started_at = now()
              WHERE id = $1",
        )
        .bind(lost)
        .execute(&db)
        .await
        .unwrap();

        let summary = admin::summary(&db, state.events.registry(), &grant())
            .await
            .unwrap();
        let [probe] = summary.kinds.as_slice() else {
            panic!("one kind expected: {summary:?}");
        };
        assert_eq!(
            (probe.queue.as_str(), probe.kind.as_str()),
            ("default", "test.probe")
        );
        assert_eq!(
            (
                probe.ready,
                probe.scheduled,
                probe.running,
                probe.expired_leases,
                probe.failed
            ),
            (1, 1, 1, 1, 1)
        );
        assert!(probe.oldest_ready_seconds.is_some());
        let text = serde_json::to_string(&summary).unwrap();
        assert!(!text.contains("secret-note"), "{text}");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn failed_lists_newest_first_in_pages_without_payloads(db: PgPool) {
        let state = state(db.clone()).await;
        let mut ids = Vec::new();
        for n in 0..5 {
            ids.push(
                die(
                    &state,
                    &doomed(&format!("note-{n}")),
                    EnqueueOptions::for_workspace("ws-1"),
                )
                .await,
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        enqueue_probe(&state, &Probe::note("fine"), EnqueueOptions::default()).await;

        let none = FailedFilter::default();
        let first = admin::failed(&db, &grant(), &none, None, Some(2))
            .await
            .unwrap();
        assert_eq!(
            first.items.iter().map(|j| j.id).collect::<Vec<_>>(),
            [ids[4], ids[3]]
        );
        assert_eq!(first.items[0].workspace_id.as_deref(), Some("ws-1"));
        assert_eq!(
            first.items[0].last_error.as_deref(),
            Some("probe told to fail")
        );
        let second = admin::failed(&db, &grant(), &none, first.next.as_deref(), Some(2))
            .await
            .unwrap();
        assert_eq!(
            second.items.iter().map(|j| j.id).collect::<Vec<_>>(),
            [ids[2], ids[1]]
        );
        let last = admin::failed(&db, &grant(), &none, second.next.as_deref(), Some(2))
            .await
            .unwrap();
        assert_eq!(
            last.items.iter().map(|j| j.id).collect::<Vec<_>>(),
            [ids[0]]
        );
        assert!(last.next.is_none());

        let other_kind = FailedFilter {
            kind: Some("nope".into()),
            ..FailedFilter::default()
        };
        assert!(
            admin::failed(&db, &grant(), &other_kind, None, None)
                .await
                .unwrap()
                .items
                .is_empty()
        );
        let text = serde_json::to_string(&first).unwrap();
        assert!(
            !text.contains("note-") && !text.contains("payload"),
            "{text}"
        );

        let bad = admin::failed(&db, &grant(), &none, Some("garbage"), None).await;
        assert!(matches!(bad, Err(crate::Error::BadRequest(_))));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn retry_touches_only_dead_jobs_and_is_idempotent(db: PgPool) {
        let state = state(db.clone()).await;
        let dead = die(&state, &doomed("x"), EnqueueOptions::default()).await;
        fix(&db, dead).await;
        let pending =
            created(enqueue_probe(&state, &Probe::note("p"), EnqueueOptions::default()).await);
        let running =
            created(enqueue_probe(&state, &Probe::note("r"), EnqueueOptions::default()).await);
        sqlx::query(
            "UPDATE kernel.jobs SET state = 'running', locked_by = 'w',
                    locked_until = now() + interval '1 minute', attempts = 1
              WHERE id = $1",
        )
        .bind(running)
        .execute(&db)
        .await
        .unwrap();
        let done =
            created(enqueue_probe(&state, &Probe::note("d"), EnqueueOptions::default()).await);
        sqlx::query(
            "UPDATE kernel.jobs SET state = 'succeeded', finished_at = now() WHERE id = $1",
        )
        .bind(done)
        .execute(&db)
        .await
        .unwrap();

        for id in [pending, running, done] {
            let before = job(&db, id).await;
            assert_eq!(
                admin::retry(&db, &grant(), id).await.unwrap(),
                Retry::Unchanged
            );
            let after = job(&db, id).await;
            assert_eq!(
                (before.state, before.attempts),
                (after.state, after.attempts)
            );
        }
        assert_eq!(
            admin::retry(&db, &grant(), JobId::new()).await.unwrap(),
            Retry::Missing
        );

        assert_eq!(
            admin::retry(&db, &grant(), dead).await.unwrap(),
            Retry::Requeued
        );
        let row = job(&db, dead).await;
        assert_eq!((row.state.as_str(), row.attempts), ("pending", 0));
        assert!(row.finished_at.is_none() && row.locked_by.is_none());
        // Twice is the same as once.
        assert_eq!(
            admin::retry(&db, &grant(), dead).await.unwrap(),
            Retry::Unchanged
        );

        // The worker runs it again, and the other jobs were not disturbed.
        sqlx::query("DELETE FROM kernel.jobs WHERE id = ANY($1)")
            .bind([pending, running, done].map(uuid::Uuid::from))
            .execute(&db)
            .await
            .unwrap();
        Worker::new(state).run_until_idle().await;
        let row = job(&db, dead).await;
        assert_eq!((row.state.as_str(), row.attempts), ("succeeded", 1));
        assert_eq!(notes(&db, Probe::NAME).await, ["fixed"]);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn discard_deletes_only_dead_jobs(db: PgPool) {
        let state = state(db.clone()).await;
        let dead = die(&state, &doomed("x"), EnqueueOptions::default()).await;
        let pending =
            created(enqueue_probe(&state, &Probe::note("p"), EnqueueOptions::default()).await);
        assert_eq!(
            admin::discard(&db, &grant(), pending).await.unwrap(),
            Discard::Unchanged
        );
        assert_eq!(job(&db, pending).await.state, "pending");
        assert_eq!(
            admin::discard(&db, &grant(), dead).await.unwrap(),
            Discard::Discarded
        );
        assert_eq!(
            admin::discard(&db, &grant(), dead).await.unwrap(),
            Discard::Missing
        );
        assert_eq!(count(&db, "SELECT count(*) FROM kernel.jobs").await, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn retry_kind_is_bounded_and_skips_a_held_unique_key(db: PgPool) {
        let state = state(db.clone()).await;
        let a = die(&state, &doomed("a"), EnqueueOptions::default().unique("k")).await;
        let b = die(&state, &doomed("b"), EnqueueOptions::default().unique("k")).await;
        let c = die(&state, &doomed("c"), EnqueueOptions::default()).await;
        let other = die(&state, &doomed("o"), EnqueueOptions::default()).await;
        sqlx::query("UPDATE kernel.jobs SET kind = 'test.other' WHERE id = $1")
            .bind(other)
            .execute(&db)
            .await
            .unwrap();

        // `a` and `b` share a key: only one of them comes back.
        let n = admin::retry_kind(&db, &grant(), "test.probe", None)
            .await
            .unwrap();
        assert_eq!(n, 2);
        let states = [a, b, c, other].map(|id| id);
        let mut pending = 0;
        for id in states {
            if job(&db, id).await.state == "pending" {
                pending += 1;
            }
        }
        assert_eq!(pending, 2, "one of the pair, and c");
        assert_eq!(job(&db, other).await.state, "failed");
        // The other of the pair stays dead while its key is held; nothing more to do.
        assert_eq!(
            admin::retry_kind(&db, &grant(), "test.probe", None)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            admin::retry_kind(&db, &grant(), "test.probe", Some("nope"))
                .await
                .unwrap(),
            0
        );
    }

    /// A dead job's unique key is held only by an unfinished job of its own
    /// workspace (campbooks-cloud#209).
    #[sqlx::test(migrations = "../../migrations")]
    async fn a_unique_key_is_held_only_in_its_own_workspace(db: PgPool) {
        let state = state(db.clone()).await;
        let keyed = |workspace: &str| EnqueueOptions::for_workspace(workspace).unique("k");
        let ana = die(&state, &doomed("a"), keyed("ana")).await;
        let bob = die(&state, &doomed("b"), keyed("bob")).await;
        let cy = die(&state, &doomed("c"), keyed("cy")).await;
        let dee = die(&state, &doomed("d"), keyed("dee")).await;
        // Ana's key is held again, by a pending job of hers.
        created(enqueue_probe(&state, &Probe::note("held"), keyed("ana")).await);

        assert_eq!(
            admin::retry(&db, &grant(), ana).await.unwrap(),
            Retry::Superseded
        );
        assert_eq!(
            admin::retry(&db, &grant(), bob).await.unwrap(),
            Retry::Requeued,
            "ana's job does not hold bob's key"
        );
        // Dead jobs of two workspaces sharing the key both come back.
        assert_eq!(
            admin::retry_kind(&db, &grant(), "test.probe", None)
                .await
                .unwrap(),
            2
        );
        for id in [cy, dee] {
            assert_eq!(job(&db, id).await.state, "pending");
        }
        assert_eq!(job(&db, ana).await.state, "failed");
    }

    /// Retrying while workers claim: every job runs exactly once per retry.
    #[sqlx::test(migrations = "../../migrations")]
    async fn retries_racing_workers_never_double_run(db: PgPool) {
        let state = state_with(db.clone(), |jobs| jobs.per_workspace = 10_000).await;
        let mut ids = Vec::new();
        for n in 0..40 {
            ids.push(die(&state, &doomed(&format!("j{n}")), EnqueueOptions::default()).await);
        }
        sqlx::query("DELETE FROM job_log")
            .execute(&db)
            .await
            .unwrap();
        for id in &ids {
            fix(&db, *id).await;
        }

        let workers = (0..3)
            .map(|_| Worker::new(state.clone()))
            .collect::<Vec<_>>();
        let g = grant();
        let retrying = async {
            for _ in 0..4 {
                let (a, b) = tokio::join!(
                    admin::retry_kind(&db, &g, "test.probe", None),
                    admin::retry_kind(&db, &g, "test.probe", None)
                );
                a.unwrap();
                b.unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        let running = async {
            for _ in 0..4 {
                tokio::join!(
                    workers[0].run_until_idle(),
                    workers[1].run_until_idle(),
                    workers[2].run_until_idle()
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::join!(retrying, running);
        for worker in &workers {
            worker.run_until_idle().await;
        }

        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE state = 'succeeded'"
            )
            .await,
            40
        );
        assert_eq!(
            count(&db, "SELECT count(*) FROM job_log").await,
            40,
            "a job ran twice"
        );
        assert_eq!(
            count(&db, "SELECT max(attempts)::bigint FROM kernel.jobs").await,
            1
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_failed_delivery_of_an_event_that_must_settle_is_never_pruned(db: PgPool) {
    let state = state(db.clone()).await;
    let mut tx = state.db.begin().await.unwrap();
    state
        .events
        .publish(&mut tx, &Departed { who: "sam".into() })
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let worker = Worker::new(state.clone());
    worker.run_until_idle().await;
    // Long past both retentions.
    sqlx::query(
        "UPDATE kernel.jobs SET finished_at = now() - interval '400 days'
          WHERE kind = 'test.forget_fails'",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("UPDATE kernel.events SET occurred_at = now() - interval '400 days'")
        .execute(&db)
        .await
        .unwrap();
    let mut tx = db.begin().await.unwrap();
    enqueue(&mut tx, &PruneJobs, EnqueueOptions::default())
        .await
        .unwrap();
    enqueue(&mut tx, &PruneEvents, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    worker.run_until_idle().await;

    let mut conn = db.acquire().await.unwrap();
    let unsettled = crate::EventBus::unsettled(
        &mut conn,
        "test.departed",
        &serde_json::json!({ "who": "sam" }),
    )
    .await
    .unwrap();
    assert!(unsettled, "still waiting for an operator");
}

// --- the kernel scope (campbooks-cloud#129) -----------------------------------

/// The kernel's machinery and its refusals as the runtime role, where
/// row-level security applies as in production: a transaction that sets
/// nothing sees nothing, and cross-workspace work runs in the kernel scope.
mod kernel_scope {
    use super::*;
    use crate::{
        blobs::{BlobError, BlobOwner, BlobStore},
        changes::Change,
        config::BlobsConfig,
        db::{self, BeginFor as _},
        records::RecordRef,
        tenancy::WorkspaceId,
    };

    /// The test module's state over the runtime role, with blobs in memory.
    async fn runtime(db: &PgPool) -> AppState {
        runtime_with(db, |_| {}).await
    }

    /// [`runtime`] with `tweak` applied to the jobs config.
    async fn runtime_with(
        db: &PgPool,
        tweak: impl FnOnce(&mut crate::config::JobsConfig),
    ) -> AppState {
        let _ = state(db.clone()).await;
        sqlx::query("GRANT SELECT, INSERT ON job_log TO campbooks_app")
            .execute(db)
            .await
            .unwrap();
        let pool = testing::app_role_pool(db).await;
        let registry = Registry::build(&[module(background().publishes::<Pinged>())]).unwrap();
        testing::allow_instance_rows(db, &registry).await;
        let mut config = testing::config("postgres://test@localhost/test");
        tweak(&mut config.jobs);
        AppState::new(pool.clone(), config)
            .with_registry(registry)
            .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
    }

    fn refused<T: std::fmt::Debug>(result: &Result<T, sqlx::Error>) -> bool {
        matches!(result, Err(sqlx::Error::InvalidArgument(_)))
    }

    fn blob_refused<T: std::fmt::Debug>(result: &Result<T, BlobError>) -> bool {
        matches!(
            result,
            Err(BlobError::Database(sqlx::Error::InvalidArgument(_)))
        )
    }

    /// `sql`'s count, read as the database owner (no row-level security).
    async fn owner_count(db: &PgPool, sql: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_one(db)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn kernel_writes_refuse_a_transaction_that_cannot_reach_their_workspace(db: PgPool) {
        let state = runtime(&db).await;
        let pool = state.db.clone();
        let (ana, bea) = (WorkspaceId::new(), WorkspaceId::new());
        let ws = ana.to_string();
        let owner = BlobOwner::new("test.thing", uuid::Uuid::now_v7());
        let blob = state
            .blobs
            .put(&ws, b"bytes".to_vec(), "text/plain")
            .await
            .unwrap();

        // Neither a workspace nor the kernel scope; then another workspace.
        for (what, mut tx) in [
            ("unset", pool.begin().await.unwrap()),
            ("another workspace", pool.begin_for(bea).await.unwrap()),
        ] {
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let enqueued = enqueue(
                &mut step,
                &Probe::default(),
                EnqueueOptions::for_workspace(ws.clone()),
            )
            .await;
            assert!(refused(&enqueued), "{what}: {enqueued:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let recorded = crate::changes::record(
                &mut step,
                EventId::new(),
                &ws,
                &[Change::upsert(RecordRef::new("test.thing", "t-1"))],
            )
            .await;
            assert!(refused(&recorded), "{what}: {recorded:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let published = state
                .events
                .publish(
                    &mut step,
                    &Touched {
                        workspace: ws.clone(),
                    },
                )
                .await;
            assert!(refused(&published), "{what}: {published:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let attached = state.blobs.attach(&mut step, &ws, &blob, &owner).await;
            assert!(blob_refused(&attached), "{what}: {attached:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let detached = state.blobs.detach(&mut step, &ws, &blob.hash, &owner).await;
            assert!(blob_refused(&detached), "{what}: {detached:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let detached = state.blobs.detach_owner(&mut step, &ws, &owner).await;
            assert!(blob_refused(&detached), "{what}: {detached:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let purged = purge_workspace(&mut step, &ws, 10).await;
            assert!(refused(&purged), "{what}: {purged:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let purged = crate::changes::purge_workspace(&mut step, &ws, 10).await;
            assert!(refused(&purged), "{what}: {purged:?}");
            drop(step);
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let tombstoned = crate::purge::tombstone(&mut step, &ws).await;
            assert!(refused(&tombstoned), "{what}: {tombstoned:?}");
            drop(step);
            tx.commit().await.unwrap();
        }
        assert_eq!(
            owner_count(
                &db,
                "SELECT (SELECT count(*) FROM kernel.jobs) + (SELECT count(*) FROM kernel.events)
                      + (SELECT count(*) FROM kernel.changes) + (SELECT count(*) FROM kernel.blob_refs)
                      + (SELECT count(*) FROM kernel.purged_workspaces)"
            )
            .await,
            0,
            "nothing was written"
        );

        // In the workspace, or the kernel scope, the same calls work.
        let mut tx = pool.begin_for(ana).await.unwrap();
        enqueue(
            &mut tx,
            &Probe::default(),
            EnqueueOptions::for_workspace(ws.clone()),
        )
        .await
        .unwrap();
        assert!(
            state
                .blobs
                .attach(&mut tx, &ws, &blob, &owner)
                .await
                .unwrap()
        );
        state
            .events
            .publish(
                &mut tx,
                &Touched {
                    workspace: ws.clone(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let mut tx = db::begin_kernel(&pool).await.unwrap();
        enqueue(
            &mut tx,
            &Probe::default(),
            EnqueueOptions::for_workspace(ws.clone()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE kind = 'test.probe'"
            )
            .await,
            2
        );
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM kernel.blob_refs").await,
            1
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_unset_transaction_still_publishes_and_enqueues_instance_wide_work(db: PgPool) {
        let state = runtime(&db).await;
        let mut tx = state.db.begin().await.unwrap();
        state
            .events
            .publish(
                &mut tx,
                &Pinged {
                    workspace: None,
                    n: 7,
                },
            )
            .await
            .unwrap();
        enqueue(&mut tx, &Probe::note("instance"), EnqueueOptions::default())
            .await
            .unwrap();
        // Lifted for those inserts only: the transaction is unset again and
        // sees none of what it just wrote.
        assert!(db::scope(&mut tx).await.unwrap().is_unset());
        let seen: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM kernel.events) + (SELECT count(*) FROM kernel.jobs)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(seen, 0);
        tx.commit().await.unwrap();

        // The worker delivers the event to both consumers and runs the job,
        // all instance-wide, and completes each in its own transaction.
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(
            (stats.succeeded, stats.failed, stats.lost),
            (3, 0, 0),
            "{stats:?}"
        );
        let logged: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT kind, workspace FROM job_log ORDER BY kind")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            logged,
            [
                ("test.count_a".to_owned(), None),
                ("test.count_b".to_owned(), None),
                ("test.probe".to_owned(), None),
            ]
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_worker_serves_every_workspace_as_the_runtime_role(db: PgPool) {
        use crate::{
            admin::{AdminGrant, PermissionSet},
            jobs::admin::{self, Discard, FailedFilter, Retry},
        };

        let state = runtime(&db).await;
        let (ana, bea) = (WorkspaceId::new(), WorkspaceId::new());
        for workspace in [ana, bea] {
            let ws = workspace.to_string();
            let mut tx = state.db.begin_for(workspace).await.unwrap();
            state
                .events
                .publish(
                    &mut tx,
                    &Pinged {
                        workspace: Some(ws.clone()),
                        n: 1,
                    },
                )
                .await
                .unwrap();
            enqueue(
                &mut tx,
                &Probe::note(&ws),
                EnqueueOptions::for_workspace(ws.clone()),
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
        // Claimed fairly across both, each run in its own workspace.
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(
            (stats.succeeded, stats.failed, stats.lost),
            (6, 0, 0),
            "{stats:?}"
        );
        for workspace in [ana, bea] {
            let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM job_log WHERE workspace = $1")
                .bind(workspace.to_string())
                .fetch_one(&db)
                .await
                .unwrap();
            assert_eq!(rows, 3, "{workspace}");
        }

        // A job that fails for good, then the admin's queue (kernel scope).
        let mut tx = state.db.begin_for(ana).await.unwrap();
        enqueue(
            &mut tx,
            &Probe {
                fatal: true,
                ..Probe::note("dead")
            },
            EnqueueOptions::for_workspace(ana.to_string()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 1, "{stats:?}");
        let grant = AdminGrant::new("ops".into(), PermissionSet::from_names(["super"]));
        let summary = admin::summary(&state.db, state.events.registry(), &grant)
            .await
            .unwrap();
        assert_eq!(summary.kinds.iter().map(|k| k.failed).sum::<i64>(), 1);
        let page = admin::failed(&state.db, &grant, &FailedFilter::default(), None, None)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let dead = page.items[0].id;
        assert_eq!(
            admin::retry(&state.db, &grant, dead).await.unwrap(),
            Retry::Requeued
        );
        Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(
            admin::discard(&state.db, &grant, dead).await.unwrap(),
            Discard::Discarded
        );
        assert_eq!(
            admin::retry_kind(&state.db, &grant, "test.probe", None)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn maintenance_works_in_the_kernel_scope(db: PgPool) {
        let state = runtime(&db).await;
        let (ana, bea) = (WorkspaceId::new(), WorkspaceId::new());
        for workspace in [ana, bea] {
            let ws = workspace.to_string();
            let mut tx = state.db.begin_for(workspace).await.unwrap();
            state
                .events
                .publish(
                    &mut tx,
                    &Pinged {
                        workspace: Some(ws.clone()),
                        n: 1,
                    },
                )
                .await
                .unwrap();
            state
                .events
                .publish(
                    &mut tx,
                    &Touched {
                        workspace: ws.clone(),
                    },
                )
                .await
                .unwrap();
            tx.commit().await.unwrap();
            state
                .blobs
                .put(&ws, ws.clone().into_bytes(), "text/plain")
                .await
                .unwrap();
        }
        // Deliveries, and the sequencer numbering each workspace's feed.
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.changes WHERE seq IS NOT NULL"
            )
            .await,
            2
        );

        // Replay reaches both workspaces' events.
        let replayed = state
            .events
            .replay(&state.db, "test.count_a", ReplayFrom::Beginning)
            .await
            .unwrap();
        assert_eq!(replayed, 2);
        Worker::new(state.clone()).run_until_idle().await;

        // Schedules tick (kernel scope) and their jobs run, the sweep among
        // them; a scheduled instance-wide job may work in the kernel scope.
        let worker = Worker::new(state.clone());
        worker.upkeep().await;
        assert!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.sweep_blobs'"
            )
            .await
                >= 1
        );
        let mut tx = state.db.begin().await.unwrap();
        enqueue(
            &mut tx,
            &Probe {
                kernel: true,
                ..Probe::note("instance")
            },
            EnqueueOptions::default(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        // A workspace's job may not.
        let mut tx = state.db.begin_for(ana).await.unwrap();
        enqueue(
            &mut tx,
            &Probe {
                kernel: true,
                ..Probe::note("workspace")
            },
            EnqueueOptions::for_workspace(ana.to_string()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let stats = worker.run_until_idle().await;
        assert_eq!(stats.failed, 1, "only the workspace's kernel_tx: {stats:?}");
        let kernel: Vec<String> =
            sqlx::query_scalar("SELECT note FROM job_log WHERE note LIKE 'kernel=%' ORDER BY note")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(kernel, ["kernel=true"]);

        // Retention: everything old goes, every workspace's.
        sqlx::query(
            "UPDATE kernel.jobs SET finished_at = now() - interval '400 days'
              WHERE state IN ('succeeded', 'failed')",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("UPDATE kernel.events SET occurred_at = now() - interval '400 days'")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE kernel.changes SET occurred_at = now() - interval '400 days',
                    sequenced_at = CASE WHEN seq IS NOT NULL THEN now() - interval '400 days' END",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("UPDATE kernel.blobs SET touched_at = now() - interval '400 days'")
            .execute(&db)
            .await
            .unwrap();
        let mut tx = state.db.begin().await.unwrap();
        enqueue(&mut tx, &PruneJobs, EnqueueOptions::default())
            .await
            .unwrap();
        enqueue(&mut tx, &PruneEvents, EnqueueOptions::default())
            .await
            .unwrap();
        enqueue(
            &mut tx,
            &crate::changes::PruneChanges,
            EnqueueOptions::default(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE finished_at < now() - interval '1 day'"
            )
            .await,
            0
        );
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM kernel.events").await,
            0
        );
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM kernel.changes").await,
            0
        );
        let swept = state
            .blobs
            .sweep_older_than(Duration::from_mins(1))
            .await
            .unwrap();
        assert_eq!(swept.blobs, 2);
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM kernel.blobs").await,
            0
        );
    }

    // --- instance-wide rows change only in the kernel scope (#196) ----------

    /// `sql`'s affected rows in `tx`.
    async fn affected(tx: &mut sqlx::PgConnection, sql: &str) -> u64 {
        sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(tx)
            .await
            .unwrap()
            .rows_affected()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_transaction_adds_instance_rows_but_never_reads_or_changes_them(
        db: PgPool,
    ) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        // Instance-wide rows (an account's event and its two deliveries, a
        // scheduled job), and one job of the workspace.
        let mut tx = state.db.begin().await.unwrap();
        state
            .events
            .publish(
                &mut tx,
                &Pinged {
                    workspace: None,
                    n: 1,
                },
            )
            .await
            .unwrap();
        enqueue(
            &mut tx,
            &Probe::note("instance"),
            EnqueueOptions::default().unique("tick"),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let mut tx = state.db.begin_for(ana).await.unwrap();
        enqueue(
            &mut tx,
            &Probe::note("own"),
            EnqueueOptions::for_workspace(ana.to_string()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let before = owner_count(
            &db,
            "SELECT count(*) FROM kernel.jobs WHERE workspace_id IS NULL AND state = 'pending'",
        )
        .await;
        assert_eq!(before, 3);

        let mut tx = state.db.begin_for(ana).await.unwrap();
        // It does not see them (campbooks-cloud#211): an account's events
        // name accounts.
        let seen: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM kernel.jobs WHERE workspace_id IS NULL),
                    (SELECT count(*) FROM kernel.events WHERE workspace_id IS NULL)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(seen, (0, 0));
        // It cannot change or delete them: row-level security hides them
        // from UPDATE and DELETE (and from a row lock).
        for sql in [
            "UPDATE kernel.jobs SET state = 'succeeded' WHERE workspace_id IS NULL",
            "UPDATE kernel.jobs SET run_at = now() + interval '1 year' WHERE workspace_id IS NULL",
            "DELETE FROM kernel.jobs WHERE workspace_id IS NULL",
            "UPDATE kernel.events SET payload = '{}' WHERE workspace_id IS NULL",
            "DELETE FROM kernel.events WHERE workspace_id IS NULL",
        ] {
            assert_eq!(affected(&mut tx, sql).await, 0, "{sql}");
        }
        let locked: Vec<uuid::Uuid> =
            sqlx::query_scalar("SELECT id FROM kernel.jobs WHERE workspace_id IS NULL FOR UPDATE")
                .fetch_all(&mut *tx)
                .await
                .unwrap();
        assert!(locked.is_empty(), "{locked:?}");
        // Nor turn its own job into an instance-wide one.
        let mut step = db::savepoint(&mut tx).await.unwrap();
        let moved =
            sqlx::query("UPDATE kernel.jobs SET workspace_id = NULL WHERE kind = 'test.probe'")
                .execute(&mut *step)
                .await;
        assert!(moved.is_err(), "{moved:?}");
        drop(step);
        // Its own rows it still changes.
        assert_eq!(
            affected(
                &mut tx,
                "UPDATE kernel.jobs SET priority = 1 WHERE workspace_id IS NOT NULL"
            )
            .await,
            1
        );
        // It no longer enqueues instance-wide jobs (campbooks-cloud#207), a
        // duplicate of one included; it still publishes a listed
        // instance-wide event with its deliveries, reading nothing back
        // (campbooks-cloud#211).
        for options in [
            EnqueueOptions::default(),
            EnqueueOptions::default().unique("tick"),
        ] {
            let mut step = db::savepoint(&mut tx).await.unwrap();
            let added = enqueue(&mut step, &Probe::note("follow-up"), options).await;
            assert!(refused(&added), "{added:?}");
            drop(step);
        }
        state
            .events
            .publish(
                &mut tx,
                &Pinged {
                    workspace: None,
                    n: 2,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE workspace_id IS NULL AND state = 'pending'
                    AND run_at < now() + interval '1 day'"
            )
            .await,
            before + 2,
            "the old rows untouched, the two new deliveries added"
        );
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.events WHERE workspace_id IS NULL
                    AND payload <> '{}'::jsonb"
            )
            .await,
            2
        );

        // The kernel scope still changes them (the worker, pruning).
        let mut tx = db::begin_kernel(&state.db).await.unwrap();
        assert_eq!(
            affected(
                &mut tx,
                "UPDATE kernel.jobs SET priority = 2 WHERE workspace_id IS NULL"
            )
            .await,
            u64::try_from(before).unwrap() + 2
        );
        tx.rollback().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(
            (stats.succeeded, stats.failed, stats.lost),
            (6, 0, 0),
            "{stats:?}"
        );
    }

    /// A workspace transaction reads none of the instance-wide rows, an
    /// account's events (which name accounts) and their deliveries
    /// included (campbooks-cloud#211); what it publishes of them still gets
    /// its id, and the kernel scope still reads them all.
    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_transaction_reads_no_account_events(db: PgPool) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        let account = uuid::Uuid::now_v7().to_string();
        for name in [
            "identity.signed_out_everywhere",
            "identity.account_suspended",
        ] {
            sqlx::query(
                "WITH e AS (
                     INSERT INTO kernel.events (name, version, payload)
                     VALUES ($1, 1, jsonb_build_object('account', $2::text)) RETURNING id)
                 INSERT INTO kernel.jobs (queue, kind, payload, event_id, unique_key, max_attempts)
                 SELECT 'events', 'bff.end_signed_out_sessions', '{}', id, id::text, 3 FROM e",
            )
            .bind(name)
            .bind(&account)
            .execute(&db)
            .await
            .unwrap();
        }
        let mut tx = state.db.begin_for(ana).await.unwrap();
        for sql in [
            "SELECT count(*) FROM kernel.events
              WHERE name IN ('identity.signed_out_everywhere', 'identity.account_suspended')",
            "SELECT count(*) FROM kernel.events WHERE payload ? 'account'",
            "SELECT count(*) FROM kernel.jobs WHERE kind = 'bff.end_signed_out_sessions'",
        ] {
            let seen: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            assert_eq!(seen, 0, "{sql}");
        }
        // Its own instance-wide event (declared `PUBLISHED_IN_WORKSPACES`)
        // it still publishes, with its deliveries, and gets its id.
        let id = state
            .events
            .publish(
                &mut tx,
                &Pinged {
                    workspace: None,
                    n: 7,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            owner_count(
                &db,
                &format!(
                    "SELECT count(*) FROM kernel.jobs WHERE event_id = '{id}'
                        AND workspace_id IS NULL AND unique_key = '{id}'"
                )
            )
            .await,
            2
        );
        let mut tx = db::begin_kernel(&state.db).await.unwrap();
        let seen: i64 =
            sqlx::query_scalar("SELECT count(*) FROM kernel.events WHERE workspace_id IS NULL")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(seen, 3);
        tx.rollback().await.unwrap();
    }

    /// A job of an instance-only kind written by hand in a workspace (its
    /// own row, no schedule key, which the policies admit) is refused, not
    /// run; the schedule's own tick still runs (campbooks-cloud#211).
    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_s_row_of_an_instance_only_kind_is_refused_not_run(db: PgPool) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        // A succeeded job long past its retention: a prune deletes it.
        let mut tx = state.db.begin().await.unwrap();
        let old = created(
            enqueue(&mut tx, &Probe::note("old"), EnqueueOptions::default())
                .await
                .unwrap(),
        );
        tx.commit().await.unwrap();
        let worker = Worker::new(state.clone());
        worker.run_until_idle().await;
        sqlx::query(
            "UPDATE kernel.jobs SET finished_at = now() - interval '30 days' WHERE id = $1",
        )
        .bind(old)
        .execute(&db)
        .await
        .unwrap();

        let mut tx = state.db.begin_for(ana).await.unwrap();
        let forged: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, max_attempts)
             VALUES ('maintenance', 'kernel.prune_jobs', 'null', $1, 3)
             RETURNING id",
        )
        .bind(ana.to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!((stats.failed, stats.succeeded), (1, 0), "{stats:?}");
        let row = job(&db, JobId::from_uuid(forged)).await;
        assert_eq!((row.state.as_str(), row.attempts), ("failed", 1));
        assert!(
            row.last_error
                .as_deref()
                .unwrap_or_default()
                .contains("instance-wide only"),
            "{:?}",
            row.last_error
        );
        assert_eq!(job(&db, old).await.state, "succeeded", "nothing was pruned");

        // The schedule's tick, instance-wide, prunes.
        worker.upkeep().await;
        worker.run_until_idle().await;
        assert_eq!(
            owner_count(
                &db,
                &format!("SELECT count(*) FROM kernel.jobs WHERE id = '{old}'")
            )
            .await,
            0
        );
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.prune_jobs'
                    AND workspace_id IS NULL AND state = 'succeeded'"
            )
            .await,
            1
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn instance_wide_jobs_still_complete_retry_and_get_reaped(db: PgPool) {
        let state = runtime_with(&db, |jobs| jobs.retry_base_ms = 60_000).await;
        // An instance-wide job (from a transaction that set nothing: a
        // workspace's may not add one, campbooks-cloud#207), failing its
        // first attempt.
        let mut tx = state.db.begin().await.unwrap();
        let flaky = created(
            enqueue(
                &mut tx,
                &Probe {
                    fail_first: 1,
                    ..Probe::note("flaky")
                },
                EnqueueOptions::default(),
            )
            .await
            .unwrap(),
        );
        tx.commit().await.unwrap();
        let worker = Worker::new(state.clone());
        let stats = worker.run_until_idle().await;
        assert_eq!((stats.retried, stats.succeeded), (1, 0), "{stats:?}");
        let row = job(&db, flaky).await;
        assert_eq!((row.state.as_str(), row.attempts), ("pending", 1));
        sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE id = $1")
            .bind(flaky)
            .execute(&db)
            .await
            .unwrap();
        let stats = worker.run_until_idle().await;
        assert_eq!(stats.succeeded, 1, "{stats:?}");
        let row = job(&db, flaky).await;
        assert_eq!((row.state.as_str(), row.attempts), ("succeeded", 2));

        // An instance-wide job whose worker died is reaped and run again.
        let mut tx = state.db.begin().await.unwrap();
        let orphan = created(
            enqueue(&mut tx, &Probe::note("orphan"), EnqueueOptions::default())
                .await
                .unwrap(),
        );
        tx.commit().await.unwrap();
        let crashed = Worker::new(state.clone());
        assert_eq!(
            crashed.claim_for_test("default", 1).await.unwrap(),
            [(orphan, None, 1)]
        );
        sqlx::query("UPDATE kernel.jobs SET locked_until = now() - interval '1 second'")
            .execute(&db)
            .await
            .unwrap();
        let survivor = Worker::new(state.clone());
        survivor.upkeep().await;
        let row = job(&db, orphan).await;
        assert_eq!(row.state, "pending");
        assert!(row.last_error.unwrap().contains("lease expired"));
        survivor.run_until_idle().await;
        let row = job(&db, orphan).await;
        assert_eq!((row.state.as_str(), row.attempts), ("succeeded", 2));

        // One that fails for good is recorded as failed.
        let mut tx = state.db.begin().await.unwrap();
        let dead = created(
            enqueue(
                &mut tx,
                &Probe {
                    fatal: true,
                    ..Probe::note("dead")
                },
                EnqueueOptions::default(),
            )
            .await
            .unwrap(),
        );
        tx.commit().await.unwrap();
        survivor.run_until_idle().await;
        assert_eq!(job(&db, dead).await.state, "failed");
        let logged: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT note, workspace FROM job_log ORDER BY at")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            logged,
            [("flaky".to_owned(), None), ("orphan".to_owned(), None),],
            "each ran with no workspace; failed attempts left no row"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_maintenance_scope_reaches_no_kernel_table(db: PgPool) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        let ws = ana.to_string();
        // A workspace's event (with a change and two deliveries), a job and a
        // blob reference, and an instance-wide event.
        let mut tx = state.db.begin_for(ana).await.unwrap();
        state
            .events
            .publish(
                &mut tx,
                &Touched {
                    workspace: ws.clone(),
                },
            )
            .await
            .unwrap();
        enqueue(
            &mut tx,
            &Probe::note("own"),
            EnqueueOptions::for_workspace(ws.clone()),
        )
        .await
        .unwrap();
        let blob = state
            .blobs
            .put(&ws, b"bytes".to_vec(), "text/plain")
            .await
            .unwrap();
        let owner = BlobOwner::new("test.thing", uuid::Uuid::now_v7());
        state
            .blobs
            .attach(&mut tx, &ws, &blob, &owner)
            .await
            .unwrap();
        state
            .events
            .publish(
                &mut tx,
                &Pinged {
                    workspace: None,
                    n: 1,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let mut tx = db::begin_maintenance(&state.db).await.unwrap();
        let seen: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM kernel.events) + (SELECT count(*) FROM kernel.jobs)
                  + (SELECT count(*) FROM kernel.changes) + (SELECT count(*) FROM kernel.change_heads)
                  + (SELECT count(*) FROM kernel.blobs) + (SELECT count(*) FROM kernel.blob_refs)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(
            seen, 0,
            "the kernel's tables do not admit the maintenance scope"
        );
        assert!(
            owner_count(&db, "SELECT count(*) FROM kernel.jobs").await > 0,
            "there were rows to see"
        );
        // A workspace's job is refused until the dispatcher works in its
        // workspace; then it is enqueued there.
        let mut step = db::savepoint(&mut tx).await.unwrap();
        let refused_here = enqueue(
            &mut step,
            &Probe::note("dispatched"),
            EnqueueOptions::for_workspace(ws.clone()),
        )
        .await;
        assert!(refused(&refused_here), "{refused_here:?}");
        drop(step);
        db::set_workspace(&mut tx, &ws).await.unwrap();
        let enqueued = enqueue(
            &mut tx,
            &Probe::note("dispatched"),
            EnqueueOptions::for_workspace(ws.clone()),
        )
        .await
        .unwrap();
        assert!(matches!(enqueued, Enqueued::Created(_)), "{enqueued:?}");
        tx.commit().await.unwrap();

        // A workspace's job may not open it.
        let info = super::super::context::JobInfo {
            id: JobId::new(),
            kind: Probe::NAME.to_owned(),
            queue: Probe::QUEUE.to_owned(),
            attempt: 1,
            max_attempts: 1,
            workspace: Some(ws.clone()),
            event_id: None,
        };
        let mut ctx = JobContext::new(state.clone(), info);
        assert!(matches!(
            ctx.maintenance_tx().await,
            Err(JobError::Fatal(_))
        ));
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
    }

    // --- workspaces cannot forge instance-wide work (#207) ------------------

    /// Run `sql` in a savepoint of `tx`: whether row-level security refused
    /// it (`42501`, "new row violates row-level security policy").
    async fn policy_refuses(
        tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
        sql: &str,
    ) -> bool {
        let mut step = db::savepoint(tx).await.unwrap();
        let result = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(&mut *step)
            .await;
        drop(step);
        match result {
            Ok(_) => false,
            Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("42501") => true,
            Err(e) => panic!("{sql}: {e}"),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_cannot_forge_instance_wide_jobs_or_events(db: PgPool) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        let delivery = |columns: &str, values: &str| {
            format!(
                "INSERT INTO kernel.jobs (queue, kind, payload, event_id, unique_key, max_attempts{columns})
                 SELECT 'events', 'test.count_a', '{{}}', id, id::text, 3{values}
                   FROM (SELECT kernel.uuid_v7() AS id) e"
            )
        };
        let mut tx = state.db.begin_for(ana).await.unwrap();
        let forgeries = [
            // A schedule's key, never due: every later tick would be its
            // duplicate.
            "INSERT INTO kernel.jobs (queue, kind, payload, unique_key, run_at, max_attempts)
             VALUES ('maintenance', 'kernel.prune_jobs', 'null', 'schedule:kernel.prune_jobs',
                     'infinity', 1)"
                .to_owned(),
            // The same as one of the workspace's own rows (the unique index
            // spans workspaces).
            format!(
                "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, unique_key, max_attempts)
                 VALUES ('maintenance', 'kernel.prune_jobs', 'null', '{ana}',
                         'schedule:kernel.prune_jobs', 1)"
            ),
            // A listed consumer's delivery, but due never, already claimed,
            // ordered behind another key, or with a key not its event's.
            delivery(", run_at", ", 'infinity'"),
            delivery(
                ", state, locked_by, locked_until",
                ", 'running', 'w', 'infinity'",
            ),
            delivery(", ordering_key", ", 'send:1'"),
            "INSERT INTO kernel.jobs (queue, kind, payload, event_id, unique_key, max_attempts)
             VALUES ('events', 'test.count_a', '{}', kernel.uuid_v7(),
                     'schedule:test.count_a', 3)"
                .to_owned(),
            // A kind no listed event is delivered to.
            "INSERT INTO kernel.jobs (queue, kind, payload, event_id, unique_key, max_attempts)
             SELECT 'maintenance', 'kernel.prune_jobs', '{}', id, id::text, 1
               FROM (SELECT kernel.uuid_v7() AS id) e"
                .to_owned(),
            // An account's event a workspace does not announce.
            "INSERT INTO kernel.events (name, version, payload)
             VALUES ('identity.account_suspended', 1, '{}')"
                .to_owned(),
            "INSERT INTO kernel.events (name, version, payload, occurred_at)
             VALUES ('test.pinged', 1, '{}', 'infinity')"
                .to_owned(),
        ];
        for sql in &forgeries {
            assert!(policy_refuses(&mut tx, sql).await, "not refused: {sql}");
        }
        // What a workspace legitimately adds still passes: a listed event,
        // and a fresh delivery of it to a listed consumer.
        for sql in [
            "INSERT INTO kernel.events (name, version, payload) VALUES ('test.pinged', 1, '{}')"
                .to_owned(),
            delivery("", ""),
        ] {
            assert!(!policy_refuses(&mut tx, &sql).await, "refused: {sql}");
        }
        // Through the kernel: an instance-wide job, or an instance-wide event
        // not declared `PUBLISHED_IN_WORKSPACES`, is refused before the
        // database is asked.
        let mut step = db::savepoint(&mut tx).await.unwrap();
        let job = enqueue(
            &mut step,
            &Probe::note("forged"),
            EnqueueOptions::default().unique("schedule:test.probe"),
        )
        .await;
        assert!(refused(&job), "{job:?}");
        drop(step);
        let mut step = db::savepoint(&mut tx).await.unwrap();
        let event = state
            .events
            .publish(&mut step, &Departed { who: "x".into() })
            .await;
        assert!(refused(&event), "{event:?}");
        drop(step);
        tx.rollback().await.unwrap();
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM kernel.jobs").await,
            0,
            "nothing was added"
        );
    }

    /// A workspace's own row holding another workspace's job key (raw SQL,
    /// which the policies admit: its own workspace, no schedule key) no
    /// longer stops that workspace's job: unique and ordering keys are per
    /// workspace (campbooks-cloud#209).
    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_cannot_hold_another_workspace_s_job_key(db: PgPool) {
        let state = runtime(&db).await;
        let (ana, bob) = (WorkspaceId::new(), WorkspaceId::new());
        let key = format!("connectors.sync_stream:{}", WorkspaceId::new());
        let mut tx = state.db.begin_for(bob).await.unwrap();
        sqlx::query(
            "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, unique_key,
                                      ordering_key, run_at, max_attempts)
             VALUES ('default', 'test.probe', '{}', $1, $2, $2, 'infinity', 3)",
        )
        .bind(bob.to_string())
        .bind(&key)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = state.db.begin_for(ana).await.unwrap();
        let enqueued = enqueue(
            &mut tx,
            &Probe::note("ana"),
            EnqueueOptions::for_workspace(ana.to_string())
                .unique(key.clone())
                .ordered_by(key.clone()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert!(matches!(enqueued, Enqueued::Created(_)), "{enqueued:?}");
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.succeeded, 1, "{stats:?}");
        assert_eq!(notes(&db, Probe::NAME).await, ["ana"]);
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE state = 'pending' AND run_at = 'infinity'"
            )
            .await,
            1,
            "bob's row is still there, and never ran"
        );
    }

    /// A schedule row: id, kind, state, workspace, last error.
    type ScheduleRow = (uuid::Uuid, String, String, Option<String>, Option<String>);

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_stuck_schedule_row_is_replaced_by_the_next_tick(db: PgPool) {
        let state = runtime(&db).await;
        let ana = WorkspaceId::new();
        let row = |kind: &str, workspace: Option<String>, run_at: &str, leased: Option<&str>| {
            let workspace = workspace.map_or_else(|| "NULL".to_owned(), |w| format!("'{w}'"));
            let (state, by, until) = match leased {
                Some(until) => ("running", "'w-1'".to_owned(), until.to_owned()),
                None => ("pending", "NULL".to_owned(), "NULL".to_owned()),
            };
            format!(
                "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, unique_key, state,
                                          run_at, locked_by, locked_until, max_attempts)
                 VALUES ('maintenance', '{kind}', 'null', {workspace}, 'schedule:{kind}',
                         '{state}', {run_at}, {by}, {until}, 3)
                 RETURNING id"
            )
        };
        let insert = |sql: String| {
            let db = db.clone();
            async move {
                sqlx::query_scalar::<_, uuid::Uuid>(sqlx::AssertSqlSafe(sql))
                    .fetch_one(&db)
                    .await
                    .unwrap()
            }
        };
        // Rows written before the policies refused them (as the owner here):
        // one never due, one of a workspace, one leased forever. And two a
        // schedule may legitimately have: waiting out a retry, and held by a
        // live worker.
        let never = insert(row("kernel.prune_jobs", None, "'infinity'", None)).await;
        let theirs = insert(row(
            "kernel.prune_events",
            Some(ana.to_string()),
            "now()",
            None,
        ))
        .await;
        let forever = insert(row(
            "kernel.prune_changes",
            None,
            "now()",
            Some("'infinity'"),
        ))
        .await;
        let retrying = insert(row(
            "kernel.prune_idempotency",
            None,
            "now() + interval '10 minutes'",
            None,
        ))
        .await;
        let live = insert(row(
            "kernel.prune_rate_limits",
            None,
            "now()",
            Some("now() + interval '30 seconds'"),
        ))
        .await;

        Worker::new(state.clone()).upkeep().await;

        let rows: Vec<ScheduleRow> = sqlx::query_as(
            "SELECT id, kind, state, workspace_id, last_error FROM kernel.jobs
                  WHERE unique_key LIKE 'schedule:%' ORDER BY kind, created_at",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        let of = |kind: &str| -> Vec<_> { rows.iter().filter(|r| r.1 == kind).collect() };
        for (kind, stuck) in [
            ("kernel.prune_jobs", never),
            ("kernel.prune_events", theirs),
            ("kernel.prune_changes", forever),
        ] {
            let rows = of(kind);
            assert_eq!(rows.len(), 2, "{kind}: {rows:?}");
            let old = rows.iter().find(|r| r.0 == stuck).unwrap();
            assert_eq!(old.2, "failed", "{kind}");
            assert!(
                old.4.as_deref().unwrap().contains("stuck schedule row"),
                "{kind}: {old:?}"
            );
            let fresh = rows.iter().find(|r| r.0 != stuck).unwrap();
            assert_eq!(
                (fresh.2.as_str(), fresh.3.as_ref()),
                ("pending", None),
                "{kind}"
            );
        }
        for (kind, kept) in [
            ("kernel.prune_idempotency", retrying),
            ("kernel.prune_rate_limits", live),
        ] {
            let rows = of(kind);
            assert_eq!(rows.len(), 1, "{kind}: {rows:?}");
            assert_eq!(rows[0].0, kept);
            assert_ne!(rows[0].2, "failed");
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn enqueue_in_works_in_the_workspace_for_its_enqueue_only(db: PgPool) {
        let state = runtime(&db).await;
        let (ana, bob) = (
            WorkspaceId::new().to_string(),
            WorkspaceId::new().to_string(),
        );
        let mut tx = db::begin_maintenance(&state.db).await.unwrap();
        for ws in [&ana, &bob] {
            let enqueued =
                super::super::enqueue_in(&mut tx, ws, &Probe::note(ws), EnqueueOptions::default())
                    .await
                    .unwrap();
            assert!(matches!(enqueued, Enqueued::Created(_)), "{enqueued:?}");
            assert_eq!(db::scope(&mut tx).await.unwrap().workspace, None, "cleared");
        }
        // Not from a transaction that works in a workspace already.
        let mut step = db::savepoint(&mut tx).await.unwrap();
        db::set_workspace(&mut step, &bob).await.unwrap();
        let refused_here = super::super::enqueue_in(
            &mut step,
            &ana,
            &Probe::note("x"),
            EnqueueOptions::default(),
        )
        .await;
        assert!(
            refused(&refused_here),
            "{refused_here:?}: it works in one already"
        );
        drop(step);
        tx.commit().await.unwrap();
        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!(stats.succeeded, 2, "{stats:?}");
        let mut logged: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT note, workspace FROM job_log")
                .fetch_all(&db)
                .await
                .unwrap();
        logged.sort();
        let mut expected = vec![
            (ana.clone(), Some(ana.clone())),
            (bob.clone(), Some(bob.clone())),
        ];
        expected.sort();
        assert_eq!(logged, expected);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_workspace_cannot_reach_the_schedules_and_a_tick_far_ahead_heals(db: PgPool) {
        let state = runtime(&db).await;
        let worker = Worker::new(state.clone());
        worker.upkeep().await;
        assert!(owner_count(&db, "SELECT count(*) FROM kernel.schedules").await > 0);

        // A workspace transaction, and one that set nothing, neither see nor
        // change the schedules' last ticks (campbooks-cloud#207).
        let mut tx = state.db.begin_for(WorkspaceId::new()).await.unwrap();
        let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.schedules")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(seen, 0);
        assert_eq!(
            affected(
                &mut tx,
                "UPDATE kernel.schedules SET last_tick = 9223372036854775807"
            )
            .await,
            0
        );
        assert!(
            policy_refuses(
                &mut tx,
                "INSERT INTO kernel.schedules (name, last_tick)
                 VALUES ('kernel.forged', 9223372036854775807)"
            )
            .await
        );
        tx.rollback().await.unwrap();
        let mut tx = state.db.begin().await.unwrap();
        assert_eq!(
            affected(
                &mut tx,
                "UPDATE kernel.schedules SET last_tick = 9223372036854775807"
            )
            .await,
            0
        );
        tx.rollback().await.unwrap();

        // A last tick far ahead (written as the owner, as before the policy)
        // counts as this tick: the schedule runs again.
        sqlx::query(
            "UPDATE kernel.schedules SET last_tick = 9223372036854775807
              WHERE name = 'kernel.prune_jobs'",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("DELETE FROM kernel.jobs")
            .execute(&db)
            .await
            .unwrap();
        worker.upkeep().await;
        assert_eq!(
            owner_count(
                &db,
                "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.prune_jobs'
                    AND unique_key = 'schedule:kernel.prune_jobs' AND state = 'pending'"
            )
            .await,
            1
        );
        assert!(
            owner_count(
                &db,
                "SELECT last_tick FROM kernel.schedules WHERE name = 'kernel.prune_jobs'"
            )
            .await
                < i64::MAX
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_event_logged_outside_its_own_workspace_is_never_handled(db: PgPool) {
        let state = runtime(&db).await;
        let (ana, bob) = (
            WorkspaceId::new().to_string(),
            WorkspaceId::new().to_string(),
        );
        // Written by hand in ana's transaction, as the runtime role: an
        // instance-wide event (its payload names no workspace) and an event
        // naming bob, each logged in ana with a delivery there.
        let mut tx = db::begin_in(&state.db, &ana).await.unwrap();
        let mut forged = Vec::new();
        for (payload, consumer) in [
            (
                serde_json::json!({ "workspace": null, "n": 1 }),
                "test.count_a",
            ),
            (
                serde_json::json!({ "workspace": bob, "n": 2 }),
                "test.count_b",
            ),
        ] {
            let id: uuid::Uuid = sqlx::query_scalar(
                "WITH e AS (
                     INSERT INTO kernel.events (name, version, workspace_id, payload)
                     VALUES ('test.pinged', 1, $1, $2) RETURNING id)
                 INSERT INTO kernel.jobs
                     (queue, kind, payload, workspace_id, event_id, unique_key, max_attempts)
                 SELECT 'events', $3, '{}', $1, id, id::text, 3 FROM e
                 RETURNING id",
            )
            .bind(&ana)
            .bind(payload)
            .bind(consumer)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            forged.push(id);
        }
        tx.commit().await.unwrap();

        let stats = Worker::new(state.clone()).run_until_idle().await;
        assert_eq!((stats.failed, stats.succeeded), (2, 0), "{stats:?}");
        for id in forged {
            let (state_now, error): (String, Option<String>) =
                sqlx::query_as("SELECT state, last_error FROM kernel.jobs WHERE id = $1")
                    .bind(id)
                    .fetch_one(&db)
                    .await
                    .unwrap();
            assert_eq!(state_now, "failed");
            assert!(
                error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("another workspace"),
                "{error:?}"
            );
        }
        assert_eq!(
            owner_count(&db, "SELECT count(*) FROM job_log").await,
            0,
            "no handler ran"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_jobs_transaction_allows_its_statements_five_minutes(db: PgPool) {
    // campbooks-cloud#254: requests' statements are cut at 30 s
    // (`db::STATEMENT_TIMEOUT`); a job's own transaction allows five
    // minutes, until it ends.
    let state = state(db.clone()).await;
    enqueue_probe(
        &state,
        &Probe {
            limits: true,
            ..Probe::note("limits")
        },
        EnqueueOptions::default(),
    )
    .await;
    Worker::new(state.clone()).run_until_idle().await;
    assert_eq!(notes(&db, "test.probe").await, ["statement_timeout=5min"]);
    // Only for that transaction: every connection of the pool, the one the
    // job ran on among them, is back to its own limit (campbooks-cloud#256).
    let size = state.db.options().get_max_connections();
    let mut connections = Vec::new();
    for _ in 0..size {
        connections.push(state.db.acquire().await.unwrap());
    }
    for connection in &mut connections {
        let after: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&mut **connection)
            .await
            .unwrap();
        assert_ne!(after, "5min");
    }
}

/// The queue's per-workspace key indexes left missing or invalid by a
/// migration run that does not drop invalid indexes first (`cargo sqlx
/// migrate run`) are built again by `20261022030100`, and a valid one is
/// left alone (campbooks-cloud#213, item 14).
#[sqlx::test(migrations = "../../migrations")]
async fn the_job_key_indexes_are_rebuilt_when_missing_or_invalid(db: PgPool) {
    const VALID: &str = "SELECT c.relname::text, i.indisvalid
                           FROM pg_index i
                           JOIN pg_class c ON c.oid = i.indexrelid
                          WHERE c.relname IN ('jobs_unique_key_per_workspace_idx',
                                              'jobs_ordering_per_workspace_idx')
                          ORDER BY 1";
    let migration = std::fs::read_to_string(format!(
        "{}/../../migrations/20261022030100_kernel__job_key_indexes_valid.sql",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let healthy = vec![
        ("jobs_ordering_per_workspace_idx".to_owned(), true),
        ("jobs_unique_key_per_workspace_idx".to_owned(), true),
    ];
    let indexes: Vec<(String, bool)> = sqlx::query_as(VALID).fetch_all(&db).await.unwrap();
    assert_eq!(indexes, healthy);
    let oid_before: i64 = sqlx::query_scalar(
        "SELECT 'kernel.jobs_ordering_per_workspace_idx'::regclass::oid::bigint",
    )
    .fetch_one(&db)
    .await
    .unwrap();

    // What a failed concurrent build leaves: an invalid index, or none.
    sqlx::query(
        "UPDATE pg_index SET indisvalid = false
          WHERE indexrelid = 'kernel.jobs_unique_key_per_workspace_idx'::regclass",
    )
    .execute(&db)
    .await
    .unwrap();
    let indexes: Vec<(String, bool)> = sqlx::query_as(VALID).fetch_all(&db).await.unwrap();
    assert_eq!(
        indexes[1],
        ("jobs_unique_key_per_workspace_idx".to_owned(), false)
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration.clone()))
        .execute(&db)
        .await
        .unwrap();
    let indexes: Vec<(String, bool)> = sqlx::query_as(VALID).fetch_all(&db).await.unwrap();
    assert_eq!(indexes, healthy);
    let oid_after: i64 = sqlx::query_scalar(
        "SELECT 'kernel.jobs_ordering_per_workspace_idx'::regclass::oid::bigint",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(oid_before, oid_after, "a valid index is left alone");

    sqlx::query("DROP INDEX kernel.jobs_ordering_per_workspace_idx")
        .execute(&db)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration))
        .execute(&db)
        .await
        .unwrap();
    let indexes: Vec<(String, bool)> = sqlx::query_as(VALID).fetch_all(&db).await.unwrap();
    assert_eq!(indexes, healthy);
}

/// A running worker beats once its loop has reached the database
/// (campbooks-cloud#185).
#[sqlx::test(migrations = "../../migrations")]
async fn a_running_worker_is_alive(db: PgPool) {
    let state = state_with(db, |_| {}).await;
    let worker = Worker::new(state);
    let liveness = worker.liveness();
    assert!(!liveness.alive(crate::jobs::ALIVE_FOR), "not started yet");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(worker.run(async {
        let _ = stopped.await;
    }));
    for _ in 0..100 {
        if liveness.alive(crate::jobs::ALIVE_FOR) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(liveness.alive(crate::jobs::ALIVE_FOR), "beats once running");
    stop.send(()).unwrap();
    running.await.unwrap();
}

/// An over-limit payload cannot fit by retrying: a job failing with it fails
/// now, like a bad request (campbooks-cloud#265).
#[test]
fn an_over_limit_payload_fails_a_job_for_good() {
    let too_large = crate::Error::from(crate::blobs::BlobError::TooLarge { limit: 10 });
    assert!(matches!(JobError::from(too_large), JobError::Fatal(_)));
    let busy = crate::Error::Unavailable(crate::i18n::MESSAGES.reason("database_unavailable"));
    assert!(matches!(JobError::from(busy), JobError::Retry { .. }));
}
