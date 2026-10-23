//! Writing events: the one insert every way of making an event uses.

use campbooks_kernel::{RecordRef, tenancy::WorkspaceId};
use sqlx::{PgConnection, types::Uuid};

use super::moment::Span;
use crate::model::{CalendarId, EventId, EventKind, EventOrigin};

/// The longest title, place and notes, as the providers take them.
pub const MAX_TITLE: usize = 1_000;
/// The longest place.
pub const MAX_PLACE: usize = 1_000;
/// The longest notes.
pub const MAX_NOTES: usize = 20_000;

/// A new event.
pub struct NewRow<'a> {
    /// Its id.
    pub id: EventId,
    /// The workspace.
    pub workspace: WorkspaceId,
    /// Its calendar.
    pub calendar: CalendarId,
    /// Its kind.
    pub kind: EventKind,
    /// Its title.
    pub title: &'a str,
    /// When it is.
    pub span: Span,
    /// The zone it is written in.
    pub time_zone: Option<&'a str>,
    /// Where.
    pub place: Option<&'a str>,
    /// Notes.
    pub notes: Option<&'a str>,
    /// Who made it.
    pub origin: EventOrigin,
    /// How Scout found it (`structured` or `model`).
    pub found_by: Option<&'a str>,
    /// The email it was read from.
    pub source: Option<&'a RecordRef>,
    /// The key of the read item that made it.
    pub item_key: Option<&'a str>,
    /// The sender's calendar `UID`.
    pub ical_uid: Option<&'a str>,
    /// Whether it counts for free time.
    pub busy: bool,
    /// The Campbooks event this one is a pushed copy of.
    pub pushed_from: Option<EventId>,
    /// An exception: its series.
    pub series_id: Option<EventId>,
    /// An exception: the name of the occurrence it replaces.
    pub original_start: Option<&'a str>,
    /// The conversation of the source message.
    pub source_conversation: Option<Uuid>,
}

/// Insert `new`.
pub async fn insert(conn: &mut PgConnection, new: &NewRow<'_>) -> Result<(), sqlx::Error> {
    let (starts_at, ends_at, starts_on, ends_on) = new.span.columns();
    let keys: Vec<&str> = new.item_key.into_iter().collect();
    sqlx::query(
        "INSERT INTO schedule.events
             (id, workspace_id, calendar_id, kind, title, starts_at, ends_at, starts_on, ends_on,
              all_day, time_zone, place, notes, origin, found_by, source_type, source_id,
              item_key, item_keys, ical_uid, busy, pushed_from, series_id, original_start, source_conversation)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                 $18, $19, $20, $21, $22, $23, $24, $25)",
    )
    .bind(new.id)
    .bind(new.workspace)
    .bind(new.calendar)
    .bind(new.kind.as_str())
    .bind(new.title)
    .bind(starts_at)
    .bind(ends_at)
    .bind(starts_on)
    .bind(ends_on)
    .bind(new.span.start.is_day())
    .bind(new.time_zone)
    .bind(new.place)
    .bind(new.notes)
    .bind(new.origin.as_str())
    .bind(new.found_by)
    .bind(new.source.map(|source| source.record_type.as_str()))
    .bind(new.source.map(|source| source.id.as_str()))
    .bind(new.item_key)
    .bind(&keys)
    .bind(new.ical_uid)
    .bind(new.busy)
    .bind(new.pushed_from)
    .bind(new.series_id)
    .bind(new.original_start)
    .bind(new.source_conversation)
    .execute(conn)
    .await?;
    Ok(())
}

/// `text` trimmed; `None` when nothing is left.
pub fn tidy(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
