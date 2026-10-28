//! Calendar roles (campbooks-cloud#399): what a grant makes mine or
//! following, the role PATCH, and what free time counts.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    inbound::CapabilityGranted,
    model::{CalendarId, ConnectionId},
    testkit::Kit,
};

/// Grant `member` three calendars: "Ines" (primary, owned), "Family"
/// (owned) and "Holidays" (read-only, not owned); with `family_owned` as the
/// provider says it for "Family".
async fn grant(
    kit: &Kit,
    workspace: WorkspaceId,
    member: UserId,
    connection: ConnectionId,
    family_owned: Option<bool>,
) {
    let mut family = json!({ "provider_id": "family", "name": "Family",
        "primary": false, "writable": true });
    if let Some(owned) = family_owned {
        family["owned"] = json!(owned);
    }
    let list = kit
        .blob(
            workspace,
            &json!({
                "provider": "google",
                "calendars": [
                    { "provider_id": "ines", "name": "Ines", "primary": true,
                      "writable": true, "owned": true },
                    family,
                    { "provider_id": "holidays", "name": "Holidays", "primary": false,
                      "writable": false, "owned": false },
                ],
            }),
        )
        .await;
    kit.publish(
        workspace,
        &CapabilityGranted {
            workspace: workspace.to_string(),
            connection,
            capability: "calendar".into(),
            granted_by: member,
            calendars: Some(list),
        },
    )
    .await;
}

