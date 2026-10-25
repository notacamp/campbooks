//! Graph's change notifications (campbooks-cloud#316): the subscription a
//! run makes and renews, and the public callback, against the fake Graph.

use axum::http::StatusCode;
use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use crate::{
    microsoft::subscriptions::{LIFETIME, RENEW_BEFORE},
    testkit::{Caller, Kit, NOTIFICATION_URL, synthetic},
    testkit_microsoft::FakeSubscription,
};

const HOOK: &str = "/hooks/microsoft/mail";

/// Ana's Outlook connected in a new workspace of `kit`, its first sync done.
async fn connect(kit: &Kit) -> (WorkspaceId, Caller) {
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect_microsoft(&ana, ws).await;
    kit.run_jobs().await;
    (ws, ana)
}

/// The body Graph posts for `subscription`, with `client_state`.
fn posted(subscription: &str, client_state: &str, lifecycle: Option<&str>) -> String {
    let mut notification = serde_json::json!({
        "subscriptionId": subscription,
        "clientState": client_state,
        "changeType": "created",
        "resource": "Users/0d9ed3e4/Messages/AAMk",
        "tenantId": "72f988bf-86f1-41af-91ab-2d7cd011db47",
    });
    if let Some(event) = lifecycle {
        notification["lifecycleEvent"] = event.into();
    }
    serde_json::json!({"value": [notification]}).to_string()
}

/// The streams with a run pending.
async fn pending_streams(kit: &Kit) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT payload->>'stream' FROM kernel.jobs
          WHERE kind = 'connectors.sync_stream' AND state = 'pending' ORDER BY 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

