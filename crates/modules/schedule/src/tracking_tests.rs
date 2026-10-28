//! The module's tracked events (ADR 0020, campbooks-cloud#349), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums, counts and times, and nothing for a change that
//! changes nothing. No event holds a title, a place, a calendar's name or a
//! provider's id: every such text in these tests says `Shop`, `Rua` or
//! `example`.

use axum::http::StatusCode;
use campbooks_kernel::{
    tenancy::{UserId, WorkspaceId},
    testing::{self, TrackedRow},
};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::{
    inbound::{
        CalendarChangeCompleted, CalendarChanged, CapabilityGranted, ConnectionRemoved,
        MessageRead, Outcome, ReadItem,
    },
    model::{CalendarId, ConnectionId, EventId, RequestId},
    testkit::Kit,
};

async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// No title, place, name, address or provider id anywhere in the body.
fn nothing_personal(event: &TrackedRow) {
    let body = event.body.to_string();
    for text in ["Shop", "Rua", "example", "@", "Europe", "#12"] {
        assert!(
            !body.contains(text),
            "{} holds personal data or content ({text}): {body}",
            event.name
        );
    }
}

/// `event` is version 1 of `name`, in `workspace`, from `sub`'s own request
/// to `operation`.
fn by_user(event: &TrackedRow, name: &str, workspace: WorkspaceId, sub: &str, operation: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "user", "id": sub}),
        "{name}"
    );
    assert_eq!(event.envelope["operation"], operation, "{name}");
    assert_eq!(
        event.envelope["workspace_id"],
        workspace.to_string(),
        "{name}"
    );
    assert_eq!(
        event.envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    nothing_personal(event);
}

/// `event` is version 1 of `name`, in `workspace`, tracked by the job or
/// handler `kind`.
fn by_system(event: &TrackedRow, name: &str, workspace: WorkspaceId, kind: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "system", "id": kind}),
        "{name}"
    );
    nothing_personal(event);
}

fn events_uri(workspace: WorkspaceId) -> String {
    format!("/api/v1/w/{workspace}/schedule/events")
}

