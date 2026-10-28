//! Rows of the module's tables, and the lookups every use case starts from.

use campbooks_kernel::{
    Error, RecordRef, Result,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgConnection, types::Uuid};

use super::moment::{Moment, Span};
use crate::model::{CalendarId, CalendarRole, ConnectionId, EVENT_RECORD, EventId};

/// A calendar, as stored.
// The table's columns: independent facts, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, FromRow)]
pub struct CalendarRow {
    pub id: CalendarId,
    pub member_id: UserId,
    pub source: String,
    pub connection_id: Option<ConnectionId>,
    pub provider_calendar_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub time_zone: Option<String>,
    pub syncing: bool,
    pub writable: bool,
    pub is_primary: bool,
    pub provider: Option<String>,
    pub following: bool,
    pub owned: Option<bool>,
}

/// The columns of [`CalendarRow`], for `SELECT`.
pub const CALENDAR_COLUMNS: &str = "id, member_id, source, connection_id, provider_calendar_id, \
     name, color, time_zone, syncing, writable, is_primary, provider, \
     following, owned";

impl CalendarRow {
    /// Whether it is the member's Campbooks calendar.
    pub fn is_campbooks(&self) -> bool {
        self.source == "campbooks"
    }

    /// What the calendar is to the member.
    pub fn role(&self) -> CalendarRole {
        if !self.is_campbooks() && !self.syncing {
            CalendarRole::Off
        } else if self.following {
            CalendarRole::Following
        } else {
            CalendarRole::Mine
        }
    }

    /// Whether it is the member's own and on: the Campbooks calendar, or a
    /// provider calendar that syncs and is not just followed.
    pub fn is_mine(&self) -> bool {
        self.role() == CalendarRole::Mine
    }
}

/// An event, as stored.
// The table's columns: independent facts, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, FromRow)]
pub struct EventRow {
    pub id: EventId,
    pub calendar_id: CalendarId,
    pub kind: String,
    pub title: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    pub all_day: bool,
    pub time_zone: Option<String>,
    pub place: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub provider_event_id: Option<String>,
    pub etag: Option<String>,
    pub outbound_pending: Option<String>,
    pub recurrence: Vec<String>,
    pub series_id: Option<EventId>,
    pub original_start: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub origin: String,
    pub found_by: Option<String>,
    pub pushed_from: Option<EventId>,
    pub item_key: Option<String>,
    pub ical_uid: Option<String>,
    pub response: Option<String>,
    pub notice: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub undo_token: Option<Uuid>,
    pub pushed_to: Option<EventId>,
    pub busy: bool,
    pub provider_version: Option<serde_json::Value>,
    pub item_keys: Vec<String>,
    pub source_conversation: Option<Uuid>,
    pub quote: Option<String>,
    pub organizer_name: Option<String>,
    pub organizer_address: Option<String>,
    pub organizer_self: bool,
    pub organizer_person: Option<bool>,
    pub invitation_revision: i64,
    pub invitation_open: bool,
}

/// The columns of [`EventRow`], for `SELECT` (table alias `e`).
pub const EVENT_COLUMNS: &str = "e.id, e.calendar_id, e.kind, e.title, e.starts_at, e.ends_at, \
     e.starts_on, e.ends_on, e.all_day, e.time_zone, e.place, e.notes, e.status, \
     e.provider_event_id, e.etag, e.outbound_pending, e.recurrence, e.series_id, \
     e.original_start, e.source_type, e.source_id, e.origin, e.found_by, e.pushed_from, \
     e.item_key, e.ical_uid, e.response, e.notice, e.deleted_at, e.undo_token, e.pushed_to, \
     e.busy, e.provider_version, e.item_keys, e.source_conversation, e.quote, \
     e.organizer_name, e.organizer_address, e.organizer_self, e.organizer_person, \
     e.invitation_revision, e.invitation_open";

