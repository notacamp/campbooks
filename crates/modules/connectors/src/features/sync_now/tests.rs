//! Sync now runs at once, collapses with a waiting run, and resets the
//! interval.

use axum::http::StatusCode;
use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn sync_now_collapses_and_resets_the_interval(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let id = kit.connect(&ana, ws).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    kit.run_jobs().await;
    // Idle for a while: the interval grew.
    kit.next_run().await;
    kit.next_run().await;
    assert!(kit.stream().await["interval_seconds"].as_i64().unwrap() > 60);

    let uri = format!("/api/v1/w/{ws}/connections/{id}/sync");
    for _ in 0..2 {
        let response = kit.post(&ana, &uri, serde_json::json!({})).await;
        assert_eq!(response.status, StatusCode::ACCEPTED);
        assert_eq!(response.json["streams"], 1);
    }
    assert_eq!(kit.pending("connectors.sync_stream").await, 1);
    assert_eq!(kit.stream().await["interval_seconds"], 60);
    // The dispatcher does not double it while it waits.
    assert_eq!(
        crate::domain::schedule::dispatch(&kit.state.db, kit.clock.now())
            .await
            .unwrap(),
        0
    );
    kit.run_jobs().await;
    assert_eq!(kit.pending("connectors.sync_stream").await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_dead_run_comes_back_after_its_lease(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect(&ana, ws).await;
    // The first run is enqueued and leased; pretend its job vanished.
    sqlx::query!("DELETE FROM kernel.jobs WHERE kind = 'connectors.sync_stream'")
        .execute(&kit.db)
        .await
        .unwrap();
    assert_eq!(kit.tick(chrono::TimeDelta::minutes(5)).await, 0);
    assert_eq!(kit.tick(chrono::TimeDelta::minutes(11)).await, 1);
    assert_eq!(kit.stream().await["phase"], "incremental");
}
