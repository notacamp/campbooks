//! Recording and sequencing changes against a real Postgres.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::*;
use crate::{
    AppState, DomainEvent,
    jobs::{Registry, Worker},
    testing,
};

/// A test event that touched some `test.thing` records.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Touched {
    workspace: String,
    things: Vec<String>,
}

impl DomainEvent for Touched {
    const NAME: &'static str = "test.touched";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        self.things
            .iter()
            .map(|id| Change::upsert(RecordRef::new("test.thing", id)))
            .collect()
    }
}

/// An event that declares no changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Quiet {
    workspace: String,
}

impl DomainEvent for Quiet {
    const NAME: &'static str = "test.quiet";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// State whose connections run as the runtime role, with the kernel's own
/// background work registered.
async fn state(db: &PgPool) -> AppState {
    testing::state(testing::app_role_pool(db).await)
        .with_registry(Registry::build(&[]).expect("the kernel's registrations are valid"))
}

async fn publish(state: &AppState, workspace: &str, things: &[&str]) {
    let mut tx = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut tx, workspace).await.unwrap();
    state
        .events
        .publish(
            &mut tx,
            &Touched {
                workspace: workspace.to_owned(),
                things: things.iter().map(ToString::to_string).collect(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// `(seq, record_id)` of a workspace's changes in insertion order.
async fn changes_of(db: &PgPool, workspace: &str) -> Vec<(Option<i64>, String)> {
    sqlx::query_as(
        "SELECT seq, record_id FROM kernel.changes WHERE workspace_id = $1 ORDER BY published",
    )
    .bind(workspace)
    .fetch_all(db)
    .await
    .unwrap()
}

async fn sequencing_jobs(db: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs
          WHERE kind = 'kernel.sequence_changes' AND workspace_id = $1",
    )
    .bind(workspace)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn head_of(db: &PgPool, workspace: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT head FROM kernel.change_heads WHERE workspace_id = $1")
        .bind(workspace)
        .fetch_optional(db)
        .await
        .unwrap()
}

/// Cursors are 1..=n with no gap, in insertion order.
fn assert_gapless(changes: &[(Option<i64>, String)]) {
    let seqs: Vec<i64> = changes
        .iter()
        .map(|(seq, id)| seq.unwrap_or_else(|| panic!("change {id} was never numbered")))
        .collect();
    let expected: Vec<i64> = (1..=i64::try_from(changes.len()).unwrap()).collect();
    assert_eq!(seqs, expected, "cursors are dense and follow publish order");
}

#[sqlx::test(migrations = "../../migrations")]
async fn publishing_records_changes_and_one_sequencer(db: PgPool) {
    let state = state(&db).await;
    publish(&state, "ws-a", &["1", "2"]).await;
    publish(&state, "ws-a", &["3"]).await;

    let changes = changes_of(&db, "ws-a").await;
    assert_eq!(
        changes,
        vec![(None, "1".into()), (None, "2".into()), (None, "3".into())]
    );
    assert_eq!(
        sequencing_jobs(&db, "ws-a").await,
        1,
        "coalesced per workspace"
    );

    Worker::new(state).run_until_idle().await;
    assert_gapless(&changes_of(&db, "ws-a").await);
    assert_eq!(head_of(&db, "ws-a").await, Some(3));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_rolled_back_publish_leaves_no_change(db: PgPool) {
    let state = state(&db).await;
    let mut tx = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut tx, "ws-a").await.unwrap();
    state
        .events
        .publish(
            &mut tx,
            &Touched {
                workspace: "ws-a".into(),
                things: vec!["1".into()],
            },
        )
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert!(changes_of(&db, "ws-a").await.is_empty());
    assert_eq!(sequencing_jobs(&db, "ws-a").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn events_without_changes_record_nothing(db: PgPool) {
    let state = state(&db).await;
    let mut tx = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut tx, "ws-a").await.unwrap();
    state
        .events
        .publish(
            &mut tx,
            &Quiet {
                workspace: "ws-a".into(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(changes_of(&db, "ws-a").await.is_empty());
    assert_eq!(sequencing_jobs(&db, "ws-a").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_burst_of_a_thousand_events_takes_one_sequencing_job(db: PgPool) {
    let state = state(&db).await;
    for i in 0..1_000 {
        publish(&state, "ws-a", &[&i.to_string()]).await;
    }
    publish(&state, "ws-b", &["other"]).await;
    assert_eq!(sequencing_jobs(&db, "ws-a").await, 1);

    Worker::new(state).run_until_idle().await;
    let changes = changes_of(&db, "ws-a").await;
    assert_eq!(changes.len(), 1_000);
    assert_gapless(&changes);
    let ids: Vec<String> = changes.into_iter().map(|(_, id)| id).collect();
    let published: Vec<String> = (0..1_000).map(|i: i32| i.to_string()).collect();
    assert_eq!(ids, published, "numbered in publish order");
    assert_eq!(head_of(&db, "ws-a").await, Some(1_000));
    assert_eq!(
        head_of(&db, "ws-b").await,
        Some(1),
        "workspaces count apart"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_burst_while_workers_run_stays_coalesced_and_gapless(db: PgPool) {
    let state = state(&db).await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(Worker::new(state.clone()).run(async {
        let _ = stopped.await;
    }));

    let producers: Vec<_> = (0..10)
        .map(|p| {
            let state = state.clone();
            tokio::spawn(async move {
                for i in 0..100 {
                    publish(&state, "ws-a", &[&format!("{p}-{i}")]).await;
                }
            })
        })
        .collect();
    for producer in producers {
        producer.await.unwrap();
    }
    let _ = stop.send(());
    worker.await.unwrap();
    sequence_all(&state, &db, "ws-a").await;

    let changes = changes_of(&db, "ws-a").await;
    assert_eq!(changes.len(), 1_000);
    let mut seqs: Vec<i64> = changes.iter().filter_map(|(seq, _)| *seq).collect();
    assert_eq!(seqs.len(), 1_000, "every change was numbered");
    seqs.sort_unstable();
    assert_eq!(
        seqs,
        (1..=1_000).collect::<Vec<_>>(),
        "no gap, no duplicate"
    );
    let jobs = sequencing_jobs(&db, "ws-a").await;
    assert!(
        jobs < 200,
        "1,000 events took {jobs} sequencing jobs; bursts should coalesce"
    );
}

/// Run the worker until every change of `workspace` is numbered (the
/// sequencer reschedules itself a little later while producers are busy,
/// which `run_until_idle` alone would leave for later).
async fn sequence_all(state: &AppState, db: &PgPool, workspace: &str) {
    let worker = Worker::new(state.clone());
    for _ in 0..100 {
        worker.run_until_idle().await;
        if changes_of(db, workspace)
            .await
            .iter()
            .all(|(seq, _)| seq.is_some())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("changes of {workspace} were never all numbered");
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_sequencer_waits_for_producers_in_flight(db: PgPool) {
    let state = state(&db).await;
    // A producer that started first and is still open…
    let mut slow = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut slow, "ws-a").await.unwrap();
    state
        .events
        .publish(
            &mut slow,
            &Touched {
                workspace: "ws-a".into(),
                things: vec!["slow".into()],
            },
        )
        .await
        .unwrap();
    // …and one that commits before it.
    publish(&state, "ws-a", &["fast"]).await;

    // The sequencer cannot number "fast" alone: a reader would then see
    // cursor 1 and later find "slow" committed below it. It leaves a
    // successor for later instead of waiting.
    Worker::new(state.clone()).run_until_idle().await;
    assert_eq!(
        changes_of(&db, "ws-a").await,
        vec![(None, "fast".into())],
        "nothing numbered while a producer is in flight"
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs
          WHERE kind = 'kernel.sequence_changes' AND state = 'pending'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pending, 1, "rescheduled, not failed");

    slow.commit().await.unwrap();
    sequence_all(&state, &db, "ws-a").await;
    assert_eq!(
        changes_of(&db, "ws-a").await,
        vec![(Some(1), "slow".into()), (Some(2), "fast".into())]
    );
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.sequence_changes'
            AND (state = 'failed' OR attempts > 1)",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(failed, 0, "a busy workspace costs no attempts");
}

#[sqlx::test(migrations = "../../migrations")]
async fn producers_never_wait_for_the_sequencer(db: PgPool) {
    let state = state(&db).await;
    // The worker gets a pool of its own, so producers measure the database
    // locks, not a wait for a pooled connection.
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(Worker::new(self::state(&db).await).run(async {
        let _ = stopped.await;
    }));

    // Open the pool's connections first: the test measures lock waits, not
    // connection setup.
    let warm = futures_util::future::join_all((0..5).map(|_| state.db.acquire())).await;
    drop(warm);

    // A long producer holds the workspace's shared lock throughout…
    let mut long = state.db.begin().await.unwrap();
    crate::db::set_workspace(&mut long, "ws-a").await.unwrap();
    state
        .events
        .publish(
            &mut long,
            &Touched {
                workspace: "ws-a".into(),
                things: vec!["long".into()],
            },
        )
        .await
        .unwrap();

    // …while a burst of others publishes and commits, with the worker
    // trying (and failing) to sequence all along.
    let mut slowest = Duration::ZERO;
    for i in 0..50 {
        let started = std::time::Instant::now();
        publish(&state, "ws-a", &[&format!("burst-{i}")]).await;
        slowest = slowest.max(started.elapsed());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        slowest < Duration::from_millis(500),
        "a producer took {slowest:?}: it waited for the sequencer"
    );
    assert!(
        changes_of(&db, "ws-a")
            .await
            .iter()
            .all(|(seq, _)| seq.is_none()),
        "nothing numbered while the long producer is open"
    );

    long.commit().await.unwrap();
    let _ = stop.send(());
    worker.await.unwrap();
    sequence_all(&state, &db, "ws-a").await;

    let changes = changes_of(&db, "ws-a").await;
    assert_eq!(changes.len(), 51);
    assert_gapless(&changes);
    assert_eq!(changes[0].1, "long", "numbered in publish order");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_publish_after_a_finished_sequencer_gets_a_new_one(db: PgPool) {
    let state = state(&db).await;
    publish(&state, "ws-a", &["1"]).await;
    Worker::new(state.clone()).run_until_idle().await;
    publish(&state, "ws-a", &["2"]).await;
    assert_eq!(sequencing_jobs(&db, "ws-a").await, 2);
    Worker::new(state).run_until_idle().await;
    assert_gapless(&changes_of(&db, "ws-a").await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn pruning_deletes_old_changes_and_rescues_stranded_ones(db: PgPool) {
    let state = state(&db).await;
    publish(&state, "ws-a", &["old", "new"]).await;
    Worker::new(state.clone()).run_until_idle().await;
    sqlx::query(
        "UPDATE kernel.changes SET occurred_at = now() - interval '73 hours'
          WHERE record_id = 'old'",
    )
    .execute(&db)
    .await
    .unwrap();
    // A change whose sequencer gave up: unnumbered, a while ago, no job.
    publish(&state, "ws-b", &["stranded"]).await;
    sqlx::query(
        "DELETE FROM kernel.jobs WHERE kind = 'kernel.sequence_changes' AND workspace_id = 'ws-b'",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("UPDATE kernel.changes SET occurred_at = now() - interval '5 minutes' WHERE workspace_id = 'ws-b'")
        .execute(&db)
        .await
        .unwrap();

    let mut tx = state.db.begin().await.unwrap();
    crate::jobs::enqueue(
        &mut tx,
        &PruneChanges,
        crate::jobs::EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state).run_until_idle().await;

    assert_eq!(changes_of(&db, "ws-a").await, vec![(Some(2), "new".into())]);
    assert_eq!(
        head_of(&db, "ws-a").await,
        Some(2),
        "the head survives pruning"
    );
    assert_eq!(
        changes_of(&db, "ws-b").await,
        vec![(Some(1), "stranded".into())]
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn purging_a_workspace_deletes_its_changes(db: PgPool) {
    let state = state(&db).await;
    publish(&state, "ws-a", &["1"]).await;
    publish(&state, "ws-b", &["1"]).await;
    Worker::new(state.clone()).run_until_idle().await;
    let mut tx = state.db.begin().await.unwrap();
    assert_eq!(purge_workspace(&mut tx, "ws-a").await.unwrap(), 1);
    tx.commit().await.unwrap();
    assert!(changes_of(&db, "ws-a").await.is_empty());
    assert_eq!(head_of(&db, "ws-a").await, None);
    assert_eq!(changes_of(&db, "ws-b").await.len(), 1);
}

#[tokio::test]
async fn the_hub_limits_streams_per_caller() {
    let config = crate::config::ChangesConfig {
        max_streams_per_principal: 2,
        ..crate::config::ChangesConfig::default()
    };
    let hub = ChangeHub::new(testing::unreachable_pool(), &config);
    let first = hub.admit("ana").unwrap();
    let _second = hub.admit("ana").unwrap();
    assert!(hub.admit("ana").is_none());
    assert!(hub.admit("bea").is_some(), "per caller");
    drop(first);
    assert!(hub.admit("ana").is_some(), "a closed stream frees its slot");
}

#[tokio::test]
async fn waking_a_workspace_reaches_only_its_streams() {
    let hub = ChangeHub::new(
        testing::unreachable_pool(),
        &crate::config::ChangesConfig::default(),
    );
    let mut a = hub.watch("ws-a");
    let mut b = hub.watch("ws-b");
    hub.wake("ws-a");
    assert!(a.has_changed().unwrap());
    assert!(!b.has_changed().unwrap());
    a.mark_unchanged();
    b.mark_unchanged();
}