async fn calendar(kit: &Kit, member: UserId, name: &str) -> CalendarId {
    sqlx::query_scalar("SELECT id FROM schedule.calendars WHERE member_id = $1 AND name = $2")
        .bind(member)
        .bind(name)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

/// A calendar's `(syncing, following, owned)` as stored.
async fn stored(kit: &Kit, id: CalendarId) -> (bool, bool, Option<bool>) {
    sqlx::query_as("SELECT syncing, following, owned FROM schedule.calendars WHERE id = $1")
        .bind(id)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_makes_owned_calendars_mine_and_the_others_following(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ines) = kit.member("ines", ws);
    grant(&kit, ws, ines, ConnectionId::new(), None).await;

    let list = kit
        .get(&token, &format!("/api/v1/w/{ws}/schedule/calendars"))
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    let role = |name: &str| -> (Value, Value) {
        let calendar = list.json["calendars"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        (calendar["role"].clone(), calendar["owned"].clone())
    };
    // The primary is on and mine; the others are off, and would come back as
    // the provider's word says.
    assert_eq!(role("Ines"), (json!("mine"), json!(true)));
    assert_eq!(role("Family"), (json!("off"), Value::Null));
    assert_eq!(role("Holidays"), (json!("off"), json!(false)));
    assert_eq!(
        stored(&kit, calendar(&kit, ines, "Holidays").await).await,
        (false, true, Some(false))
    );
    assert_eq!(
        stored(&kit, calendar(&kit, ines, "Family").await).await,
        (false, false, None)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_repeated_grant_fills_owned_and_changes_nothing_else(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ines) = kit.member("ines", ws);
    let connection = ConnectionId::new();
    grant(&kit, ws, ines, connection, None).await;
    let family = calendar(&kit, ines, "Family").await;
    // The member made it theirs, then the provider says it is not owned.
    let patched = kit
        .patch(
            &token,
            &format!("/api/v1/w/{ws}/schedule/calendars/{family}"),
            json!({ "role": "mine" }),
        )
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{:?}", patched.json);
    grant(&kit, ws, ines, connection, Some(false)).await;
    assert_eq!(stored(&kit, family).await, (true, false, Some(false)));
    // Once known, the fact is not overwritten.
    grant(&kit, ws, ines, connection, Some(true)).await;
    assert_eq!(stored(&kit, family).await, (true, false, Some(false)));
    let calendars: i64 = sqlx::query_scalar("SELECT count(*) FROM schedule.calendars")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(calendars, 4);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_role_patch_moves_between_mine_following_and_off_and_refuses_nonsense(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (token, ines) = kit.member("ines", ws);
    grant(&kit, ws, ines, ConnectionId::new(), None).await;
    let family = calendar(&kit, ines, "Family").await;
    let uri = format!("/api/v1/w/{ws}/schedule/calendars/{family}");
    let patch = |body: Value| kit.patch(&token, &uri, body);

    let mine = patch(json!({ "role": "mine" })).await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.json);
    assert_eq!(mine.json["role"], "mine");
    assert_eq!(mine.json["syncing"], true);
    let following = patch(json!({ "role": "following" })).await;
    assert_eq!(following.json["role"], "following");
    assert_eq!(following.json["syncing"], true);
    // Off keeps whether it would come back mine or following.
    let off = patch(json!({ "role": "off" })).await;
    assert_eq!(off.json["role"], "off");
    assert_eq!(stored(&kit, family).await, (false, true, None));
    let on = patch(json!({ "syncing": true })).await;
    assert_eq!(on.json["role"], "following");

    // Sync changes go to connectors for off <-> on only: on, off, on again.
    let changes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events WHERE name = 'schedule.calendar_sync_changed'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(changes, 3);

    // A role and syncing that disagree.
    let disagree = patch(json!({ "role": "off", "syncing": true })).await;
    assert_eq!(disagree.status, StatusCode::BAD_REQUEST);
    let agree = patch(json!({ "role": "mine", "syncing": true })).await;
    assert_eq!(agree.status, StatusCode::OK);

    // The Campbooks calendar is always mine.
    let campbooks: CalendarId = sqlx::query_scalar(
        "SELECT id FROM schedule.calendars WHERE member_id = $1 AND source = 'campbooks'",
    )
    .bind(ines)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let own = format!("/api/v1/w/{ws}/schedule/calendars/{campbooks}");
    for body in [json!({ "role": "following" }), json!({ "role": "off" })] {
        let refused = kit.patch(&token, &own, body).await;
        assert_eq!(
            refused.status,
            StatusCode::BAD_REQUEST,
            "{:?}",
            refused.json
        );
    }
    let fine = kit.patch(&token, &own, json!({ "role": "mine" })).await;
    assert_eq!(fine.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn free_slots_do_not_count_a_calendar_you_follow(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ines) = kit.member("ines", ws);
    grant(&kit, ws, ines, ConnectionId::new(), None).await;
    let ines_calendar = calendar(&kit, ines, "Ines").await;
    let patch_uri = format!("/api/v1/w/{ws}/schedule/calendars/{ines_calendar}");
    let free =
        format!("/api/v1/w/{ws}/schedule/free-slots?from=2026-11-16&to=2026-11-17&minutes=60");
    let names = |answer: &Value| -> Vec<String> {
        answer["calendars"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_owned())
            .collect()
    };

    let mine = kit.get(&token, &free).await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.json);
    assert!(names(&mine.json).contains(&"Ines".to_owned()));
    kit.patch(&token, &patch_uri, json!({ "role": "following" }))
        .await;
    let following = kit.get(&token, &free).await;
    assert!(!names(&following.json).contains(&"Ines".to_owned()));
    kit.patch(&token, &patch_uri, json!({ "role": "mine" }))
        .await;
    let back = kit.get(&token, &free).await;
    assert!(names(&back.json).contains(&"Ines".to_owned()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_migration_backfill_follows_what_cannot_be_written(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ines) = kit.member("ines", ws);
    grant(&kit, ws, ines, ConnectionId::new(), None).await;
    // The same statement as the migration's backfill, run on rows that were
    // made before roles (everything not following).
    sqlx::query("UPDATE schedule.calendars SET following = false")
        .execute(&kit.db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE schedule.calendars SET following = true WHERE source = 'provider' AND NOT writable",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    assert!(stored(&kit, calendar(&kit, ines, "Holidays").await).await.1);
    assert!(!stored(&kit, calendar(&kit, ines, "Ines").await).await.1);
    // The Campbooks calendar cannot be following.
    let refused =
        sqlx::query("UPDATE schedule.calendars SET following = true WHERE source = 'campbooks'")
            .execute(&kit.db)
            .await;
    assert!(refused.is_err() || refused.unwrap().rows_affected() == 0);
}
