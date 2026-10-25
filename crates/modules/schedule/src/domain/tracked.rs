//! The snapshots the module's tracked events carry (ADR 0020), read from its
//! own rows in the transaction of the change, and the plan's enums for its
//! values.
//!
//! Ids, enums, booleans, counts and times only: never a title, a place,
//! notes, the quote Scout read, a calendar's name or colour, a time zone, a
//! provider's calendar or event id, an etag or an iCalendar `UID`.

use campbooks_kernel::{
    AppState, Result,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};

use super::{rows::EventRow, settings::Settings};
use crate::{
    model::{CalendarId, EventId, EventKind},
    tracking,
};

/// The plan's kind (`none` for a kind it does not name).
pub fn kind(kind: &str) -> tracking::EventKind {
    match kind.parse::<EventKind>().unwrap_or(EventKind::None) {
        EventKind::Charge => tracking::EventKind::Charge,
        EventKind::Appointment => tracking::EventKind::Appointment,
        EventKind::Meeting => tracking::EventKind::Meeting,
        EventKind::Travel => tracking::EventKind::Travel,
        EventKind::Stay => tracking::EventKind::Stay,
        EventKind::Show => tracking::EventKind::Show,
        EventKind::Reservation => tracking::EventKind::Reservation,
        EventKind::Delivery => tracking::EventKind::Delivery,
        EventKind::Expiry => tracking::EventKind::Expiry,
        EventKind::Hold => tracking::EventKind::Hold,
        EventKind::None => tracking::EventKind::None,
    }
}

/// The plan's answer to an invitation (`needs_action` for none).
pub fn response(response: Option<&str>) -> tracking::Response {
    match response {
        Some("accepted") => tracking::Response::Accepted,
        Some("tentative") => tracking::Response::Tentative,
        Some("declined") => tracking::Response::Declined,
        _ => tracking::Response::NeedsAction,
    }
}

fn origin(origin: &str) -> tracking::EventOrigin {
    match origin {
        "provider" => tracking::EventOrigin::Provider,
        "scout" => tracking::EventOrigin::Scout,
        _ => tracking::EventOrigin::Member,
    }
}

fn found_by(found_by: Option<&str>) -> Option<tracking::FoundBy> {
    match found_by {
        Some("structured") => Some(tracking::FoundBy::Structured),
        Some("model") => Some(tracking::FoundBy::Model),
        _ => None,
    }
}

fn status(status: &str) -> tracking::EventStatus {
    match status {
        "tentative" => tracking::EventStatus::Tentative,
        "cancelled" => tracking::EventStatus::Cancelled,
        _ => tracking::EventStatus::Confirmed,
    }
}

fn notice(notice: Option<&str>) -> Option<tracking::EventNotice> {
    match notice {
        Some("kept_theirs") => Some(tracking::EventNotice::KeptTheirs),
        Some("not_saved") => Some(tracking::EventNotice::NotSaved),
        _ => None,
    }
}

fn change(change: Option<&str>) -> Option<tracking::CalendarChange> {
    match change {
        Some("create") => Some(tracking::CalendarChange::Create),
        Some("update") => Some(tracking::CalendarChange::Update),
        Some("update_occurrence") => Some(tracking::CalendarChange::UpdateOccurrence),
        Some("delete") => Some(tracking::CalendarChange::Delete),
        Some("respond") => Some(tracking::CalendarChange::Respond),
        _ => None,
    }
}

fn source(source: &str) -> tracking::CalendarSource {
    if source == "campbooks" {
        tracking::CalendarSource::Campbooks
    } else {
        tracking::CalendarSource::Provider
    }
}

fn provider(provider: Option<&str>) -> Option<tracking::Provider> {
    match provider {
        Some("google") => Some(tracking::Provider::Google),
        Some("zoho") => Some(tracking::Provider::Zoho),
        _ => None,
    }
}

