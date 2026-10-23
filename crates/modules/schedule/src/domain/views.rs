//! What the API shows of calendars and events.

use campbooks_kernel::RecordRef;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    recur::Occurrence,
    rows::{CalendarRow, EventRow},
};
use crate::model::{
    CalendarId, CalendarSource, EventId, EventKind, EventNotice, EventOrigin, EventStatus, Response,
};

/// A calendar of the member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CalendarView {
    /// The calendar.
    pub id: CalendarId,
    /// Their Campbooks calendar, or one of a connected provider account.
    pub source: CalendarSource,
    /// Its name.
    #[schema(example = "Family")]
    pub name: String,
    /// Its colour, `#rrggbb`, when the provider gave one.
    pub color: Option<String>,
    /// Its IANA time zone, when known.
    pub time_zone: Option<String>,
    /// Whether its events are shown (the Campbooks calendar always is).
    pub syncing: bool,
    /// Whether events may be created and changed on it.
    pub writable: bool,
    /// Whether it is the account's primary calendar.
    pub primary: bool,
}

impl From<&CalendarRow> for CalendarView {
    fn from(row: &CalendarRow) -> Self {
        Self {
            id: row.id,
            source: if row.is_campbooks() {
                CalendarSource::Campbooks
            } else {
                CalendarSource::Provider
            },
            name: row.name.clone(),
            color: row.color.clone(),
            time_zone: row.time_zone.clone(),
            syncing: row.syncing,
            writable: row.writable,
            primary: row.is_primary,
        }
    }
}

/// An event, or one occurrence of a series.
// The wire shape: independent facts a client reads, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EventView {
    /// The event (a series and its occurrences share it).
    pub id: EventId,
    /// Its calendar.
    pub calendar_id: CalendarId,
    /// What kind of dated thing it is.
    pub kind: EventKind,
    /// Its title.
    #[schema(example = "Flight LIS → AMS")]
    pub title: String,
    /// Where it is.
    pub place: Option<String>,
    /// Notes.
    pub notes: Option<String>,
    /// Whether it lasts whole days.
    pub all_day: bool,
    /// When it starts: an RFC 3339 time with the offset of the member's time
    /// zone, or a `YYYY-MM-DD` date when it is all-day.
    #[schema(example = "2026-11-06T07:40:00+00:00")]
    pub starts: String,
    /// When it ends, in the same form (exclusive for an all-day event).
    #[schema(example = "2026-11-06T11:55:00+00:00")]
    pub ends: String,
    /// The IANA zone it was written in, when it has one.
    pub time_zone: Option<String>,
    /// Whether it stands.
    pub status: EventStatus,
    /// Whether it is an occurrence of a series.
    pub recurring: bool,
    /// For an occurrence, the name to change it by (`?occurrence=`).
    pub occurrence: Option<String>,
    /// Who made it.
    pub origin: EventOrigin,
    /// How Scout found it, when Scout added it: `structured` or `model`.
    pub found_by: Option<String>,
    /// The email it was read from, if any.
    pub source: Option<RecordRef>,
    /// The conversation of that email, when known.
    pub source_conversation: Option<String>,
    /// The sentence of the email that gave the date, when Scout read one.
    pub quote: Option<String>,
    /// Your answer to the invitation, if it is one.
    pub response: Option<Response>,
    /// Whether your last change still waits to reach the provider.
    pub pending: bool,
    /// Why it does not show what you last asked for.
    pub notice: Option<EventNotice>,
    /// Whether it is on a provider calendar and exists there.
    pub on_provider: bool,
    /// Whether it counts when free time is worked out.
    pub busy: bool,
}

/// `row`, shown in `zone`; `occurrence` is the one occurrence of a series it
/// stands for.
pub fn event_view(row: &EventRow, zone: Tz, occurrence: Option<&Occurrence>) -> Option<EventView> {
    let span = occurrence.map_or_else(|| row.span(), |o| Some(o.span))?;
    Some(EventView {
        id: row.id,
        calendar_id: row.calendar_id,
        kind: row.kind.parse().unwrap_or(EventKind::None),
        title: row.title.clone(),
        place: row.place.clone(),
        notes: row.notes.clone(),
        all_day: row.all_day,
        starts: span.start.render(zone),
        ends: span.end.render(zone),
        time_zone: row.time_zone.clone(),
        status: row.status.parse().unwrap_or(EventStatus::Confirmed),
        recurring: occurrence.is_some() || row.series_id.is_some(),
        occurrence: occurrence
            .map(|o| o.key.clone())
            .or_else(|| row.original_start.clone()),
        origin: row.origin.parse().unwrap_or(EventOrigin::Member),
        found_by: row.found_by.clone(),
        source: row.source(),
        source_conversation: row.source_conversation.map(|id| id.to_string()),
        quote: row.quote.clone(),
        response: row.response.as_deref().and_then(|r| r.parse().ok()),
        pending: row.is_pending(),
        notice: row.notice.as_deref().and_then(|n| n.parse().ok()),
        on_provider: row.provider_event_id.is_some(),
        busy: row.busy,
    })
}
