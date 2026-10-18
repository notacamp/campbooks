//! The batched purge of a workspace's kernel rows, its tombstone and the
//! hourly sweep of what is left (campbooks-cloud#65).

use sqlx::PgPool;

use super::*;
use crate::{
    AppState,
    jobs::{EnqueueOptions, Registry, Worker, enqueue},
    tenancy::{LiveFuture, Membership, MembershipFuture, WorkspaceId},
    testing,
};

/// The kernel's state over the runtime role (row-level security applies),
/// with the kernel's own work registered.
async fn runtime(db: &PgPool) -> AppState {
    let registry = Registry::build(&[]).unwrap();
    testing::allow_instance_rows(db, &registry).await;
    testing::state(testing::app_role_pool(db).await).with_registry(registry)
}

/// `sql`'s count, read as the database owner (no row-level security).
async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

/// `n` events of `workspace`, each delivered (a succeeded job reads it).
async fn delivered_events(db: &PgPool, workspace: &str, n: i32) {
    sqlx::query(
        "WITH e AS (
             INSERT INTO kernel.events (name, version, workspace_id, payload)
             SELECT 'test.happened', 1, $1, jsonb_build_object('n', i)
               FROM generate_series(1, $2) AS i
             RETURNING id)
         INSERT INTO kernel.jobs
             (queue, kind, payload, workspace_id, event_id, state, max_attempts,
              finished_at)
         SELECT 'events', 'test.consumer', '{}', $1, id, 'succeeded', 1, now() FROM e",
    )
    .bind(workspace)
    .bind(n)
    .execute(db)
    .await
    .unwrap();
}

/// One numbered change of `workspace` and its cursor head.
async fn change(db: &PgPool, workspace: &str) {
    sqlx::query(
        "INSERT INTO kernel.changes
             (workspace_id, seq, sequenced_at, event_id, record_type, record_id, op)
         VALUES ($1, 1, now(), kernel.uuid_v7(), 'test.thing', 't-1', 'upsert')",
    )
    .bind(workspace)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO kernel.change_heads (workspace_id, head) VALUES ($1, 1)")
        .bind(workspace)
        .execute(db)
        .await
        .unwrap();
}

/// An event of `workspace` whose delivery is still `state` (pending, or
/// running under a live lease), scheduled far ahead so no worker claims it.
async fn undelivered_event(db: &PgPool, workspace: &str, state: &str) {
    sqlx::query(
        "WITH e AS (
             INSERT INTO kernel.events (name, version, workspace_id, payload)
             VALUES ('test.waiting', 1, $1, jsonb_build_object('state', $2::text))
             RETURNING id)
         INSERT INTO kernel.jobs
             (queue, kind, payload, workspace_id, event_id, state, max_attempts, run_at,
              locked_by, locked_until)
         SELECT 'events', 'test.consumer', '{}', $1, id, $2, 1, now() + interval '1 day',
                CASE WHEN $2 = 'running' THEN 'elsewhere' END,
                CASE WHEN $2 = 'running' THEN now() + interval '1 day' END
           FROM e",
    )
    .bind(workspace)
    .bind(state)
    .execute(db)
    .await
    .unwrap();
}

async fn tombstoned(db: &PgPool, workspace: &str, days_ago: i32) {
    sqlx::query(
        "INSERT INTO kernel.purged_workspaces (workspace_id, purged_at)
         VALUES ($1, now() - make_interval(days => $2))",
    )
    .bind(workspace)
    .bind(days_ago)
    .execute(db)
    .await
    .unwrap();
}

async fn sweep(state: &AppState) {
    let mut tx = crate::db::begin_kernel(&state.db).await.unwrap();
    enqueue(&mut tx, &SweepPurgedWorkspaces, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state.clone()).run_until_idle().await;
}

/// 12,000 events and their deliveries take three transactions of
/// [`PURGE_BATCH`] rows per table; another workspace's rows stay.
#[sqlx::test(migrations = "../../migrations")]
async fn a_large_workspace_is_purged_in_batches(db: PgPool) {
    let state = runtime(&db).await;
    delivered_events(&db, "big", 12_000).await;
    change(&db, "big").await;
    delivered_events(&db, "other", 3).await;
    change(&db, "other").await;

    let purged = purge_workspace(&state.db, "big").await.unwrap();
    assert_eq!(
        purged,
        Purged {
            jobs: 12_000,
            events: 12_000,
            changes: 1,
            keys: 0,
            batches: 3,
        }
    );
    assert_eq!(PURGE_BATCH, 5_000, "5,000 + 5,000 + 2,000");
    let left = count(
        &db,
        "SELECT (SELECT count(*) FROM kernel.jobs WHERE workspace_id = 'big')
              + (SELECT count(*) FROM kernel.events WHERE workspace_id = 'big')
              + (SELECT count(*) FROM kernel.changes WHERE workspace_id = 'big')
              + (SELECT count(*) FROM kernel.change_heads WHERE workspace_id = 'big')",
    )
    .await;
    assert_eq!(left, 0);
    let other = count(
        &db,
        "SELECT (SELECT count(*) FROM kernel.jobs WHERE workspace_id = 'other')
              + (SELECT count(*) FROM kernel.events WHERE workspace_id = 'other')
              + (SELECT count(*) FROM kernel.changes WHERE workspace_id = 'other')
              + (SELECT count(*) FROM kernel.change_heads WHERE workspace_id = 'other')",
    )
    .await;
    assert_eq!(other, 3 + 3 + 1 + 1);

    let again = purge_workspace(&state.db, "big").await.unwrap();
    assert_eq!(
        again,
        Purged {
            batches: 1,
            ..Purged::default()
        },
        "idempotent"
    );
}