/// An event joined to its calendar, as the snapshot reads it.
#[derive(sqlx::FromRow)]
#[allow(clippy::struct_excessive_bools)]
struct EventFacts {
    id: Uuid,
    calendar_id: Uuid,
    member_id: Uuid,
    calendar_source: String,
    provider: Option<String>,
    kind: String,
    origin: String,
    found_by: Option<String>,
    status: String,
    all_day: bool,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    starts_on: Option<NaiveDate>,
    ends_on: Option<NaiveDate>,
    recurring: bool,
    series_id: Option<Uuid>,
    busy: bool,
    response: Option<String>,
    outbound_pending: Option<String>,
    notice: Option<String>,
    at_provider: bool,
    source_type: Option<String>,
    source_id: Option<String>,
    pushed_from: Option<Uuid>,
    pushed_to: Option<Uuid>,
    deleted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// The snapshot of event `id` as it is now in `conn`'s transaction, if it
/// exists.
pub async fn calendar_event(
    conn: &mut PgConnection,
    id: EventId,
) -> Result<Option<tracking::CalendarEventSnapshot>> {
    let facts = sqlx::query_as::<_, EventFacts>(
        "SELECT e.id, e.calendar_id, c.member_id, c.source AS calendar_source, c.provider,
                e.kind, e.origin, e.found_by, e.status, e.all_day, e.starts_at, e.ends_at,
                e.starts_on, e.ends_on,
                (cardinality(e.recurrence) > 0 AND e.series_id IS NULL) AS recurring,
                e.series_id, e.busy, e.response, e.outbound_pending, e.notice,
                (e.provider_event_id IS NOT NULL) AS at_provider, e.source_type, e.source_id,
                e.pushed_from, e.pushed_to, e.deleted_at, e.created_at, e.updated_at
           FROM schedule.events e
           JOIN schedule.calendars c ON c.id = e.calendar_id
          WHERE e.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(facts.map(|row| tracking::CalendarEventSnapshot {
        id: row.id,
        calendar_id: row.calendar_id,
        member_id: row.member_id,
        calendar_source: source(&row.calendar_source),
        provider: provider(row.provider.as_deref()),
        kind: kind(&row.kind),
        origin: origin(&row.origin),
        found_by: found_by(row.found_by.as_deref()),
        status: status(&row.status),
        all_day: row.all_day,
        starts_at: row.starts_at,
        ends_at: row.ends_at,
        starts_on: row.starts_on,
        ends_on: row.ends_on,
        recurring: row.recurring,
        series_id: row.series_id,
        busy: row.busy,
        response: row.response.as_deref().map(|answer| response(Some(answer))),
        pending_change: change(row.outbound_pending.as_deref()),
        notice: notice(row.notice.as_deref()),
        at_provider: row.at_provider,
        // Only an email's id: any other source line is left out.
        source_message_id: match (row.source_type.as_deref(), row.source_id.as_deref()) {
            (Some("conversations.message"), Some(id)) => id.parse::<Uuid>().ok(),
            _ => None,
        },
        pushed_from_id: row.pushed_from,
        pushed_to_id: row.pushed_to,
        deleted_at: row.deleted_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

/// A calendar, as the snapshot reads it.
#[derive(sqlx::FromRow)]
struct CalendarFacts {
    id: Uuid,
    member_id: Uuid,
    source: String,
    provider: Option<String>,
    connection_id: Option<Uuid>,
    syncing: bool,
    writable: bool,
    is_primary: bool,
    created_at: DateTime<Utc>,
}

/// The columns of [`CalendarFacts`].
const CALENDAR_FACTS: &str =
    "id, member_id, source, provider, connection_id, syncing, writable, is_primary, created_at";

fn calendar_snapshot(row: &CalendarFacts) -> tracking::CalendarSnapshot {
    tracking::CalendarSnapshot {
        id: row.id,
        member_id: row.member_id,
        source: source(&row.source),
        provider: provider(row.provider.as_deref()),
        connection_id: row.connection_id,
        syncing: row.syncing,
        writable: row.writable,
        primary: row.is_primary,
        created_at: row.created_at,
    }
}

/// The snapshot of calendar `id` as it is now in `conn`'s transaction, if it
/// exists.
pub async fn calendar(
    conn: &mut PgConnection,
    id: CalendarId,
) -> Result<Option<tracking::CalendarSnapshot>> {
    let row = sqlx::query_as::<_, CalendarFacts>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_FACTS} FROM schedule.calendars WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row.as_ref().map(calendar_snapshot))
}

/// The calendars of `connection` as they stand, each with how many events
/// it holds: read before they are deleted.
pub async fn calendars_of_connection(
    conn: &mut PgConnection,
    connection: crate::model::ConnectionId,
) -> Result<Vec<(tracking::CalendarSnapshot, i64)>> {
    let rows = sqlx::query_as::<_, CalendarFacts>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_FACTS} FROM schedule.calendars WHERE connection_id = $1 ORDER BY id"
    )))
    .bind(connection)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let events: i64 =
            sqlx::query_scalar("SELECT count(*) FROM schedule.events WHERE calendar_id = $1")
                .bind(row.id)
                .fetch_one(&mut *conn)
                .await?;
        out.push((calendar_snapshot(&row), events));
    }
    Ok(out)
}

