//! A calendar stream, against a fake Google Calendar (wiremock, invented
//! data): the grant lists the account's calendars, the first read and the
//! reads by sync token announce batches, members' changes are made with
//! `If-Match`, and a provider that refuses the calendar API marks the
//! calendar stream only.

use campbooks_kernel::{
    blobs::BlobRef,
    db::BeginFor as _,
    outbound::StoredCalendarChange,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{header, method, path, query_param, query_param_is_missing},
};

use crate::{
    domain::model::ConnectionId,
    events::OutboundRequested,
    testkit::{ACCOUNT_SUB, Caller, Kit, assert_contract},
};

const CALENDAR: &str = "sam@example.com";
const EVENTS: &str = "/calendar/v3/calendars/sam@example.com/events";
const CHANGE_TYPE: &str = "application/vnd.campbooks.calendar-change+json; version=1";

/// Google's `calendarList` with the member's one calendar.
async fn calendar_list(kit: &Kit) {
    kit.google
        .mount_first(
            Mock::given(method("GET"))
                .and(path("/calendar/v3/users/me/calendarList"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "items": [{
                        "id": CALENDAR, "summary": "Sam", "primary": true,
                        "accessRole": "owner", "timeZone": "Europe/Lisbon",
                        "backgroundColor": "#0b8043"
                    }]
                }))),
        )
        .await;
}

/// A timed event the fake calendar holds.
fn timed(id: &str, etag: &str, title: &str) -> Value {
    json!({
        "id": id, "etag": etag, "status": "confirmed", "summary": title,
        "start": {"dateTime": "2026-11-05T15:00:00+00:00", "timeZone": "Europe/Lisbon"},
        "end": {"dateTime": "2026-11-05T16:00:00+00:00", "timeZone": "Europe/Lisbon"}
    })
}

/// `items` as the answer of `events.list` when no sync token is sent.
async fn first_read(kit: &Kit, items: &[Value], token: &str, times: Option<u64>) {
    let mock = Mock::given(method("GET"))
        .and(path(EVENTS))
        .and(query_param_is_missing("syncToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": items, "nextSyncToken": token
        })));
    let mock = match times {
        Some(n) => mock.up_to_n_times(n),
        None => mock,
    };
    kit.google.mount_first(mock).await;
}

/// `items` as the answer of `events.list` from sync token `from`.
async fn changes_since(kit: &Kit, from: &str, items: &[Value], token: &str) {
    kit.google
        .mount_first(
            Mock::given(method("GET"))
                .and(path(EVENTS))
                .and(query_param("syncToken", from))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "items": items, "nextSyncToken": token
                }))),
        )
        .await;
}

fn member(kit: &Kit) -> (Caller, WorkspaceId) {
    let ws = WorkspaceId::new();
    let sam = kit.user("sam");
    kit.join(&sam, ws, Role::Member);
    (sam, ws)
}

/// The batches announced for calendars in `ws`, as documents.
async fn batches(kit: &Kit, ws: WorkspaceId) -> Vec<Value> {
    let mut documents = Vec::new();
    for payload in kit.payloads(ws, "connectors.calendar_changed").await {
        assert_contract("connectors.calendar_changed.v1.schema.json", &payload);
        let blob: BlobRef = serde_json::from_value(payload["batch"].clone()).unwrap();
        let document: Value = serde_json::from_slice(&kit.blob(ws, &blob).await).unwrap();
        assert_contract("calendar-change-batch.v1.schema.json", &document);
        documents.push(document);
    }
    documents
}

