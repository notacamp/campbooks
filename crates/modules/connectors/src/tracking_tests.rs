//! The module's tracked events (ADR 0020, campbooks-cloud#329), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums, counts and times, and nothing for a change that
//! changes nothing. Sends are checked in `send_message/tests.rs`, outbound
//! changes in `request_changes/tests.rs`, calendar changes in
//! `sync_stream/calendar_tests.rs` and IMAP connections in
//! `connect_imap/tests.rs`, next to the worlds they build.

use axum::http::StatusCode;
use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{Role, UserId, WorkspaceId},
    testing::{self, TrackedRow},
    tracking::{Actor, TrackingContext},
};
use chrono::TimeDelta;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    domain::model::MailChoice,
    events::MemberLeft,
    testkit::{ACCOUNT_SUB, Caller, Kit, synthetic},
};

pub(crate) async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// `event` is version 1 of `name`, in `workspace`, from `actor` (an actor
/// object) in `operation` (`None`: none), and holds no address or name.
pub(crate) fn tracked_as(
    event: &TrackedRow,
    name: &str,
    workspace: WorkspaceId,
    actor: &serde_json::Value,
    operation: Option<&str>,
) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    let envelope = &event.envelope;
    assert_eq!(envelope["name"], name);
    assert_eq!(&envelope["actor"], actor, "{name}");
    assert_eq!(
        envelope["operation"],
        operation.map_or(serde_json::Value::Null, Into::into),
        "{name}"
    );
    assert_eq!(envelope["workspace_id"], workspace.to_string(), "{name}");
    assert_eq!(
        envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    nothing_personal(event);
}

/// The actor of a job of `kind`.
pub(crate) fn job(kind: &str) -> serde_json::Value {
    json!({"type": "system", "id": kind})
}

/// Ids, enums, counts and times only: no address, display name, host,
/// subject or provider id anywhere in the body.
pub(crate) fn nothing_personal(event: &TrackedRow) {
    let body = event.body.to_string();
    for word in [
        "@",
        "example",
        "Sam",
        "Rivera",
        "Message",
        "google-sub",
        "zoho-zuid",
        "INBOX",
        "thread-",
        "imap.",
        "smtp.",
    ] {
        assert!(
            !body.contains(word),
            "{} holds personal data or content ({word}): {body}",
            event.name
        );
    }
}

/// A member of a fresh workspace: the caller, the workspace and their user
/// id.
fn member(kit: &Kit, sub: &str) -> (Caller, WorkspaceId, UserId) {
    let ws = WorkspaceId::new();
    let caller = kit.user(sub);
    let id = kit.join(&caller, ws, Role::Member);
    (caller, ws, id)
}