/// The snapshot of `member`'s settings, `settings`, as stored.
pub fn settings(member: UserId, settings: &Settings) -> tracking::SettingsSnapshot {
    let working_days = settings
        .hours()
        .values()
        .filter(|ranges| !ranges.is_empty())
        .count();
    tracking::SettingsSnapshot {
        member_id: *member.as_uuid(),
        time_zone_set: settings.time_zone.is_some(),
        working_hours_set: !settings.working_hours.is_empty(),
        working_days: i64::try_from(working_days).unwrap_or(i64::MAX),
        scout_kinds: settings
            .scout_kinds
            .iter()
            .map(|k| kind(k.as_str()))
            .collect(),
        also_add_to_kinds: settings.also_add_to.keys().map(|k| kind(k)).collect(),
    }
}

/// The times of `row`.
fn times(row: &EventRow) -> tracking::CalendarEventTimesSnapshot {
    tracking::CalendarEventTimesSnapshot {
        all_day: row.all_day,
        starts_at: row.starts_at,
        ends_at: row.ends_at,
        starts_on: row.starts_on,
        ends_on: row.ends_on,
    }
}

/// Whether `before` and `after` are at different times.
fn moved(before: &EventRow, after: &EventRow) -> bool {
    times(before) != times(after)
}

/// The fields other than the times that differ between `before` and
/// `after`.
fn edited(before: &EventRow, after: &EventRow) -> Vec<tracking::EventField> {
    let mut fields = Vec::new();
    if before.title != after.title {
        fields.push(tracking::EventField::Title);
    }
    if before.kind != after.kind {
        fields.push(tracking::EventField::Kind);
    }
    if before.place != after.place {
        fields.push(tracking::EventField::Place);
    }
    if before.notes != after.notes {
        fields.push(tracking::EventField::Notes);
    }
    if before.time_zone != after.time_zone {
        fields.push(tracking::EventField::TimeZone);
    }
    if before.busy != after.busy {
        fields.push(tracking::EventField::Busy);
    }
    if before.status != after.status {
        fields.push(tracking::EventField::Status);
    }
    if (&before.source_type, &before.source_id) != (&after.source_type, &after.source_id) {
        fields.push(tracking::EventField::Source);
    }
    fields
}

/// Track, in `tx`, what changed of an event that was `before`: its times
/// (`schedule.calendar_event_moved`) and its other fields
/// (`schedule.calendar_event_edited`), each only when it changed.
pub async fn changes(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    before: &EventRow,
) -> Result<()> {
    let Some(after) = super::rows::event_by_id(tx, before.id).await? else {
        return Ok(());
    };
    let fields = edited(before, &after);
    let was_moved = moved(before, &after);
    if fields.is_empty() && !was_moved {
        return Ok(());
    }
    let Some(snapshot) = calendar_event(tx, before.id).await? else {
        return Ok(());
    };
    if was_moved {
        state
            .tracker
            .track(
                tx,
                &tracking::CalendarEventMoved {
                    workspace,
                    calendar_event: snapshot.clone(),
                    before: times(before),
                },
            )
            .await?;
    }
    if !fields.is_empty() {
        let before_kind = fields
            .contains(&tracking::EventField::Kind)
            .then(|| kind(&before.kind));
        state
            .tracker
            .track(
                tx,
                &tracking::CalendarEventEdited {
                    workspace,
                    calendar_event: snapshot,
                    fields,
                    before_kind,
                },
            )
            .await?;
    }
    Ok(())
}

/// Track, in `tx`, that event `id` was made (`schedule.calendar_event_created`).
pub async fn created(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    id: EventId,
) -> Result<()> {
    if let Some(calendar_event) = calendar_event(tx, id).await? {
        state
            .tracker
            .track(
                tx,
                &tracking::CalendarEventCreated {
                    workspace,
                    calendar_event,
                },
            )
            .await?;
    }
    Ok(())
}