/// The sweep deletes what reached a purged workspace's kernel rows after its
/// purge, however recent, but a delivery still pending or running survives,
/// with the event it reads; a live workspace's recent rows are untouched.
#[sqlx::test(migrations = "../../migrations")]
async fn the_sweep_deletes_leftovers_but_keeps_unfinished_deliveries(db: PgPool) {
    let state = runtime(&db).await;
    tombstoned(&db, "gone", 0).await;
    for workspace in ["gone", "live"] {
        delivered_events(&db, workspace, 2).await;
        change(&db, workspace).await;
    }
    sqlx::query(
        "INSERT INTO kernel.jobs
             (queue, kind, payload, workspace_id, state, max_attempts, finished_at, last_error)
         VALUES ('default', 'test.purge', '{}', 'gone', 'failed', 1, now(), 'gave up')",
    )
    .execute(&db)
    .await
    .unwrap();
    undelivered_event(&db, "gone", "pending").await;
    undelivered_event(&db, "gone", "running").await;

    sweep(&state).await;

    let gone: Vec<(String, String)> = sqlx::query_as(
        "SELECT kind, state FROM kernel.jobs WHERE workspace_id = 'gone' ORDER BY state",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        gone,
        [
            ("test.consumer".to_owned(), "pending".to_owned()),
            ("test.consumer".to_owned(), "running".to_owned()),
        ],
        "only the unfinished deliveries are left"
    );
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT payload->>'state' FROM kernel.events WHERE workspace_id = 'gone' ORDER BY 1",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(events, ["pending", "running"], "with the events they read");
    assert_eq!(
        count(
            &db,
            "SELECT (SELECT count(*) FROM kernel.changes WHERE workspace_id = 'gone')
                  + (SELECT count(*) FROM kernel.change_heads WHERE workspace_id = 'gone')"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT (SELECT count(*) FROM kernel.jobs WHERE workspace_id = 'live')
                  + (SELECT count(*) FROM kernel.events WHERE workspace_id = 'live')
                  + (SELECT count(*) FROM kernel.changes WHERE workspace_id = 'live')
                  + (SELECT count(*) FROM kernel.change_heads WHERE workspace_id = 'live')"
        )
        .await,
        2 + 2 + 1 + 1,
        "a live workspace keeps its recent rows"
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM kernel.purged_workspaces").await,
        1,
        "the tombstone stays while rows are left"
    );
}

/// A tombstone goes once it is older than 30 days and nothing of its
/// workspace is left; a recent one, or one with a delivery still waiting,
/// stays.
#[sqlx::test(migrations = "../../migrations")]
async fn a_tombstone_goes_once_old_and_nothing_is_left(db: PgPool) {
    let state = runtime(&db).await;
    tombstoned(&db, "old-empty", 31).await;
    tombstoned(&db, "old-waiting", 31).await;
    tombstoned(&db, "recent-empty", 1).await;
    undelivered_event(&db, "old-waiting", "pending").await;

    sweep(&state).await;

    let left: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM kernel.purged_workspaces ORDER BY 1")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(left, ["old-waiting", "recent-empty"]);
}