/// `caller`'s Google consent coming back through the identity module's front
/// door: tracked in its sign-in callback, before anyone is signed in.
async fn adopt_at_the_front_door(kit: &Kit, caller: &Caller, ws: WorkspaceId) -> String {
    let context = TrackingContext::new(Actor::anonymous()).in_operation("identity.finish_sign_in");
    campbooks_kernel::tracking::scope(context, kit.adopt(caller, ws, ACCOUNT_SUB, None))
        .await
        .unwrap_or_else(|e| panic!("adopting: {e:?}"))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_is_tracked_from_added_to_removed(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ana, ws, ana_id) = member(&kit, "ana");
    let anonymous = json!({"type": "anonymous"});
    let user = json!({"type": "user", "id": "ana"});

    let id = adopt_at_the_front_door(&kit, &ana, ws).await;
    let added = tracked(&kit, "connectors.connection_added").await;
    assert_eq!(added.len(), 1);
    tracked_as(
        &added[0],
        "connectors.connection_added",
        ws,
        &anonymous,
        Some("identity.finish_sign_in"),
    );
    let connection = &added[0].body["connection"];
    assert_eq!(connection["id"], id);
    assert_eq!(connection["provider"], "google");
    assert_eq!(connection["granted_by"], ana_id.to_string());
    assert_eq!(connection["capabilities"], json!(["messages"]));
    assert_eq!(connection["health"], "ok");
    assert_eq!(connection["mailboxes"], 1);
    assert!(connection.get("health_reason").is_none());
    assert!(connection.get("address").is_none());

    // The same account again is a reconnect, not a second connection.
    adopt_at_the_front_door(&kit, &ana, ws).await;
    assert_eq!(tracked(&kit, "connectors.connection_added").await.len(), 1);
    let reconnected = tracked(&kit, "connectors.connection_reconnected").await;
    assert_eq!(reconnected.len(), 1);
    tracked_as(
        &reconnected[0],
        "connectors.connection_reconnected",
        ws,
        &anonymous,
        Some("identity.finish_sign_in"),
    );
    assert_eq!(reconnected[0].body["before_health"], "ok");
    assert_eq!(reconnected[0].body["capabilities_added"], json!([]));
    assert_eq!(reconnected[0].body["mailboxes_added"], 0);
    assert_eq!(reconnected[0].body["connection"]["id"], id);

    // Sync now: its streams, by count.
    let sync = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/{id}/sync"),
            json!({}),
        )
        .await;
    assert_eq!(sync.status, StatusCode::ACCEPTED, "{:?}", sync.json);
    let requested = tracked(&kit, "connectors.connection_sync_requested").await;
    assert_eq!(requested.len(), 1);
    tracked_as(
        &requested[0],
        "connectors.connection_sync_requested",
        ws,
        &user,
        Some("connectors.sync_now"),
    );
    assert_eq!(requested[0].body["streams"], 1);
    assert_eq!(requested[0].body["connection"]["id"], id);

    // Disconnected: the connection as it stood; once.
    let uri = format!("/api/v1/w/{ws}/connections/{id}?mail=keep");
    assert_eq!(kit.delete(&ana, &uri).await.status, StatusCode::NO_CONTENT);
    assert_ne!(kit.delete(&ana, &uri).await.status, StatusCode::NO_CONTENT);
    let removed = tracked(&kit, "connectors.connection_removed").await;
    assert_eq!(removed.len(), 1);
    tracked_as(
        &removed[0],
        "connectors.connection_removed",
        ws,
        &user,
        Some("connectors.disconnect"),
    );
    assert_eq!(removed[0].body["cause"], "disconnected");
    assert_eq!(removed[0].body["mail"], "keep");
    assert_eq!(removed[0].body["connection"]["id"], id);
    assert_eq!(removed[0].body["connection"]["mailboxes"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_departed_members_connection_removal_is_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ana, ws, ana_id) = member(&kit, "ana");
    let connection = kit.connect(&ana, ws).await;
    kit.leave(&ana, ws);
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &MemberLeft {
                workspace: ws.to_string(),
                user: ana_id,
                mail: MailChoice::Delete,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;

    let removed = tracked(&kit, "connectors.connection_removed").await;
    assert_eq!(removed.len(), 1);
    tracked_as(
        &removed[0],
        "connectors.connection_removed",
        ws,
        &job("connectors.revoke_departed_grants"),
        None,
    );
    assert_eq!(removed[0].body["cause"], "member_left");
    assert_eq!(removed[0].body["mail"], "delete");
    assert_eq!(removed[0].body["connection"]["id"], connection["id"]);
    assert_eq!(
        removed[0].body["connection"]["granted_by"],
        ana_id.to_string()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_sync_run_is_tracked_once_when_it_stored_something(db: PgPool) {
    let kit = Kit::new(db).await;
    for n in 0..60 {
        kit.google.seed(
            &synthetic(n),
            &["INBOX"],
            kit.clock.now() - TimeDelta::hours(1) - TimeDelta::minutes(i64::try_from(n).unwrap()),
        );
    }
    let (ana, ws, _) = member(&kit, "ana");
    kit.connect(&ana, ws).await;
    kit.run_jobs().await;

    // The backfill: one summary per run, the messages counted, never named.
    let synced = tracked(&kit, "connectors.stream_synced").await;
    assert!(!synced.is_empty());
    let (mut changes, mut batches) = (0, 0);
    for event in &synced {
        tracked_as(
            event,
            "connectors.stream_synced",
            ws,
            &job("connectors.sync_stream"),
            None,
        );
        assert_eq!(event.body["provider"], "google");
        assert_eq!(event.body["before_phase"], "backfill");
        assert_eq!(event.body["stream"]["capability"], "messages");
        assert!(event.body["stream"]["inbox_id"].is_string());
        changes += event.body["changes"].as_i64().unwrap();
        batches += event.body["batches"].as_i64().unwrap();
    }
    assert_eq!(changes, 60);
    assert!(batches >= 1);
    let last = &synced.last().unwrap().body["stream"];
    assert_eq!(last["phase"], "incremental");
    assert_eq!(last["paused"], false);

    // A run that finds nothing tracks nothing.
    kit.next_run().await;
    assert_eq!(
        tracked(&kit, "connectors.stream_synced").await.len(),
        synced.len()
    );

    // New mail is one more summary.
    kit.google
        .deliver(&synthetic(200), &["INBOX", "UNREAD"], kit.clock.now());
    kit.next_run().await;
    let after = tracked(&kit, "connectors.stream_synced").await;
    assert_eq!(after.len(), synced.len() + 1);
    let newest = &after.last().unwrap().body;
    assert_eq!(newest["before_phase"], "incremental");
    assert_eq!(newest["changes"], 1);
    assert_eq!(newest["batches"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_s_health_changes_are_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ana, ws, _) = member(&kit, "ana");
    let connection = kit.connect(&ana, ws).await;
    kit.run_jobs().await;
    let health = || tracked(&kit, "connectors.connection_health_changed");

    // Throttled, then ok again after a good run.
    kit.google
        .deliver(&synthetic(1), &["INBOX"], kit.clock.now());
    kit.google.rate_limit_next(120);
    kit.next_run().await;
    let changed = health().await;
    assert_eq!(changed.len(), 1);
    tracked_as(
        &changed[0],
        "connectors.connection_health_changed",
        ws,
        &job("connectors.sync_stream"),
        None,
    );
    assert_eq!(changed[0].body["before"], "ok");
    assert_eq!(changed[0].body["connection"]["id"], connection["id"]);
    assert_eq!(changed[0].body["connection"]["health"], "throttled");
    assert_eq!(
        changed[0].body["connection"]["health_reason"],
        "rate_limited"
    );
    kit.next_run().await;
    let changed = health().await;
    assert_eq!(changed.len(), 2);
    assert_eq!(changed[1].body["before"], "throttled");
    assert_eq!(changed[1].body["connection"]["health"], "ok");
    assert!(changed[1].body["connection"].get("health_reason").is_none());

    // The grant revoked: needs the member, once.
    kit.google.revoke_grant();
    kit.tick(TimeDelta::hours(2)).await;
    let changed = health().await;
    assert_eq!(changed.len(), 3);
    assert_eq!(changed[2].body["before"], "ok");
    assert_eq!(changed[2].body["connection"]["health"], "needs_reauth");
    assert_eq!(
        changed[2].body["connection"]["health_reason"],
        "grant_revoked"
    );

    // Connecting again is a reconnect, not a health change.
    kit.connect(&ana, ws).await;
    assert_eq!(health().await.len(), 3);
    let reconnected = tracked(&kit, "connectors.connection_reconnected").await;
    assert_eq!(reconnected.len(), 1);
    assert_eq!(reconnected[0].body["before_health"], "needs_reauth");
    assert_eq!(reconnected[0].body["connection"]["health"], "ok");
}