async fn stream_of(kit: &Kit, ws: WorkspaceId) -> String {
    sqlx::query_scalar("SELECT id::text FROM connectors.streams WHERE workspace_id = $1")
        .bind(ws)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

async fn due_of(kit: &Kit, ws: WorkspaceId) -> DateTime<Utc> {
    sqlx::query_scalar(
        "SELECT d.due_at FROM connectors.sync_due d
           JOIN connectors.streams s ON s.id = d.stream_id WHERE s.workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

fn only(subscriptions: &[FakeSubscription]) -> FakeSubscription {
    assert_eq!(subscriptions.len(), 1, "{subscriptions:?}");
    subscriptions[0].clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_validation_handshake_answers_the_token_and_nothing_else(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    let answered = kit.hook(&format!("{HOOK}?validationToken=4242"), "").await;
    assert_eq!(answered.status, StatusCode::OK);
    assert_eq!(
        answered.content_type.as_deref(),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(answered.json, 4242);
    let text = kit
        .hook(
            &format!("{HOOK}?validationToken=Validation%3A+Testing+client+reachability"),
            "",
        )
        .await;
    assert_eq!(text.status, StatusCode::OK);
    let long = "a".repeat(2000);
    let refused = kit
        .hook(&format!("{HOOK}?validationToken={long}"), "")
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(pending_streams(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_notifications_the_callback_is_not_there(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let answered = kit.hook(&format!("{HOOK}?validationToken=4242"), "").await;
    assert_eq!(answered.status, StatusCode::NOT_FOUND);
    let off = Kit::new(kit.db.clone()).await;
    assert_eq!(
        off.hook(HOOK, &posted("7f105c7d", "x", None)).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_subscribes_and_then_polls_at_the_longest_interval(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    let (ws, _) = connect(&kit).await;
    let subscription = only(&kit.microsoft.subscriptions());
    assert_eq!(subscription.notification_url, NOTIFICATION_URL);
    assert_eq!(subscription.resource, "me/messages");
    assert_eq!(subscription.expires, kit.clock.now() + LIFETIME);
    // Only a keyed hash of the clientState is kept.
    let (id, tag): (String, Vec<u8>) = sqlx::query_as(
        "SELECT subscription_id, client_state_tag FROM connectors.microsoft_subscriptions",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(id, subscription.id);
    assert_eq!(tag.len(), 32);
    let row: String =
        sqlx::query_scalar("SELECT to_jsonb(m)::text FROM connectors.microsoft_subscriptions m")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert!(!row.contains(&subscription.client_state));
    let longest = TimeDelta::seconds(i64::from(kit.deps.config.poll.max_seconds));
    assert_eq!(due_of(&kit, ws).await, kit.clock.now() + longest);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_runs_that_connections_stream_only(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    let (ws, _) = connect(&kit).await;
    let (other, _) = connect(&kit).await;
    let subscriptions = kit.microsoft.subscriptions();
    assert_eq!(subscriptions.len(), 2);
    let ours = stream_of(&kit, ws).await;
    let row: String = sqlx::query_scalar(
        "SELECT subscription_id FROM connectors.microsoft_subscriptions WHERE workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let mine = subscriptions.iter().find(|s| s.id == row).unwrap();
    assert!(pending_streams(&kit).await.is_empty());

    let answered = kit
        .hook(HOOK, &posted(&mine.id, &mine.client_state, None))
        .await;
    assert_eq!(answered.status, StatusCode::ACCEPTED);
    assert_eq!(pending_streams(&kit).await, std::slice::from_ref(&ours));
    // Again while it is pending: absorbed.
    kit.hook(HOOK, &posted(&mine.id, &mine.client_state, None))
        .await;
    assert_eq!(pending_streams(&kit).await, [ours]);
    assert_ne!(stream_of(&kit, other).await, pending_streams(&kit).await[0]);

    // The run reads the new mail.
    let new = kit
        .microsoft
        .deliver("fld-inbox", &synthetic(1), kit.clock.now());
    kit.run_jobs().await;
    assert_eq!(
        kit.microsoft.raw_fetches(&new),
        1,
        "only the notified stream ran"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_wrong_client_state_or_an_unknown_subscription_is_404(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    connect(&kit).await;
    let subscription = only(&kit.microsoft.subscriptions());
    let wrong = kit
        .hook(HOOK, &posted(&subscription.id, "not-the-state", None))
        .await;
    assert_eq!(wrong.status, StatusCode::NOT_FOUND);
    let missing = kit.hook(HOOK, &posted(&subscription.id, "", None)).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let unknown = kit
        .hook(
            HOOK,
            &posted(
                "00000000-0000-0000-0000-000000000000",
                &subscription.client_state,
                None,
            ),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let shaped = kit
        .hook(HOOK, &posted("../evil", &subscription.client_state, None))
        .await;
    assert_eq!(shaped.status, StatusCode::NOT_FOUND);
    assert!(pending_streams(&kit).await.is_empty());
    // Not JSON: refused.
    assert_eq!(
        kit.hook(HOOK, "{not json").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_body_is_bounded(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    connect(&kit).await;
    let subscription = only(&kit.microsoft.subscriptions());
    let huge = format!(
        r#"{{"value": [], "padding": "{}"}}"#,
        "x".repeat(super::MAX_BODY)
    );
    assert_eq!(
        kit.hook(HOOK, &huge).await.status,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let one = serde_json::json!({
        "subscriptionId": subscription.id, "clientState": subscription.client_state
    });
    let many = serde_json::json!({"value": vec![one; super::MAX_NOTIFICATIONS + 1]});
    assert_eq!(
        kit.hook(HOOK, &many.to_string()).await.status,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(pending_streams(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stream_that_needs_the_member_is_not_run(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    connect(&kit).await;
    let subscription = only(&kit.microsoft.subscriptions());
    kit.clock.advance(TimeDelta::hours(5));
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({"error": "invalid_grant", "error_codes": [70000]}),
    );
    kit.next_run().await;
    assert_eq!(kit.stream().await["paused"], true);
    let answered = kit
        .hook(
            HOOK,
            &posted(&subscription.id, &subscription.client_state, None),
        )
        .await;
    assert_eq!(answered.status, StatusCode::ACCEPTED);
    assert!(pending_streams(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_subscription_is_renewed_before_it_expires(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    connect(&kit).await;
    let made = only(&kit.microsoft.subscriptions());
    // Runs while it has more than a day left leave it as it is.
    kit.next_run().await;
    assert!(!kit.microsoft.calls().iter().any(|c| c.starts_with("PATCH")));
    // Within a day of its end: renewed, the same subscription.
    kit.clock
        .advance(LIFETIME - RENEW_BEFORE + TimeDelta::minutes(1));
    kit.next_run().await;
    let renewed = only(&kit.microsoft.subscriptions());
    assert_eq!(renewed.id, made.id);
    assert_eq!(renewed.client_state, made.client_state);
    assert_eq!(renewed.expires, kit.clock.now() + LIFETIME);
    let stored: DateTime<Utc> =
        sqlx::query_scalar("SELECT expires_at FROM connectors.microsoft_subscriptions")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(stored, renewed.expires);
    // Graph forgot it: a new one, with a new clientState.
    kit.microsoft.drop_subscriptions();
    kit.clock
        .advance(LIFETIME - RENEW_BEFORE + TimeDelta::minutes(1));
    kit.next_run().await;
    let again = only(&kit.microsoft.subscriptions());
    assert_ne!(again.id, made.id);
    assert_ne!(again.client_state, made.client_state);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn graph_asking_to_reauthorize_or_removing_it_is_handled_by_the_run(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    let (ws, _) = connect(&kit).await;
    let made = only(&kit.microsoft.subscriptions());
    let renew = kit
        .hook(
            HOOK,
            &posted(
                &made.id,
                &made.client_state,
                Some("reauthorizationRequired"),
            ),
        )
        .await;
    assert_eq!(renew.status, StatusCode::ACCEPTED);
    assert_eq!(pending_streams(&kit).await.len(), 1);
    kit.run_jobs().await;
    assert!(kit.microsoft.calls().iter().any(|c| c.starts_with("PATCH")));
    assert_eq!(only(&kit.microsoft.subscriptions()).id, made.id);

    kit.microsoft.drop_subscriptions();
    let removed = kit
        .hook(
            HOOK,
            &posted(&made.id, &made.client_state, Some("subscriptionRemoved")),
        )
        .await;
    assert_eq!(removed.status, StatusCode::ACCEPTED);
    kit.run_jobs().await;
    let again = only(&kit.microsoft.subscriptions());
    assert_ne!(again.id, made.id);
    let longest = TimeDelta::seconds(i64::from(kit.deps.config.poll.max_seconds));
    assert_eq!(due_of(&kit, ws).await, kit.clock.now() + longest);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_subscription_polls_and_is_tried_again_an_hour_later(db: PgPool) {
    let kit = Kit::with_microsoft_notifications(db).await;
    kit.microsoft.refuse_subscriptions(true);
    let (ws, _) = connect(&kit).await;
    assert!(kit.microsoft.subscriptions().is_empty());
    let longest = TimeDelta::seconds(i64::from(kit.deps.config.poll.max_seconds));
    assert!(due_of(&kit, ws).await < kit.clock.now() + longest, "polled");
    let tries = |kit: &Kit| {
        kit.microsoft
            .calls()
            .iter()
            .filter(|c| *c == "POST /v1.0/subscriptions")
            .count()
    };
    assert_eq!(tries(&kit), 1);
    kit.next_run().await;
    assert_eq!(tries(&kit), 1, "not again within the hour");
    kit.microsoft.refuse_subscriptions(false);
    kit.clock.advance(TimeDelta::hours(1));
    kit.next_run().await;
    assert_eq!(tries(&kit), 2);
    assert_eq!(kit.microsoft.subscriptions().len(), 1);
    kit.microsoft.clear_calls();
}