/// The tombstone is written in a transaction working in its workspace and
/// holds the id and the time only; writing it twice keeps one row.
#[sqlx::test(migrations = "../../migrations")]
async fn a_tombstone_holds_the_id_and_the_time_only(db: PgPool) {
    let state = runtime(&db).await;
    for _ in 0..2 {
        let mut tx = crate::db::begin_in(&state.db, "gone").await.unwrap();
        tombstone(&mut tx, "gone").await.unwrap();
        tx.commit().await.unwrap();
    }
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
          WHERE table_schema = 'kernel' AND table_name = 'purged_workspaces'
          ORDER BY ordinal_position",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(columns, ["workspace_id", "purged_at"]);
    assert_eq!(
        count(&db, "SELECT count(*) FROM kernel.purged_workspaces").await,
        1
    );
    // Another workspace's transaction sees no tombstone but its own, and
    // cannot write one for another workspace, even as raw SQL.
    let mut tx = crate::db::begin_in(&state.db, "elsewhere").await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.purged_workspaces")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 0);
    let forged =
        sqlx::query("INSERT INTO kernel.purged_workspaces (workspace_id) VALUES ('gone2')")
            .execute(&mut *tx)
            .await
            .unwrap_err();
    assert!(
        forged.to_string().contains("row-level security"),
        "{forged}"
    );
    drop(tx);
    assert_eq!(
        count(&db, "SELECT count(*) FROM kernel.purged_workspaces").await,
        1
    );
}

/// Which workspaces are live, and whose check fails.
#[derive(Debug, Default)]
struct Ports {
    live: Vec<WorkspaceId>,
    broken: Vec<WorkspaceId>,
}

impl Membership for Ports {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        _principal: &'a crate::auth::Principal,
        _workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    fn workspace_live<'a>(&'a self, _db: &'a PgPool, workspace: WorkspaceId) -> LiveFuture<'a> {
        let live = self.live.contains(&workspace);
        let broken = self.broken.contains(&workspace);
        Box::pin(async move {
            if broken {
                Err(crate::Error::NotFound)
            } else {
                Ok(live)
            }
        })
    }
}

/// Every row of `workspace` in the queue, the log and the feed.
async fn rows_of(db: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM kernel.jobs WHERE workspace_id = $1)
              + (SELECT count(*) FROM kernel.events WHERE workspace_id = $1)
              + (SELECT count(*) FROM kernel.changes WHERE workspace_id = $1)
              + (SELECT count(*) FROM kernel.change_heads WHERE workspace_id = $1)",
    )
    .bind(workspace)
    .fetch_one(db)
    .await
    .unwrap()
}

/// A tombstoned workspace that is live again (its row came back) is left
/// alone: nothing of it is deleted and its tombstone stays, even with
/// nothing left. One whose check fails is skipped too, and the sweep goes on
/// to the others.
#[sqlx::test(migrations = "../../migrations")]
async fn the_sweep_leaves_a_live_workspace_alone(db: PgPool) {
    let [live, live_empty, broken, gone] = std::array::from_fn(|_| WorkspaceId::new());
    let state = runtime(&db)
        .await
        .with_membership(std::sync::Arc::new(Ports {
            live: vec![live, live_empty],
            broken: vec![broken],
        }));
    for workspace in [live, live_empty, broken, gone] {
        tombstoned(&db, &workspace.to_string(), 31).await;
    }
    for workspace in [live, broken, gone] {
        delivered_events(&db, &workspace.to_string(), 2).await;
        change(&db, &workspace.to_string()).await;
    }

    sweep(&state).await;

    assert_eq!(rows_of(&db, &live.to_string()).await, 2 + 2 + 1 + 1);
    assert_eq!(rows_of(&db, &broken.to_string()).await, 2 + 2 + 1 + 1);
    assert_eq!(rows_of(&db, &gone.to_string()).await, 0, "the others go on");
    let mut left: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM kernel.purged_workspaces")
            .fetch_all(&db)
            .await
            .unwrap();
    left.sort();
    let mut kept = vec![live.to_string(), live_empty.to_string(), broken.to_string()];
    kept.sort();
    assert_eq!(
        left, kept,
        "the gone workspace's old, empty tombstone is dropped"
    );
}

/// The purge deletes the workspace's idempotency keys too, expired or not,
/// and leaves another workspace's and the instance-wide ones.
#[sqlx::test(migrations = "../../migrations")]
async fn the_purge_deletes_the_workspaces_idempotency_keys(db: PgPool) {
    let state = runtime(&db).await;
    for (key, workspace, expires) in [
        ("k-1", Some("gone"), "1 day"),
        ("k-2", Some("gone"), "-1 day"),
        ("k-3", Some("other"), "1 day"),
        ("k-4", None, "1 day"),
    ] {
        sqlx::query(
            "INSERT INTO kernel.idempotency
                 (principal, key, operation_id, request_hash, expires_at, workspace_id)
             VALUES ('client:app', $1, 'test.op', '\\x00', now() + $3::interval, $2)",
        )
        .bind(key)
        .bind(workspace)
        .bind(expires)
        .execute(&db)
        .await
        .unwrap();
    }
    let purged = purge_workspace(&state.db, "gone").await.unwrap();
    assert_eq!(purged.keys, 2);
    let left: Vec<String> = sqlx::query_scalar("SELECT key FROM kernel.idempotency ORDER BY key")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(left, ["k-3", "k-4"]);
}