/// Make an event on the member's Campbooks calendar; its id.
async fn make(kit: &Kit, token: &str, workspace: WorkspaceId) -> String {
    let made = kit
        .post(
            token,
            &events_uri(workspace),
            Some(json!({
                "title": "Dentist at Shop",
                "kind": "appointment",
                "starts": "2026-11-12T15:00:00+00:00",
                "place": "Rua Example 1",
                "notes": "Bring the Shop card",
            })),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    made.json["id"].as_str().unwrap().to_owned()
}

/// Grant `member` a Google account's calendars: a primary one and a second;
/// the connection.
async fn grant(kit: &Kit, workspace: WorkspaceId, member: UserId) -> ConnectionId {
    let list = kit
        .blob(
            workspace,
            &json!({
                "provider": "google",
                "calendars": [
                    { "provider_id": "ana@example.com", "name": "Shop main",
                      "color": "#0B8043", "time_zone": "Europe/Lisbon",
                      "primary": true, "writable": true },
                    { "provider_id": "family@group.example.com", "name": "Shop family",
                      "primary": false, "writable": true },
                ],
            }),
        )
        .await;
    let connection = ConnectionId::new();
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
    connection
}

/// The member's provider calendar that is (or is not) primary.
async fn provider_calendar(kit: &Kit, member: UserId, primary: bool) -> CalendarId {
    sqlx::query_scalar(
        "SELECT id FROM schedule.calendars
          WHERE member_id = $1 AND source = 'provider' AND is_primary = $2",
    )
    .bind(member)
    .bind(primary)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
#[allow(clippy::too_many_lines)]
async fn an_event_is_tracked_from_made_to_restored(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    let id = make(&kit, &ana, ws).await;

    let created = tracked(&kit, "schedule.calendar_event_created").await;
    assert_eq!(created.len(), 1);
    by_user(
        &created[0],
        "schedule.calendar_event_created",
        ws,
        "ana",
        "schedule.create_event",
    );
    let snapshot = &created[0].body["calendar_event"];
    assert_eq!(snapshot["id"], id);
    assert_eq!(snapshot["member_id"], ana_id.to_string());
    assert_eq!(snapshot["calendar_source"], "campbooks");
    assert_eq!(snapshot["kind"], "appointment");
    assert_eq!(snapshot["origin"], "member");
    assert_eq!(snapshot["status"], "confirmed");
    assert_eq!(snapshot["all_day"], false);
    assert_eq!(snapshot["starts_at"], "2026-11-12T15:00:00Z");
    assert_eq!(snapshot["ends_at"], "2026-11-12T16:00:00Z");
    assert_eq!(snapshot["busy"], true);
    assert_eq!(snapshot["recurring"], false);
    assert_eq!(snapshot["at_provider"], false);
    assert!(snapshot.get("provider").is_none());
    assert!(snapshot.get("title").is_none());

    // Moved: the times before, and nothing edited.
    let uri = format!("{}/{id}", events_uri(ws));
    let moved = kit
        .patch(&ana, &uri, json!({ "starts": "2026-11-12T17:00:00+00:00" }))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    let events = tracked(&kit, "schedule.calendar_event_moved").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_moved",
        ws,
        "ana",
        "schedule.update_event",
    );
    assert_eq!(
        events[0].body["before"],
        json!({
            "all_day": false,
            "starts_at": "2026-11-12T15:00:00Z",
            "ends_at": "2026-11-12T16:00:00Z",
        })
    );
    assert_eq!(
        events[0].body["calendar_event"]["starts_at"],
        "2026-11-12T17:00:00Z"
    );
    assert!(
        tracked(&kit, "schedule.calendar_event_edited")
            .await
            .is_empty()
    );

    // Edited: which fields, never their values; the kind before. The same
    // values again track nothing.
    for _ in 0..2 {
        let edited = kit
            .patch(
                &ana,
                &uri,
                json!({ "title": "Shop meeting", "kind": "meeting", "place": "Rua Two" }),
            )
            .await;
        assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.json);
    }
    let events = tracked(&kit, "schedule.calendar_event_edited").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_edited",
        ws,
        "ana",
        "schedule.update_event",
    );
    assert_eq!(events[0].body["fields"], json!(["title", "kind", "place"]));
    assert_eq!(events[0].body["before_kind"], "appointment");
    assert_eq!(events[0].body["calendar_event"]["kind"], "meeting");
    assert_eq!(
        tracked(&kit, "schedule.calendar_event_moved").await.len(),
        1
    );

    // Deleted, then restored with its undo token.
    let deleted = kit.delete(&ana, &uri).await;
    assert_eq!(deleted.status, StatusCode::OK, "{:?}", deleted.json);
    let events = tracked(&kit, "schedule.calendar_event_deleted").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_deleted",
        ws,
        "ana",
        "schedule.delete_event",
    );
    assert!(events[0].body["calendar_event"]["deleted_at"].is_string());
    let restored = kit
        .post(
            &ana,
            &format!("{uri}/restore"),
            Some(json!({ "undo_token": deleted.json["undo_token"] })),
        )
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json);
    let events = tracked(&kit, "schedule.calendar_event_restored").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_restored",
        ws,
        "ana",
        "schedule.restore_event",
    );
    assert_eq!(events[0].body["remade_at_provider"], false);
    assert!(events[0].body["calendar_event"].get("deleted_at").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_answer_is_tracked_with_the_one_before(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, _) = kit.member("ana", ws);
    let id = make(&kit, &ana, ws).await;
    let uri = format!("{}/{id}/response", events_uri(ws));
    for response in ["accepted", "accepted", "declined"] {
        let answered = kit
            .post(&ana, &uri, Some(json!({ "response": response })))
            .await;
        assert_eq!(answered.status, StatusCode::OK, "{:?}", answered.json);
    }
    let events = tracked(&kit, "schedule.calendar_event_responded").await;
    assert_eq!(events.len(), 2, "the same answer again tracks nothing");
    for event in &events {
        by_user(
            event,
            "schedule.calendar_event_responded",
            ws,
            "ana",
            "schedule.respond_event",
        );
    }
    assert_eq!(events[0].body["before"], "needs_action");
    assert_eq!(events[0].body["calendar_event"]["response"], "accepted");
    assert_eq!(events[1].body["before"], "accepted");
    assert_eq!(events[1].body["calendar_event"]["response"], "declined");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn settings_name_what_changed_never_the_values(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    let (bea, _) = kit.member("bea", ws);
    let uri = format!("/api/v1/w/{ws}/schedule/settings");

    let set = kit
        .patch(&ana, &uri, json!({ "time_zone": "Europe/Lisbon" }))
        .await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.json);
    // The device's zone once one is set changes nothing.
    let ignored = kit
        .patch(&ana, &uri, json!({ "device_time_zone": "Europe/Paris" }))
        .await;
    assert_eq!(ignored.status, StatusCode::OK);
    let hours = kit
        .patch(
            &ana,
            &uri,
            json!({
                "working_hours": { "mon": [{ "start": "10:00", "end": "16:00" }] },
                "scout_kinds": ["travel", "stay"],
            }),
        )
        .await;
    assert_eq!(hours.status, StatusCode::OK, "{:?}", hours.json);
    let device = kit
        .patch(&bea, &uri, json!({ "device_time_zone": "Europe/Paris" }))
        .await;
    assert_eq!(device.status, StatusCode::OK);
    let read = kit.get(&bea, &uri).await;
    assert_eq!(read.json["time_zone"], "Europe/Paris");

    let events = tracked(&kit, "schedule.settings_updated").await;
    assert_eq!(events.len(), 3);
    by_user(
        &events[0],
        "schedule.settings_updated",
        ws,
        "ana",
        "schedule.update_settings",
    );
    assert_eq!(events[0].body["fields"], json!(["time_zone"]));
    let settings = &events[0].body["settings"];
    assert_eq!(settings["member_id"], ana_id.to_string());
    assert_eq!(settings["time_zone_set"], true);
    assert_eq!(settings["working_hours_set"], false);
    assert_eq!(settings["working_days"], 5);
    assert_eq!(
        events[1].body["fields"],
        json!(["working_hours", "scout_kinds"])
    );
    assert_eq!(events[1].body["settings"]["working_days"], 1);
    assert_eq!(events[1].body["settings"]["working_hours_set"], true);
    assert_eq!(
        events[1].body["settings"]["scout_kinds"],
        json!(["travel", "stay"])
    );
    by_user(
        &events[2],
        "schedule.settings_updated",
        ws,
        "bea",
        "schedule.update_settings",
    );
    assert_eq!(events[2].body["fields"], json!(["device_time_zone"]));
}

