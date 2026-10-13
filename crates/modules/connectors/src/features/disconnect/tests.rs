//! Disconnecting revokes the grant and deletes what the connection holds.

use axum::http::StatusCode;
use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::TimeDelta;
use sqlx::PgPool;

use crate::testkit::{Kit, synthetic};

#[sqlx::test(migrations = "../../../migrations")]
async fn the_grantor_disconnects_and_the_grant_is_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.google.seed(
        &synthetic(1),
        &["INBOX"],
        kit.clock.now() - TimeDelta::hours(1),
    );
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let connection = kit.connect(&ana, ws).await;
    kit.run_jobs().await;
    let id = connection["id"].as_str().unwrap();
    let uri = format!("/api/v1/w/{ws}/connections/{id}");

    // Someone who cannot see it, or a client token, cannot disconnect it.
    assert_eq!(kit.delete(&bea, &uri).await.status, StatusCode::NOT_FOUND);
    let client = kit.delete(&kit.client(ws), &uri).await;
    assert_eq!(client.status, StatusCode::FORBIDDEN);
    assert_eq!(client.json["code"], "connection_needs_user");

    assert_eq!(kit.delete(&ana, &uri).await.status, StatusCode::NO_CONTENT);
    let revoked = kit.google.revoked();
    assert_eq!(revoked.len(), 1);
    assert!(revoked[0].starts_with("fake-refresh"));
    let removed = kit.payloads(ws, "connectors.connection_removed").await;
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0]["inbox_ids"][0],
        connection["streams"][0]["inbox_id"]
    );
    let left: i64 = sqlx::query_scalar!(
        r#"SELECT (SELECT count(*) FROM connectors.connections)
                + (SELECT count(*) FROM connectors.credentials)
                + (SELECT count(*) FROM connectors.streams)
                + (SELECT count(*) FROM connectors.sync_due)
                + (SELECT count(*) FROM connectors.gmail_messages)
                + (SELECT count(*) FROM connectors.batches)
                + (SELECT count(*) FROM kernel.blob_refs WHERE owner_type LIKE 'connectors.%')
           AS "n!""#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(left, 0);
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
}