/// The calendar stream's row, as JSON.
async fn stream(kit: &Kit) -> Value {
    sqlx::query_scalar("SELECT to_jsonb(s) FROM connectors.streams s WHERE capability = 'calendar'")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

async fn sync_token(kit: &Kit) -> Option<String> {
    sqlx::query_scalar("SELECT sync_token FROM connectors.calendars")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_calendar_grant_lists_the_calendars_and_the_stream_reads_them(db: PgPool) {
    let kit = Kit::new(db).await;
    let (sam, ws) = member(&kit);
    calendar_list(&kit).await;
    first_read(
        &kit,
        &[
            timed("e1", "\"1\"", "Dentist"),
            timed("e2", "\"1\"", "Lunch"),
        ],
        "t1",
        Some(1),
    )
    .await;
    // A token Google no longer accepts: the calendar is read whole.
    kit.google
        .mount_first(
            Mock::given(method("GET"))
                .and(path(EVENTS))
                .and(query_param("syncToken", "t1"))
                .respond_with(ResponseTemplate::new(410).set_body_json(json!({
                    "error": {"errors": [{"reason": "fullSyncRequired"}]}
                }))),
        )
        .await;
    first_read(&kit, &[timed("e1", "\"1\"", "Dentist")], "t3", None).await;
    changes_since(
        &kit,
        "t3",
        &[json!({"id": "e1", "status": "cancelled"})],
        "t4",
    )
    .await;

    kit.adopt_calendar(&sam, ws, ACCOUNT_SUB).await.unwrap();
    kit.run_jobs().await;

    // The grant: the calendar list is a blob the event points at.
    let granted = kit.payloads(ws, "connectors.capability_granted").await;
    let calendar = granted
        .iter()
        .find(|event| event["capability"] == "calendar")
        .expect("a calendar grant");
    assert_contract("connectors.capability_granted.v1.schema.json", calendar);
    assert!(!calendar.to_string().contains(CALENDAR), "{calendar}");
    let list: BlobRef = serde_json::from_value(calendar["calendars"].clone()).unwrap();
    let list: Value = serde_json::from_slice(&kit.blob(ws, &list).await).unwrap();
    assert_contract("calendar-list.v1.schema.json", &list);
    assert_eq!(list["provider"], "google");
    assert_eq!(list["calendars"][0]["provider_id"], CALENDAR);
    assert_eq!(list["calendars"][0]["primary"], true);
    assert_eq!(list["calendars"][0]["writable"], true);

    // The first read: the whole calendar, with the sync token kept.
    let first = batches(&kit, ws).await;
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0]["full"], true);
    assert_eq!(first[0]["provider_calendar_id"], CALENDAR);
    assert_eq!(first[0]["upserts"].as_array().unwrap().len(), 2);
    assert_eq!(sync_token(&kit).await.as_deref(), Some("t1"));
    // The connection and its mail are as they were.
    let connections = kit
        .get(&sam, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json;
    assert_eq!(
        connections["connections"][0]["health"], "ok",
        "{connections}"
    );

    // Later: Google forgets the token, so the calendar is read in full, and
    // what is gone from it is for the schedule module to see.
    kit.tick(TimeDelta::minutes(2)).await;
    let second = batches(&kit, ws).await;
    assert_eq!(second.len(), 2, "{second:?}");
    assert_eq!(second[1]["full"], true);
    assert_eq!(second[1]["upserts"].as_array().unwrap().len(), 1);
    assert_eq!(sync_token(&kit).await.as_deref(), Some("t3"));

    // Then by token: a cancelled event is a removal.
    kit.tick(TimeDelta::minutes(2)).await;
    let third = batches(&kit, ws).await;
    assert_eq!(third.len(), 3, "{third:?}");
    assert_eq!(third[2]["full"], Value::Null);
    assert_eq!(third[2]["removals"][0]["provider_id"], "e1");
    assert_eq!(sync_token(&kit).await.as_deref(), Some("t4"));
}

/// Ask for `change` to be made, as the schedule module would after it
/// stored the request; the request's id.
async fn ask(kit: &Kit, ws: WorkspaceId, connection: Uuid, document: Value, bound: bool) -> Uuid {
    let request = Uuid::now_v7();
    let (calendar, event) = (Uuid::now_v7(), Uuid::now_v7());
    let bytes = serde_json::to_vec(&document).unwrap();
    let body = kit
        .state
        .blobs
        .put(&ws.to_string(), bytes, CHANGE_TYPE)
        .await
        .unwrap();
    if bound {
        kit.calendar.release(
            request,
            StoredCalendarChange {
                calendar,
                event,
                connection,
                body: body.clone(),
            },
        );
    }
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &OutboundRequested {
                workspace: ws.to_string(),
                request,
                calendar,
                event,
                connection: ConnectionId::from_uuid(connection),
                change: document["change"].as_str().unwrap().to_owned(),
                body,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    request
}

/// A connected member whose calendar holds event `e1`, and the id of the
/// connection.
async fn connected(kit: &Kit) -> (WorkspaceId, Uuid) {
    let (sam, ws) = member(kit);
    calendar_list(kit).await;
    first_read(kit, &[timed("e1", "\"1\"", "Dentist")], "t1", None).await;
    kit.adopt_calendar(&sam, ws, ACCOUNT_SUB).await.unwrap();
    kit.run_jobs().await;
    let connection: Uuid = sqlx::query_scalar("SELECT id FROM connectors.connections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    (ws, connection)
}

fn update(etag: &str) -> Value {
    json!({
        "version": 1, "change": "update", "provider_calendar_id": CALENDAR,
        "provider_event_id": "e1", "etag": etag,
        "event": {"title": "Dentist", "start": {"date_time": "2026-11-05T16:00:00+00:00",
                                                 "time_zone": "Europe/Lisbon"},
                  "end": {"date_time": "2026-11-05T17:00:00+00:00", "time_zone": "Europe/Lisbon"}}
    })
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_reaches_google_with_the_etag_and_is_not_echoed_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, connection) = connected(&kit).await;
    kit.google
        .mount_first(
            Mock::given(method("PATCH"))
                .and(path(format!("{EVENTS}/e1")))
                .and(header("if-match", "\"1\""))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": "e1", "etag": "\"2\""
                }))),
        )
        .await;
    // Google reports its own write, with the etag it answered.
    changes_since(&kit, "t1", &[timed("e1", "\"2\"", "Dentist")], "t2").await;
    let before = batches(&kit, ws).await.len();

    let request = ask(&kit, ws, connection, update("\"1\""), true).await;
    kit.run_jobs().await;

    let done = kit
        .payloads(ws, "connectors.calendar_change_completed")
        .await;
    assert_eq!(done.len(), 1, "{done:?}");
    assert_contract(
        "connectors.calendar_change_completed.v1.schema.json",
        &done[0],
    );
    assert_eq!(done[0]["request"], request.to_string());
    assert_eq!(done[0]["outcome"], "applied");
    assert_eq!(done[0]["provider_event_id"], "e1");
    assert_eq!(done[0]["etag"], "\"2\"");
    // Nothing came back as a change.
    assert_eq!(batches(&kit, ws).await.len(), before);
    assert_eq!(sync_token(&kit).await.as_deref(), Some("t2"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_made_at_google_first_is_superseded_and_googles_version_arrives(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, connection) = connected(&kit).await;
    kit.google
        .mount_first(
            Mock::given(method("PATCH"))
                .and(path(format!("{EVENTS}/e1")))
                .respond_with(ResponseTemplate::new(412).set_body_json(json!({
                    "error": {"errors": [{"reason": "conditionNotMet"}]}
                }))),
        )
        .await;
    changes_since(&kit, "t1", &[timed("e1", "\"3\"", "Dentist, moved")], "t2").await;
    let before = batches(&kit, ws).await.len();

    ask(&kit, ws, connection, update("\"1\""), true).await;
    kit.run_jobs().await;

    let done = kit
        .payloads(ws, "connectors.calendar_change_completed")
        .await;
    assert_eq!(done.len(), 1, "{done:?}");
    assert_contract(
        "connectors.calendar_change_completed.v1.schema.json",
        &done[0],
    );
    assert_eq!(done[0]["outcome"], "superseded");
    assert_eq!(done[0]["reason"], "changed_at_provider");
    // Google's version is what the next batch carries.
    let after = batches(&kit, ws).await;
    assert_eq!(after.len(), before + 1);
    assert_eq!(after[before]["upserts"][0]["etag"], "\"3\"");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_nobody_asked_for_is_never_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, connection) = connected(&kit).await;

    // The schedule module does not confirm it: nothing is recorded, and the
    // schedule module is told, so its event does not wait.
    ask(&kit, ws, connection, update("\"1\""), false).await;
    kit.run_jobs().await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.calendar_changes")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let done = kit
        .payloads(ws, "connectors.calendar_change_completed")
        .await;
    assert_eq!(done[0]["outcome"], "skipped");
    assert_eq!(done[0]["reason"], "unverified");

    // A row planted with SQL carries no binding that verifies.
    let stream: Uuid = stream(&kit).await["id"].as_str().unwrap().parse().unwrap();
    let body = kit
        .state
        .blobs
        .put(
            &ws.to_string(),
            serde_json::to_vec(&update("\"1\"")).unwrap(),
            CHANGE_TYPE,
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO connectors.calendar_changes
             (id, workspace_id, stream_id, request_id, calendar_id, event_id, change, body, mac)
         VALUES ($1, $2, $3, $4, $5, $6, 'update', $7, $8)",
    )
    .bind(Uuid::now_v7())
    .bind(ws.as_uuid())
    .bind(stream)
    .bind(Uuid::now_v7())
    .bind(Uuid::now_v7())
    .bind(Uuid::now_v7())
    .bind(sqlx::types::Json(&body))
    .bind(vec![1_u8; 33])
    .execute(&kit.db)
    .await
    .unwrap();
    kit.tick(TimeDelta::minutes(2)).await;
    let state: String = sqlx::query_scalar(
        "SELECT state || ':' || coalesce(reason, '') FROM connectors.calendar_changes",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(state, "skipped:unverified");
    let writes: Vec<String> = kit
        .google
        .received()
        .await
        .into_iter()
        .filter(|call| call.starts_with("PATCH ") || call.starts_with("POST /calendar"))
        .collect();
    assert!(writes.is_empty(), "{writes:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn google_refusing_the_calendar_api_marks_the_calendar_stream_only(db: PgPool) {
    let kit = Kit::new(db).await;
    let (sam, ws) = member(&kit);
    kit.google
        .mount_first(
            Mock::given(method("GET"))
                .and(path("/calendar/v3/users/me/calendarList"))
                .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                    "error": {"code": 403, "errors": [{"reason": "accessNotConfigured"}]}
                }))),
        )
        .await;
    kit.adopt_calendar(&sam, ws, ACCOUNT_SUB).await.unwrap();
    kit.run_jobs().await;

    let connections = kit
        .get(&sam, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json;
    assert_eq!(
        connections["connections"][0]["health"], "ok",
        "{connections}"
    );
    let streams = connections["connections"][0]["streams"].as_array().unwrap();
    let calendar = streams
        .iter()
        .find(|s| s["capability"] == "calendar")
        .expect("a calendar stream");
    assert_eq!(calendar["health_reason"], "calendar_api_not_enabled");
    let mail = streams
        .iter()
        .find(|s| s["capability"] == "messages")
        .expect("a mail stream");
    assert!(mail["health_reason"].is_null(), "{mail}");
    assert!(!mail["paused"].as_bool().unwrap());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn zoho_calendar_is_not_offered_while_its_flag_is_off(db: PgPool) {
    let kit = Kit::new(db).await;
    let (sam, ws) = member(&kit);
    let refused = kit
        .post(
            &sam,
            &format!("/api/v1/w/{ws}/connections/authorizations"),
            json!({"provider": "zoho", "capabilities": ["messages", "calendar"]}),
        )
        .await;
    assert_eq!(refused.status.as_u16(), 409, "{:?}", refused.json);
    assert!(kit.door.asked().is_empty());

    // Google's calendar is asked for with its two scopes; Zoho's scopes
    // only while the flag is on.
    let intake = crate::grant_intake(kit.deps.clone());
    let both = ["messages".to_owned(), "calendar".to_owned()];
    let scopes = intake.capability_scopes("google", &both);
    assert!(scopes.contains(&crate::google::CALENDAR_EVENTS_SCOPE.to_owned()));
    assert!(scopes.contains(&crate::google::CALENDAR_LIST_SCOPE.to_owned()));
    assert!(intake.capability_scopes("zoho", &both).is_empty());
    assert!(
        intake
            .capability_scopes("google", &["messages".to_owned()])
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn turning_a_calendar_on_reads_it_again_from_the_start(db: PgPool) {
    let kit = Kit::new(db).await;
    // As `connected`, but its first read answers once: the full read after
    // the calendar is turned on again gets the answer mounted below.
    let (sam, ws) = member(&kit);
    calendar_list(&kit).await;
    first_read(&kit, &[timed("e1", "\"1\"", "Dentist")], "t1", Some(1)).await;
    kit.adopt_calendar(&sam, ws, ACCOUNT_SUB).await.unwrap();
    kit.run_jobs().await;
    let connection: Uuid = sqlx::query_scalar("SELECT id FROM connectors.connections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let reference = kit
        .state
        .blobs
        .put(
            &ws.to_string(),
            serde_json::to_vec(&json!({"version": 1, "provider_calendar_id": CALENDAR})).unwrap(),
            "application/vnd.campbooks.calendar-ref+json; version=1",
        )
        .await
        .unwrap();
    let schedule_calendar = Uuid::now_v7();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    for syncing in [false, true] {
        kit.state
            .events
            .publish(
                &mut tx,
                &crate::events::CalendarSyncChanged {
                    workspace: ws.to_string(),
                    calendar: schedule_calendar,
                    connection: ConnectionId::from_uuid(connection),
                    syncing,
                    provider_calendar: Some(reference.clone()),
                },
            )
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    // Off, then on: the token is dropped and the calendar read whole.
    first_read(&kit, &[timed("e1", "\"1\"", "Dentist")], "t9", None).await;
    kit.run_jobs().await;
    let row: (Option<Uuid>, bool, Option<String>) =
        sqlx::query_as("SELECT schedule_calendar, syncing, sync_token FROM connectors.calendars")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(row.0, Some(schedule_calendar));
    assert!(row.1);
    assert_eq!(row.2.as_deref(), Some("t9"));

    // Deliveries come in any order: one older than the last the calendar
    // heard (here, every one) changes nothing.
    sqlx::query(
        "UPDATE connectors.calendars SET sync_changed_by = 'ffffffff-ffff-7fff-bfff-ffffffffffff'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::CalendarSyncChanged {
                workspace: ws.to_string(),
                calendar: schedule_calendar,
                connection: ConnectionId::from_uuid(connection),
                syncing: false,
                provider_calendar: Some(reference.clone()),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let after: (bool, Option<String>) =
        sqlx::query_as("SELECT syncing, sync_token FROM connectors.calendars")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(after, (true, Some("t9".to_owned())));
}