#[sqlx::test(migrations = "../../../migrations")]
#[allow(clippy::too_many_lines)]
async fn calendars_are_tracked_from_grant_to_removal(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana_id).await;

    let added = tracked(&kit, "schedule.calendar_added").await;
    assert_eq!(added.len(), 2);
    for event in &added {
        by_system(
            event,
            "schedule.calendar_added",
            ws,
            "schedule.grant_calendars",
        );
        let calendar = &event.body["calendar"];
        assert_eq!(calendar["member_id"], ana_id.to_string());
        assert_eq!(calendar["source"], "provider");
        assert_eq!(calendar["provider"], "google");
        assert_eq!(calendar["connection_id"], connection.to_string());
        assert_eq!(calendar["writable"], true);
        assert_eq!(calendar["syncing"], calendar["primary"]);
    }
    // A repeated grant adds nothing.
    grant_again(&kit, ws, ana_id, connection).await;
    assert_eq!(tracked(&kit, "schedule.calendar_added").await.len(), 2);

    // On (twice: the second changes nothing), recoloured, off.
    let family = provider_calendar(&kit, ana_id, false).await;
    let uri = format!("/api/v1/w/{ws}/schedule/calendars/{family}");
    for patch in [
        json!({ "syncing": true }),
        json!({ "syncing": true }),
        json!({ "color": "#123456" }),
        json!({ "role": "following" }),
        json!({ "role": "following" }),
        json!({ "syncing": false }),
    ] {
        let changed = kit.patch(&ana, &uri, patch).await;
        assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.json);
    }
    let enabled = tracked(&kit, "schedule.calendar_enabled").await;
    assert_eq!(enabled.len(), 1);
    by_user(
        &enabled[0],
        "schedule.calendar_enabled",
        ws,
        "ana",
        "schedule.update_calendar",
    );
    assert_eq!(enabled[0].body["calendar"]["id"], family.to_string());
    assert_eq!(enabled[0].body["calendar"]["syncing"], true);
    let recoloured = tracked(&kit, "schedule.calendar_recoloured").await;
    assert_eq!(recoloured.len(), 1);
    by_user(
        &recoloured[0],
        "schedule.calendar_recoloured",
        ws,
        "ana",
        "schedule.update_calendar",
    );
    let role = tracked(&kit, "schedule.calendar_role_changed").await;
    assert_eq!(role.len(), 1);
    by_user(
        &role[0],
        "schedule.calendar_role_changed",
        ws,
        "ana",
        "schedule.update_calendar",
    );
    assert_eq!(role[0].body["role"], "following");
    let disabled = tracked(&kit, "schedule.calendar_disabled").await;
    assert_eq!(disabled.len(), 1);
    by_user(
        &disabled[0],
        "schedule.calendar_disabled",
        ws,
        "ana",
        "schedule.update_calendar",
    );
    assert_eq!(disabled[0].body["calendar"]["syncing"], false);

    // The connection removed: each calendar, as it stood, with its events.
    kit.publish(
        ws,
        &ConnectionRemoved {
            workspace: ws.to_string(),
            connection,
        },
    )
    .await;
    let removed = tracked(&kit, "schedule.calendar_removed").await;
    assert_eq!(removed.len(), 2);
    for event in &removed {
        by_system(
            event,
            "schedule.calendar_removed",
            ws,
            "schedule.forget_connection",
        );
        assert_eq!(event.body["calendar_events"], 0);
        assert_eq!(
            event.body["calendar"]["connection_id"],
            connection.to_string()
        );
    }
}

