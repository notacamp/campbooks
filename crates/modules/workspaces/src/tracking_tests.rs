//! The module's tracked events (ADR 0020, campbooks-cloud#328), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums, counts and times, and nothing for a change that
//! changes nothing. Signup requests and codes are checked in
//! `admin_tests.rs` (they reuse its world of administrators).

use axum::http::StatusCode;
use campbooks_kernel::{
    DomainEvent,
    jobs::Worker,
    tenancy::WorkspaceId,
    testing::{self, TrackedRow},
};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    events::{AccountSuspended, AccountUnsuspended},
    testkit::Kit,
};

async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// `event` is version 1 of `name`, in `workspace`, from `sub`'s own request
/// to `operation`, and holds no address or name.
pub(crate) fn by_user(
    event: &TrackedRow,
    name: &str,
    workspace: Option<WorkspaceId>,
    sub: &str,
    operation: &str,
) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(
        event.workspace_id,
        workspace.map(|w| *w.as_uuid()),
        "{name}"
    );
    let envelope = &event.envelope;
    assert_eq!(envelope["name"], name);
    assert_eq!(
        envelope["actor"],
        json!({"type": "user", "id": sub}),
        "{name}"
    );
    assert_eq!(envelope["operation"], operation, "{name}");
    assert_eq!(
        envelope["workspace_id"],
        workspace.map_or(Value::Null, |w| w.to_string().into()),
        "{name}"
    );
    assert_eq!(
        envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    nothing_personal(event);
}

