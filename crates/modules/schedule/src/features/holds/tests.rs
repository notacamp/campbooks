//! The `Holds` port: a held block on the member's Campbooks calendar or on a
//! provider calendar (asked of it in the same transaction), never on another
//! member's or a read-only one, and taken off again.

use campbooks_kernel::{
    Error, RecordRef,
    db::BeginFor as _,
    schedule::HoldRequest,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use super::holds;
use crate::{
    inbound::{CalendarChangeCompleted, CapabilityGranted, Outcome},
    model::{CalendarId, ConnectionId, RequestId},
    testkit::Kit,
};

const TASK: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01";

fn wednesday_ten() -> DateTime<Utc> {
    "2026-11-11T10:00:00Z".parse().unwrap()
}

fn request(member: UserId, calendar: Option<CalendarId>) -> HoldRequest<'static> {
    HoldRequest {
        member,
        task: RecordRef::new("tasks.task", TASK),
        title: "Send the signed lease",
        starts: wednesday_ten(),
        ends: wednesday_ten() + TimeDelta::minutes(30),
        calendar: calendar.map(|id| *id.as_uuid()),
    }
}

/// Grant `member` a Google account's primary calendar; its id.
async fn google(kit: &Kit, workspace: WorkspaceId, member: UserId) -> CalendarId {
    let list = kit
        .blob(
            workspace,
            &json!({
                "provider": "google",
                "calendars": [
                    { "provider_id": "ana@example.com", "name": "Work",
                      "time_zone": "Europe/Lisbon", "primary": true, "writable": true },
                ],
            }),
        )
        .await;
    kit.publish(
        workspace,
        &CapabilityGranted {
            workspace: workspace.to_string(),
            connection: ConnectionId::new(),
            capability: "calendar".into(),
            granted_by: member,
            calendars: Some(list),
        },
    )
    .await;
    sqlx::query_scalar("SELECT id FROM schedule.calendars WHERE member_id = $1 AND is_primary")
        .bind(member)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

async fn events(kit: &Kit, name: &str) -> Vec<Value> {
    sqlx::query_scalar("SELECT payload FROM kernel.events WHERE name = $1 ORDER BY id")
        .bind(name)
        .fetch_all(&kit.db)
        .await
        .unwrap()
}

/// What a test reads of an event.
#[derive(sqlx::FromRow)]
struct Row {
    kind: String,
    busy: bool,
    origin: String,
    source_type: Option<String>,
    source_id: Option<String>,
    deleted: bool,
    outbound_pending: Option<String>,
}

async fn row(kit: &Kit, id: Uuid) -> Row {
    sqlx::query_as(
        "SELECT kind, busy, origin, source_type, source_id, deleted_at IS NOT NULL AS deleted,
                outbound_pending
           FROM schedule.events WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_hold_is_a_busy_block_on_the_campbooks_calendar_and_goes_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let port = holds();

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let held = port
        .hold(&kit.state, &mut tx, ws, request(ana, None))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(!held.at_provider);
    let made = row(&kit, held.event).await;
    assert_eq!(
        (
            made.kind.as_str(),
            made.busy,
            made.origin.as_str(),
            made.deleted
        ),
        ("hold", true, "member", false)
    );
    assert_eq!(made.source_type.as_deref(), Some("tasks.task"));
    assert_eq!(made.source_id.as_deref(), Some(TASK));
    let created = events(&kit, "schedule.event_created").await;
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["kind"], "hold");
    assert_eq!(
        created[0]["source"],
        json!({"type": "tasks.task", "id": TASK})
    );
    assert!(!created[0].to_string().contains("signed lease"));

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    port.release(&kit.state, &mut tx, ws, ana, held.event)
        .await
        .unwrap();
    // Twice, or for an event that does not exist: nothing.
    port.release(&kit.state, &mut tx, ws, ana, held.event)
        .await
        .unwrap();
    port.release(&kit.state, &mut tx, ws, ana, Uuid::now_v7())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(row(&kit, held.event).await.deleted);
    assert_eq!(events(&kit, "schedule.event_cancelled").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_hold_on_google_is_asked_of_it_in_the_same_transaction(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let work = google(&kit, ws, ana).await;
    let port = holds();

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let held = port
        .hold(&kit.state, &mut tx, ws, request(ana, Some(work)))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(held.at_provider);
    assert_eq!(held.calendar, *work.as_uuid());
    assert_eq!(
        row(&kit, held.event).await.outbound_pending.as_deref(),
        Some("create")
    );
    let asked: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, change FROM schedule.change_requests WHERE event_id = $1 ORDER BY id",
    )
    .bind(held.event)
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, "create");
    assert_eq!(events(&kit, "schedule.outbound_requested").await.len(), 1);

    // Released before Google made it: deleted here at once, and at Google as
    // soon as it lands there.
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    port.release(&kit.state, &mut tx, ws, ana, held.event)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(row(&kit, held.event).await.deleted);
    kit.publish(
        ws,
        &CalendarChangeCompleted {
            workspace: ws.to_string(),
            request: RequestId::from(asked[0].0),
            calendar: work,
            event: held.event.into(),
            outcome: Outcome::Applied,
            provider_event_id: Some("hold@example.com".into()),
            etag: Some("etag-1".into()),
            reason: None,
        },
    )
    .await;
    let changes: Vec<String> = sqlx::query_scalar(
        "SELECT change FROM schedule.change_requests WHERE event_id = $1 ORDER BY id",
    )
    .bind(held.event)
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(changes, ["create", "delete"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_members_calendar_is_not_found_and_a_read_only_one_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let (_, joao) = kit.member("joao", ws);
    let theirs = google(&kit, ws, joao).await;
    let port = holds();

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let refused = port
        .hold(&kit.state, &mut tx, ws, request(ana, Some(theirs)))
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::NotFound), "{refused:?}");
    drop(tx);

    let mine = google(&kit, ws, ana).await;
    sqlx::query("UPDATE schedule.calendars SET writable = false WHERE id = $1")
        .bind(mine)
        .execute(&kit.db)
        .await
        .unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let refused = port
        .hold(&kit.state, &mut tx, ws, request(ana, Some(mine)))
        .await
        .unwrap_err();
    assert_eq!(refused.reason().code(), "calendar_read_only");
    drop(tx);
    let made: i64 = sqlx::query_scalar("SELECT count(*) FROM schedule.events WHERE kind = 'hold'")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(made, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_calendar_the_member_follows_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let marta = google(&kit, ws, ana).await;
    sqlx::query("UPDATE schedule.calendars SET following = true WHERE id = $1")
        .bind(marta)
        .execute(&kit.db)
        .await
        .unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let refused = holds()
        .hold(&kit.state, &mut tx, ws, request(ana, Some(marta)))
        .await
        .unwrap_err();
    assert_eq!(refused.reason().code(), "calendar_followed");
    drop(tx);
    let made: i64 = sqlx::query_scalar("SELECT count(*) FROM schedule.events WHERE kind = 'hold'")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(made, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn release_leaves_alone_what_is_not_the_members_hold(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (ana_token, ana) = kit.member("ana", ws);
    let (_, joao) = kit.member("joao", ws);
    let port = holds();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    let held = port
        .hold(&kit.state, &mut tx, ws, request(ana, None))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let dentist = kit
        .post(
            &ana_token,
            &format!("/api/v1/w/{ws}/schedule/events"),
            Some(json!({"title": "Dentist", "starts": "2026-11-12T15:00:00+00:00"})),
        )
        .await;
    let dentist: Uuid = dentist.json["id"].as_str().unwrap().parse().unwrap();

    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    // Not Joao's, and not a hold.
    port.release(&kit.state, &mut tx, ws, joao, held.event)
        .await
        .unwrap();
    port.release(&kit.state, &mut tx, ws, ana, dentist)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(!row(&kit, held.event).await.deleted);
    assert!(!row(&kit, dentist).await.deleted);
    assert!(events(&kit, "schedule.event_cancelled").await.is_empty());
}