/// The same grant as [`grant`], once more.
async fn grant_again(kit: &Kit, workspace: WorkspaceId, member: UserId, connection: ConnectionId) {
    let list = kit
        .blob(
            workspace,
            &json!({
                "provider": "google",
                "calendars": [
                    { "provider_id": "ana@example.com", "name": "Shop main",
                      "primary": true, "writable": true },
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

/// An upsert of a batch: provider event `id` at `starts` for an hour.
fn upsert(id: &str, starts: &str, ends: &str) -> Value {
    json!({
        "provider_id": id,
        "etag": "etag-1",
        "status": "confirmed",
        "title": "Shop lunch",
        "place": "Rua Example",
        "start": { "date_time": starts },
        "end": { "date_time": ends },
    })
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_provider_batch_is_one_summary(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (_, ana_id) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana_id).await;
    let primary = provider_calendar(&kit, ana_id, true).await;

    let first = json!({
        "version": 1,
        "provider_calendar_id": "ana@example.com",
        "upserts": [
            upsert("p1@example.com", "2026-11-10T12:00:00+00:00", "2026-11-10T13:00:00+00:00"),
            upsert("p2@example.com", "2026-11-11T12:00:00+00:00", "2026-11-11T13:00:00+00:00"),
        ],
        "removals": [],
    });
    let removal = json!({
        "version": 1,
        "provider_calendar_id": "ana@example.com",
        "upserts": [],
        "removals": [{ "provider_id": "p1@example.com" }],
    });
    // The same batch twice: the second changes nothing.
    for batch in [&first, &first, &removal] {
        let batch = kit.blob(ws, batch).await;
        kit.publish(
            ws,
            &CalendarChanged {
                workspace: ws.to_string(),
                connection,
                batch,
            },
        )
        .await;
    }
    let events = tracked(&kit, "schedule.provider_changes_applied").await;
    assert_eq!(events.len(), 2);
    for event in &events {
        by_system(
            event,
            "schedule.provider_changes_applied",
            ws,
            "schedule.apply_calendar_changes",
        );
        assert_eq!(event.body["calendar"]["id"], primary.to_string());
        assert_eq!(event.body["full"], false);
    }
    assert_eq!(events[0].body["created"], 2);
    assert_eq!(events[0].body["changed"], 0);
    assert_eq!(events[0].body["cancelled"], 0);
    assert_eq!(events[1].body["created"], 0);
    assert_eq!(events[1].body["cancelled"], 1);
    // The provider's events are counted, never one event each.
    assert!(
        tracked(&kit, "schedule.calendar_event_created")
            .await
            .is_empty()
    );
}

fn read(workspace: WorkspaceId, owner: UserId, message: Uuid, item: ReadItem) -> MessageRead {
    MessageRead {
        workspace: workspace.to_string(),
        message: message.to_string(),
        conversation: Uuid::now_v7().to_string(),
        owner: Some(owner),
        layer: "structured".into(),
        items: vec![item],
        texts: None,
    }
}

fn item(key: &str, starts: &str, ends: Option<&str>) -> ReadItem {
    ReadItem {
        key: key.into(),
        kind: "travel".into(),
        starts: starts.into(),
        ends: ends.map(Into::into),
        all_day: false,
        time_zone: None,
        updates: None,
        cancels: None,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
#[allow(clippy::too_many_lines)]
async fn what_scout_reads_is_tracked_as_the_system(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (_, ana_id) = kit.member("ana", ws);
    let message = Uuid::now_v7();

    // A flight, with its title and place in the texts blob.
    let texts = kit
        .blob(
            ws,
            &json!({ "items": [{ "key": "k1", "title": "Shop flight", "place": "Rua Airport" }] }),
        )
        .await;
    let mut first = read(
        ws,
        ana_id,
        message,
        item(
            "k1",
            "2026-11-06T07:40:00+00:00",
            Some("2026-11-06T10:55:00+00:00"),
        ),
    );
    first.texts = Some(texts);
    kit.publish(ws, &first).await;
    let created = tracked(&kit, "schedule.calendar_event_created").await;
    assert_eq!(created.len(), 1);
    by_system(
        &created[0],
        "schedule.calendar_event_created",
        ws,
        "schedule.add_read_items",
    );
    let snapshot = &created[0].body["calendar_event"];
    assert_eq!(snapshot["origin"], "scout");
    assert_eq!(snapshot["kind"], "travel");
    assert_eq!(snapshot["found_by"], "structured");
    assert_eq!(snapshot["source_message_id"], message.to_string());
    // The same read again adds nothing.
    kit.publish(ws, &first).await;
    assert_eq!(
        tracked(&kit, "schedule.calendar_event_created").await.len(),
        1
    );

    // A later email moves it.
    let mut later = item(
        "k2",
        "2026-11-06T09:00:00+00:00",
        Some("2026-11-06T12:00:00+00:00"),
    );
    later.updates = Some("k1".into());
    kit.publish(ws, &read(ws, ana_id, Uuid::now_v7(), later))
        .await;
    let moved = tracked(&kit, "schedule.calendar_event_moved").await;
    assert_eq!(moved.len(), 1);
    by_system(
        &moved[0],
        "schedule.calendar_event_moved",
        ws,
        "schedule.add_read_items",
    );
    assert_eq!(moved[0].body["before"]["starts_at"], "2026-11-06T07:40:00Z");
    assert_eq!(
        moved[0].body["calendar_event"]["starts_at"],
        "2026-11-06T09:00:00Z"
    );
    assert!(
        tracked(&kit, "schedule.calendar_event_edited")
            .await
            .is_empty()
    );

    // And a third cancels it, once.
    let mut cancelling = item("k3", "2026-11-06T09:00:00+00:00", None);
    cancelling.cancels = Some("k1".into());
    let cancel = read(ws, ana_id, Uuid::now_v7(), cancelling);
    kit.publish(ws, &cancel).await;
    kit.publish(ws, &cancel).await;
    let cancelled = tracked(&kit, "schedule.calendar_event_cancelled").await;
    assert_eq!(cancelled.len(), 1);
    by_system(
        &cancelled[0],
        "schedule.calendar_event_cancelled",
        ws,
        "schedule.add_read_items",
    );
    assert_eq!(cancelled[0].body["calendar_event"]["status"], "cancelled");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn adding_to_a_provider_calendar_and_taking_it_away(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    grant(&kit, ws, ana_id).await;
    let primary = provider_calendar(&kit, ana_id, true).await;
    let id = make(&kit, &ana, ws).await;

    let pushed = kit
        .post(
            &ana,
            &format!("{}/{id}/push", events_uri(ws)),
            Some(json!({ "calendar_id": primary })),
        )
        .await;
    assert_eq!(pushed.status, StatusCode::OK, "{:?}", pushed.json);
    let events = tracked(&kit, "schedule.calendar_event_pushed").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_pushed",
        ws,
        "ana",
        "schedule.push_event",
    );
    let copy = &events[0].body["calendar_event"];
    assert_eq!(copy["pushed_from_id"], id);
    assert_eq!(copy["calendar_id"], primary.to_string());
    assert_eq!(copy["calendar_source"], "provider");
    assert_eq!(copy["provider"], "google");
    assert_eq!(copy["pending_change"], "create");
    let copy_id: EventId = copy["id"].as_str().unwrap().parse().unwrap();

    // The provider made it (connectors' outcome, applied here).
    let request: Uuid =
        sqlx::query_scalar("SELECT id FROM schedule.change_requests WHERE event_id = $1")
            .bind(copy_id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    kit.publish(
        ws,
        &CalendarChangeCompleted {
            workspace: ws.to_string(),
            request: RequestId::from(request),
            calendar: primary,
            event: copy_id,
            outcome: Outcome::Applied,
            provider_event_id: Some("copy@example.com".into()),
            etag: Some("etag-2".into()),
            reason: None,
        },
    )
    .await;

    let unpushed = kit
        .delete(&ana, &format!("{}/{copy_id}/push", events_uri(ws)))
        .await;
    assert_eq!(unpushed.status, StatusCode::OK, "{:?}", unpushed.json);
    let events = tracked(&kit, "schedule.calendar_event_unpushed").await;
    assert_eq!(events.len(), 1);
    by_user(
        &events[0],
        "schedule.calendar_event_unpushed",
        ws,
        "ana",
        "schedule.unpush_event",
    );
    assert_eq!(events[0].body["calendar_event"]["id"], id);
    assert_eq!(events[0].body["copy_id"], copy_id.to_string());
    assert_eq!(events[0].body["copy_calendar_id"], primary.to_string());
    assert!(
        events[0].body["calendar_event"]
            .get("pushed_to_id")
            .is_none()
    );
}