/// Ids, enums, counts and times only: no address and no workspace name
/// anywhere in the body (every name in these tests starts `Shop`).
pub(crate) fn nothing_personal(event: &TrackedRow) {
    let body = event.body.to_string();
    assert!(
        !body.contains('@') && !body.contains("Shop") && !body.contains("example"),
        "{} holds personal data or content: {body}",
        event.name
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_is_tracked_from_creation_to_purge(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ana = kit.user("ana");
    let ws = kit.create_workspace(&ana, "Shop").await;

    let created = tracked(&kit, "workspaces.workspace_created").await;
    assert_eq!(created.len(), 1);
    by_user(
        &created[0],
        "workspaces.workspace_created",
        Some(ws),
        "ana",
        "workspaces.create",
    );
    let snapshot = &created[0].body["workspace_snapshot"];
    assert_eq!(snapshot["id"], ws.to_string());
    assert_eq!(snapshot["kind"], "business");
    assert_eq!(snapshot["accent"], "business");
    assert_eq!(snapshot["member_count"], 1);
    assert!(snapshot.get("name").is_none());
    assert!(snapshot.get("deleted_at").is_none());

    let joined = tracked(&kit, "workspaces.member_joined").await;
    assert_eq!(joined.len(), 1);
    by_user(
        &joined[0],
        "workspaces.member_joined",
        Some(ws),
        "ana",
        "workspaces.create",
    );
    assert_eq!(joined[0].body["via"], "created");
    assert_eq!(joined[0].body["member"]["role"], "owner");
    assert_eq!(joined[0].body["member"]["suspended"], false);
    assert_eq!(
        joined[0].body["member"]["user_id"],
        kit.user_id("ana").await.to_string()
    );
    assert!(joined[0].body.get("invitation").is_none());

    // Renamed: which setting, never its value; the same name again is a
    // no-op and tracks nothing.
    let uri = format!("/api/v1/w/{ws}");
    for _ in 0..2 {
        let renamed = kit.patch(&ana, &uri, json!({ "name": "Shop two" })).await;
        assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json);
    }
    let recoloured = kit.patch(&ana, &uri, json!({ "accent": "forest" })).await;
    assert_eq!(recoloured.status, StatusCode::OK);
    let updated = tracked(&kit, "workspaces.workspace_updated").await;
    assert_eq!(updated.len(), 2);
    for event in &updated {
        by_user(
            event,
            "workspaces.workspace_updated",
            Some(ws),
            "ana",
            "workspaces.update",
        );
    }
    assert_eq!(updated[0].body["fields"], json!(["name"]));
    assert_eq!(updated[0].body["workspace_snapshot"]["accent"], "business");
    assert_eq!(updated[1].body["fields"], json!(["accent"]));
    assert_eq!(updated[1].body["workspace_snapshot"]["accent"], "forest");

    // Deleted: when, and when it is purged.
    let deleted = kit.delete(&ana, &uri).await;
    assert_eq!(deleted.status, StatusCode::ACCEPTED, "{:?}", deleted.json);
    let events = tracked(&kit, "workspaces.workspace_deleted").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "workspaces.workspace_deleted",
        Some(ws),
        "ana",
        "workspaces.delete",
    );
    let snapshot = &events[0].body["workspace_snapshot"];
    assert!(snapshot["deleted_at"].is_string());
    assert_eq!(snapshot["purge_after"], deleted.json["purge_after"]);

    // Purged at the end of the grace period, by the job, for Ana.
    let worker = Worker::new(kit.state.clone());
    worker.run_until_idle().await;
    assert!(
        tracked(&kit, "workspaces.workspace_purged")
            .await
            .is_empty()
    );
    sqlx::query(
        "UPDATE workspaces.workspaces SET purge_after = now() - interval '1 second' WHERE id = $1",
    )
    .bind(ws)
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE kind = 'workspaces.purge_workspace'")
        .execute(&kit.db)
        .await
        .unwrap();
    let stats = worker.run_until_idle().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
    let purged = tracked(&kit, "workspaces.workspace_purged").await;
    assert_eq!(purged.len(), 1);
    let event = &purged[0];
    assert_eq!(event.version, 1);
    assert_eq!(event.workspace_id, Some(*ws.as_uuid()));
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "system", "id": "workspaces.purge_workspace"})
    );
    assert_eq!(
        event.envelope["on_behalf_of"],
        json!({"type": "user", "id": "ana"})
    );
    assert_eq!(event.body["workspace_snapshot"]["id"], ws.to_string());
    assert!(event.body["workspace_snapshot"]["deleted_at"].is_string());
    nothing_personal(event);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invitation_is_tracked_by_id_and_joining_carries_it(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ana = kit.user("ana");
    let ws = kit.create_workspace(&ana, "Shop").await;
    let uri = format!("/api/v1/w/{ws}/invitations");

    let first = kit
        .post(
            &ana,
            &uri,
            json!({ "email": "bea@example.com", "role": "admin" }),
        )
        .await;
    assert_eq!(first.status, StatusCode::CREATED, "{:?}", first.json);
    let again = kit
        .post(
            &ana,
            &uri,
            json!({ "email": "Bea@example.com", "role": "admin" }),
        )
        .await;
    assert_eq!(again.status, StatusCode::CREATED, "{:?}", again.json);

    let invited = tracked(&kit, "workspaces.invitation_created").await;
    assert_eq!(invited.len(), 2);
    for event in &invited {
        by_user(
            event,
            "workspaces.invitation_created",
            Some(ws),
            "ana",
            "workspaces.invite_member",
        );
        let invitation = &event.body["invitation"];
        assert_eq!(invitation["role"], "admin");
        assert_eq!(invitation["state"], "pending");
        assert_eq!(
            invitation["invited_by"],
            kit.user_id("ana").await.to_string()
        );
        assert!(invitation.get("email").is_none());
    }
    assert_eq!(invited[0].body["invitation"]["id"], first.json["id"]);
    assert_eq!(invited[0].body["replaced"], json!([]));
    assert_eq!(invited[1].body["invitation"]["id"], again.json["id"]);
    assert_eq!(invited[1].body["replaced"], json!([first.json["id"]]));

    // Bea accepts the second: she joins by it, accepted.
    let token = again.json["accept_url"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let bea = kit.user("bea");
    let accepted = kit
        .post(
            &bea,
            "/api/v1/workspaces/invitations/accept",
            json!({ "token": token }),
        )
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.json);
    let joined = tracked(&kit, "workspaces.member_joined").await;
    assert_eq!(joined.len(), 2, "Ana creating, Bea accepting");
    by_user(
        &joined[1],
        "workspaces.member_joined",
        Some(ws),
        "bea",
        "workspaces.accept_invitation",
    );
    let body = &joined[1].body;
    assert_eq!(body["via"], "invitation");
    assert_eq!(
        body["member"]["user_id"],
        kit.user_id("bea").await.to_string()
    );
    assert_eq!(body["member"]["role"], "admin");
    assert_eq!(body["invitation"]["id"], again.json["id"]);
    assert_eq!(body["invitation"]["state"], "accepted");
    assert_eq!(
        body["invitation"]["accepted_by"],
        kit.user_id("bea").await.to_string()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn role_changes_and_departures_are_tracked(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ana = kit.user("ana");
    let ws = kit.create_workspace(&ana, "Shop").await;
    let bea = kit.join(&ana, ws, "bea", "member").await;
    kit.join(&ana, ws, "cid", "member").await;
    let (bea_id, cid_id) = (kit.user_id("bea").await, kit.user_id("cid").await);

    // Promoted, then promoted again: the second changes nothing.
    let member = format!("/api/v1/w/{ws}/members/{bea_id}");
    for _ in 0..2 {
        let promoted = kit.patch(&ana, &member, json!({ "role": "admin" })).await;
        assert_eq!(promoted.status, StatusCode::OK, "{:?}", promoted.json);
    }
    let changed = tracked(&kit, "workspaces.member_role_changed").await;
    assert_eq!(changed.len(), 1);
    by_user(
        &changed[0],
        "workspaces.member_role_changed",
        Some(ws),
        "ana",
        "workspaces.change_role",
    );
    assert_eq!(changed[0].body["before"], "member");
    assert_eq!(changed[0].body["member"]["role"], "admin");
    assert_eq!(changed[0].body["member"]["user_id"], bea_id.to_string());

    // Cid is removed, his mail kept; Bea leaves.
    let removed = kit
        .delete(&ana, &format!("/api/v1/w/{ws}/members/{cid_id}?mail=keep"))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{:?}", removed.json);
    let left = kit
        .delete(&bea, &format!("/api/v1/w/{ws}/membership"))
        .await;
    assert_eq!(left.status, StatusCode::NO_CONTENT, "{:?}", left.json);

    let departures = tracked(&kit, "workspaces.member_left").await;
    assert_eq!(departures.len(), 2);
    by_user(
        &departures[0],
        "workspaces.member_left",
        Some(ws),
        "ana",
        "workspaces.remove_member",
    );
    assert_eq!(departures[0].body["how"], "removed");
    assert_eq!(departures[0].body["mail"], "keep");
    assert_eq!(departures[0].body["member"]["user_id"], cid_id.to_string());
    assert_eq!(departures[0].body["member"]["role"], "member");
    by_user(
        &departures[1],
        "workspaces.member_left",
        Some(ws),
        "bea",
        "workspaces.leave",
    );
    assert_eq!(departures[1].body["how"], "left");
    assert_eq!(departures[1].body["mail"], "delete");
    assert_eq!(departures[1].body["member"]["role"], "admin");
}

/// Publish an account's event as identity does, and deliver it.
async fn publish<E: DomainEvent>(kit: &Kit, event: &E) {
    let mut tx = kit.state.db.begin().await.unwrap();
    kit.state.events.publish(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
    let stats = Worker::new(kit.state.clone()).run_until_idle().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspension_is_tracked_once_in_each_workspace(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let one = kit.create_workspace(&ana, "Shop one").await;
    let two = kit.create_workspace(&ana, "Shop two").await;
    let ana_id = kit.user_id(&account.to_string()).await.to_string();
    let now = Utc::now();
    let suspended = AccountSuspended {
        account,
        at: Some(now),
        seq: Some(1),
    };

    publish(&kit, &suspended).await;
    // The same change again (a replay): announced again, tracked once.
    publish(&kit, &suspended).await;
    let events = tracked(&kit, "workspaces.member_suspended").await;
    let mut workspaces: Vec<_> = events.iter().map(|e| e.workspace_id).collect();
    workspaces.sort();
    let mut expected = vec![Some(*one.as_uuid()), Some(*two.as_uuid())];
    expected.sort();
    assert_eq!(workspaces, expected);
    for event in &events {
        assert_eq!(event.version, 1);
        assert_eq!(
            event.envelope["actor"],
            json!({"type": "system", "id": "workspaces.announce_member_suspended"})
        );
        assert_eq!(event.body["member"]["user_id"], ana_id);
        assert_eq!(event.body["member"]["suspended"], true);
        assert_eq!(event.body["member"]["role"], "owner");
        nothing_personal(event);
    }

    publish(
        &kit,
        &AccountUnsuspended {
            account,
            at: now,
            seq: Some(2),
        },
    )
    .await;
    let lifted = tracked(&kit, "workspaces.member_unsuspended").await;
    assert_eq!(lifted.len(), 2);
    for event in &lifted {
        assert_eq!(
            event.envelope["actor"],
            json!({"type": "system", "id": "workspaces.announce_member_unsuspended"})
        );
        assert_eq!(event.body["member"]["suspended"], false);
    }
    // The suspension it lifted, delivered late: older, nothing tracked.
    publish(&kit, &suspended).await;
    assert_eq!(tracked(&kit, "workspaces.member_suspended").await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_tracking_off_nothing_is_written(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    kit.create_workspace(&ana, "Shop").await;
    let any: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.tracked_events")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(any, 0);
}
