//! Invitations awaiting the member's answer (campbooks-cloud#360): announced
//! when a provider's batch brings one, closed when it is answered, cancelled,
//! hidden or removed, never for an event the member organised, an answer
//! that reaches the provider only after the undo window (and never when it
//! was taken back), and the invitations stored before announced once.

use axum::http::StatusCode;
use campbooks_kernel::{
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use super::AnnounceInvitations;
use crate::{
    inbound::{CalendarChanged, CapabilityGranted, ConnectionRemoved},
    model::{CalendarId, ConnectionId},
    testkit::Kit,
};

/// Grant `member` a Google account's primary calendar; the connection.
async fn grant(kit: &Kit, workspace: WorkspaceId, member: UserId) -> ConnectionId {
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

/// The member's provider calendar.
async fn calendar(kit: &Kit, member: UserId) -> CalendarId {
    sqlx::query_scalar(
        "SELECT id FROM schedule.calendars WHERE member_id = $1 AND source = 'provider'",
    )
    .bind(member)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// 14:00 UTC, `days` from today.
fn at(days: i64) -> DateTime<Utc> {
    (Utc::now() + TimeDelta::days(days))
        .date_naive()
        .and_hms_opt(14, 0, 0)
        .unwrap()
        .and_utc()
}

/// An event of a batch: `id`, at `starts` for an hour, with `response` and
/// an organiser.
fn invite(id: &str, starts: DateTime<Utc>, response: &str, organizer: &Value) -> Value {
    json!({
        "provider_id": id, "etag": format!("\"{id}-1\""), "status": "confirmed",
        "title": "Offsite planning", "place": "Office, 3rd floor",
        "start": {"date_time": starts.to_rfc3339()},
        "end": {"date_time": (starts + TimeDelta::hours(1)).to_rfc3339()},
        "response": response,
        "organizer": organizer,
    })
}

/// Apply a batch of `upserts` and `removals` to the primary calendar.
async fn apply(
    kit: &Kit,
    workspace: WorkspaceId,
    connection: ConnectionId,
    upserts: Vec<Value>,
    removals: Vec<Value>,
) {
    let batch = kit
        .blob(
            workspace,
            &json!({
                "version": 1, "provider_calendar_id": "ana@example.com",
                "upserts": upserts, "removals": removals,
            }),
        )
        .await;
    kit.publish(
        workspace,
        &CalendarChanged {
            workspace: workspace.to_string(),
            connection,
            batch,
        },
    )
    .await;
}

async fn announced(kit: &Kit) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT payload FROM kernel.events WHERE name = 'schedule.invitation_changed' ORDER BY id",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

async fn event_id(kit: &Kit, provider_id: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM schedule.events WHERE provider_event_id = $1")
        .bind(provider_id)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

fn joao() -> Value {
    json!({"name": "João Pereira", "email": "joao@example.com"})
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invitation_from_a_person_is_announced_with_its_day_and_closes_when_cancelled(
    db: PgPool,
) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    let starts = at(3);
    apply(
        &kit,
        ws,
        connection,
        vec![invite("offsite", starts, "needs_action", &joao())],
        vec![],
    )
    .await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), 1, "{events:?}");
    let open = &events[0];
    assert_eq!(open["owner"], ana.to_string());
    assert_eq!(open["calendar"], calendar(&kit, ana).await.to_string());
    assert_eq!(open["revision"], 1);
    // Europe/Lisbon is not set as the member's zone: UTC.
    assert_eq!(
        open["invitation"]["on"],
        starts.date_naive().format("%Y-%m-%d").to_string()
    );
    assert_eq!(
        open["invitation"]["lapses_at"]
            .as_str()
            .unwrap()
            .parse::<DateTime<Utc>>()
            .unwrap(),
        starts
    );
    assert_eq!(open["invitation"]["from_person"], true);
    assert!(open.get("closed_because").is_none());
    // Nothing of what it says travels: no title, place or address.
    let text = open.to_string();
    for word in ["Offsite", "Office", "joao", "João"] {
        assert!(!text.contains(word), "{word}: {text}");
    }
    // Moved an hour later: announced again, newer.
    apply(
        &kit,
        ws,
        connection,
        vec![invite(
            "offsite",
            starts + TimeDelta::hours(1),
            "needs_action",
            &joao(),
        )],
        vec![],
    )
    .await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["revision"], 2);

    apply(
        &kit,
        ws,
        connection,
        vec![],
        vec![json!({"provider_id": "offsite"})],
    )
    .await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["revision"], 3);
    assert_eq!(events[2]["closed_because"], "cancelled");
    assert!(events[2].get("invitation").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_service_an_organiser_and_an_answered_event_are_told_apart(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    apply(
        &kit,
        ws,
        connection,
        vec![
            invite(
                "webinar",
                at(1),
                "needs_action",
                &json!({"name": "Webinars", "email": "no-reply@webinars.example.com"}),
            ),
            // Ana organises it.
            invite(
                "standup",
                at(2),
                "needs_action",
                &json!({"name": "Ana", "email": "ana@example.com", "self": true}),
            ),
            invite("lunch", at(2), "accepted", &joao()),
            // Already started.
            invite(
                "earlier",
                Utc::now() - TimeDelta::minutes(5),
                "needs_action",
                &joao(),
            ),
        ],
        vec![],
    )
    .await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(
        events[0]["event"],
        event_id(&kit, "webinar").await.to_string()
    );
    assert_eq!(events[0]["invitation"]["from_person"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_answer_closes_it_and_reaches_google_only_after_the_undo_window(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    apply(
        &kit,
        ws,
        connection,
        vec![invite("offsite", at(3), "needs_action", &joao())],
        vec![],
    )
    .await;
    let id = event_id(&kit, "offsite").await;
    let uri = format!("/api/v1/w/{ws}/schedule/events/{id}/response");
    let requests = |kit: &Kit| {
        let db = kit.db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM schedule.change_requests WHERE change = 'respond'",
            )
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };

    let accepted = kit
        .post(&token, &uri, Some(json!({"response": "accepted"})))
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.json);
    assert_eq!(accepted.json["previous_response"], "needs_action");
    assert_eq!(accepted.json["event"]["pending"], true);
    let events = announced(&kit).await;
    assert_eq!(events.last().unwrap()["closed_because"], "answered");
    // Nothing asked of Google yet: the answer waits out its undo window.
    assert_eq!(requests(&kit).await, 0);

    // Taken back in time: open again, and nothing ever reaches Google.
    let undone = kit
        .post(&token, &uri, Some(json!({"response": "needs_action"})))
        .await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json["event"]["pending"], false);
    let events = announced(&kit).await;
    assert!(events.last().unwrap()["invitation"].is_object());
    sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE state = 'pending'")
        .execute(&kit.db)
        .await
        .unwrap();
    kit.run_jobs().await;
    assert_eq!(requests(&kit).await, 0);

    // Declined, then the window passes: the answer is asked of Google.
    let declined = kit
        .post(&token, &uri, Some(json!({"response": "declined"})))
        .await;
    assert_eq!(declined.status, StatusCode::OK, "{:?}", declined.json);
    // A second answer within the window replaces it.
    let maybe = kit
        .post(&token, &uri, Some(json!({"response": "tentative"})))
        .await;
    assert_eq!(maybe.status, StatusCode::OK, "{:?}", maybe.json);
    assert_eq!(maybe.json["previous_response"], "declined");
    sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE state = 'pending'")
        .execute(&kit.db)
        .await
        .unwrap();
    kit.run_jobs().await;
    assert_eq!(requests(&kit).await, 1);
    let asked: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM kernel.events WHERE name = 'schedule.outbound_requested'",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["change"], "respond");
    // Once asked of Google, another answer waits for it to finish.
    let again = kit
        .post(&token, &uri, Some(json!({"response": "accepted"})))
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{:?}", again.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_calendar_turned_off_or_removed_closes_its_invitations(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    apply(
        &kit,
        ws,
        connection,
        vec![invite("offsite", at(3), "needs_action", &joao())],
        vec![],
    )
    .await;
    let work = calendar(&kit, ana).await;
    let uri = format!("/api/v1/w/{ws}/schedule/calendars/{work}");
    let off = kit.patch(&token, &uri, json!({"syncing": false})).await;
    assert_eq!(off.status, StatusCode::OK, "{:?}", off.json);
    let events = announced(&kit).await;
    assert_eq!(events.last().unwrap()["closed_because"], "hidden");
    let on = kit.patch(&token, &uri, json!({"syncing": true})).await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.json);
    let events = announced(&kit).await;
    assert!(events.last().unwrap()["invitation"].is_object());

    kit.publish(
        ws,
        &ConnectionRemoved {
            workspace: ws.to_string(),
            connection,
        },
    )
    .await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(events[3]["closed_because"], "removed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn following_a_calendar_hides_its_invitations_and_mine_announces_them_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (token, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    apply(
        &kit,
        ws,
        connection,
        vec![invite("offsite", at(3), "needs_action", &joao())],
        vec![],
    )
    .await;
    let work = calendar(&kit, ana).await;
    let uri = format!("/api/v1/w/{ws}/schedule/calendars/{work}");
    let followed = kit.patch(&token, &uri, json!({"role": "following"})).await;
    assert_eq!(followed.status, StatusCode::OK, "{:?}", followed.json);
    let events = announced(&kit).await;
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[1]["closed_because"], "hidden");
    // Following keeps it hidden when a batch changes the event.
    let mine = kit.patch(&token, &uri, json!({"role": "mine"})).await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.json);
    let events = announced(&kit).await;
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(events[2]["invitation"].is_object());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn invitations_stored_before_are_announced_once_by_the_backfill(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (_, ana) = kit.member("ana", ws);
    let connection = grant(&kit, ws, ana).await;
    apply(
        &kit,
        ws,
        connection,
        vec![invite("offsite", at(3), "needs_action", &joao())],
        vec![],
    )
    .await;
    // As if stored before invitations were announced.
    sqlx::query("UPDATE schedule.events SET invitation_open = false, invitation_revision = 0")
        .execute(&kit.db)
        .await
        .unwrap();
    let before = announced(&kit).await.len();
    sqlx::query("INSERT INTO schedule.invitation_backfills (workspace_id) VALUES ($1)")
        .bind(ws)
        .execute(&kit.db)
        .await
        .unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    jobs::enqueue(
        &mut tx,
        &AnnounceInvitations,
        EnqueueOptions::for_workspace(ws.to_string()),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let events = announced(&kit).await;
    assert_eq!(events.len(), before + 1, "{events:?}");
    let last = events.last().unwrap();
    assert_eq!(last["revision"], 1);
    assert!(last["invitation"].is_object());
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM schedule.invitation_backfills")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