impl EventRow {
    /// When it starts and ends.
    pub fn span(&self) -> Option<Span> {
        if self.all_day {
            let start = Moment::On(self.starts_on?);
            let end = self.ends_on.map(Moment::On);
            Span::new(start, end)
        } else {
            let start = Moment::At(self.starts_at?);
            let end = self.ends_at.map(Moment::At);
            Span::new(start, end)
        }
    }

    /// Whether it is a series (it has recurrence lines and is no exception).
    pub fn is_series(&self) -> bool {
        !self.recurrence.is_empty() && self.series_id.is_none()
    }

    /// The record it was read from.
    pub fn source(&self) -> Option<RecordRef> {
        match (&self.source_type, &self.source_id) {
            (Some(kind), Some(id)) => Some(RecordRef::new(kind.clone(), id)),
            _ => None,
        }
    }

    /// Whether the member's change to it still waits for the provider.
    pub fn is_pending(&self) -> bool {
        self.outbound_pending.is_some()
    }
}

/// The member's calendar, by id, locked for the change that follows.
pub async fn calendar_of(
    conn: &mut PgConnection,
    member: UserId,
    id: CalendarId,
) -> Result<CalendarRow> {
    sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars WHERE id = $1 AND member_id = $2"
    )))
    .bind(id)
    .bind(member)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)
}

/// The member's calendars: their Campbooks calendar (made now when they have
/// none) and their provider calendars, the Campbooks one first.
pub async fn calendars_of(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    member: UserId,
) -> Result<Vec<CalendarRow>> {
    ensure_campbooks_calendar(conn, workspace, member).await?;
    let rows = sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars WHERE member_id = $1
          ORDER BY (source = 'campbooks') DESC, is_primary DESC, name, id"
    )))
    .bind(member)
    .fetch_all(conn)
    .await?;
    Ok(rows)
}

/// The member's Campbooks calendar, made the first time anything needs it.
pub async fn ensure_campbooks_calendar(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    member: UserId,
) -> Result<CalendarRow> {
    let id = CalendarId::new();
    sqlx::query(
        "INSERT INTO schedule.calendars (id, workspace_id, member_id, source, name, syncing, writable)
         VALUES ($1, $2, $3, 'campbooks', 'Campbooks', true, true)
         ON CONFLICT (workspace_id, member_id) WHERE source = 'campbooks' DO NOTHING",
    )
    .bind(id)
    .bind(workspace)
    .bind(member)
    .execute(&mut *conn)
    .await?;
    let row = sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars
          WHERE member_id = $1 AND source = 'campbooks'"
    )))
    .bind(member)
    .fetch_one(conn)
    .await?;
    Ok(row)
}

/// An event of the member's, by id (including one they deleted and can still
/// restore), locked for the change that follows. Another member's event, and
/// one that does not exist, are both [`Error::NotFound`].
pub async fn event_of(conn: &mut PgConnection, member: UserId, id: EventId) -> Result<EventRow> {
    sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {EVENT_COLUMNS} FROM schedule.events e
           JOIN schedule.calendars c ON c.id = e.calendar_id
          WHERE e.id = $1 AND c.member_id = $2
            FOR UPDATE OF e"
    )))
    .bind(id)
    .bind(member)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)
}

/// The record reference of an event.
pub fn event_ref(id: EventId) -> RecordRef {
    RecordRef::new(EVENT_RECORD, &id)
}

/// A calendar by id, whoever's it is: for background work that already knows
/// whose request it carries out.
pub async fn calendar_by_id(conn: &mut PgConnection, id: CalendarId) -> Result<CalendarRow> {
    sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)
}

/// An event by id, whoever's it is, locked: for background work.
pub async fn event_by_id(conn: &mut PgConnection, id: EventId) -> Result<Option<EventRow>> {
    let row = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {EVENT_COLUMNS} FROM schedule.events e WHERE e.id = $1 FOR UPDATE"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row)
}
