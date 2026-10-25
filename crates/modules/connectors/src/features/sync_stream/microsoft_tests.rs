//! A Microsoft stream before reading lands (campbooks-cloud#316): it keeps
//! the grant alive and its health known, and reads nothing.

use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use crate::testkit::{Caller, Kit};

/// A kit with Microsoft mail on and ana's Outlook connected, its first run
/// done.
async fn connected(db: PgPool) -> (Kit, WorkspaceId, Caller) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect_microsoft(&ana, ws).await;
    kit.run_jobs().await;
    (kit, ws, ana)
}

async fn connection(kit: &Kit, ws: WorkspaceId, ana: &Caller) -> serde_json::Value {
    kit.get(ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0]
        .clone()
}

async fn due(kit: &Kit) -> DateTime<Utc> {
    serde_json::from_value(kit.stream().await["due_at"].clone()).unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_healthy_grant_waits_the_longest_interval_and_reads_nothing(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    // The stored access token is still valid: Microsoft is not asked.
    assert_eq!(kit.microsoft.token_calls(), 0);
    assert!(
        kit.microsoft.calls().is_empty(),
        "{:?}",
        kit.microsoft.calls()
    );
    let max = i64::from(kit.deps.config.poll.max_seconds);
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::seconds(max));
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    // Nothing is announced: no batch until reading lands.
    assert!(
        kit.payloads(ws, "connectors.messages_changed")
            .await
            .is_empty()
    );

    // Once the access token has expired, the run refreshes it (and keeps
    // the rotated refresh token), still reading nothing.
    kit.next_run().await;
    assert_eq!(kit.microsoft.token_calls(), 1);
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    kit.next_run().await;
    assert_eq!(kit.microsoft.token_calls(), 2, "the rotated token works");
    assert!(
        kit.microsoft
            .calls()
            .iter()
            .all(|call| call.ends_with("/oauth2/v2.0/token")),
        "no Graph call: {:?}",
        kit.microsoft.calls()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_microsoft_no_longer_honours_needs_reauth(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({
            "error": "invalid_grant",
            "error_codes": [70000],
            "error_description": "AADSTS70000: The provided grant has expired due to it being revoked."
        }),
    );
    kit.next_run().await;
    let connection = connection(&kit, ws, &ana).await;
    assert_eq!(connection["health"], "needs_reauth");
    assert_eq!(connection["health_reason"], "grant_revoked");
    assert_eq!(connection["streams"][0]["paused"], true);
    let events = kit.payloads(ws, "connectors.connection_needs_reauth").await;
    assert_eq!(events.len(), 1);
    // Nothing runs any more.
    assert_eq!(kit.tick(TimeDelta::hours(5)).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn consent_withdrawn_at_microsoft_needs_reauth(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({
            "error": "invalid_grant",
            "error_codes": [65001],
            "error_description": "AADSTS65001: The user or administrator has not consented."
        }),
    );
    kit.next_run().await;
    assert_eq!(connection(&kit, ws, &ana).await["health"], "needs_reauth");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttling_pauses_only_until_retry_after(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    kit.microsoft.throttle_refresh(120);
    kit.next_run().await;
    let throttled = connection(&kit, ws, &ana).await;
    assert_eq!(throttled["health"], "throttled");
    assert_eq!(throttled["health_reason"], "rate_limited");
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::seconds(120));
    // Not before.
    assert_eq!(kit.tick(TimeDelta::seconds(60)).await, 0);

    kit.microsoft.allow_refresh();
    kit.next_run().await;
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    let max = i64::from(kit.deps.config.poll.max_seconds);
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::seconds(max));
}
