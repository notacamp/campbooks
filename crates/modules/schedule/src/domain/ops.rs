//! The access checks every handler starts with (ADR 0017), and the path
//! and query parameters of the routes.

use campbooks_kernel::{
    AppState, Error, RecordRef, Result,
    authz::{self, Action},
    tenancy::{Access, UserId},
};
use serde::Deserialize;
use sqlx::PgConnection;
use utoipa::IntoParams;

use crate::model::{CALENDAR_RECORD, CalendarId, EVENT_RECORD, EventId};

/// The member a request acts for. A client token acts for none, and has no
/// calendar.
pub fn member_of(access: &Access) -> Result<UserId> {
    access.user.ok_or(Error::Forbidden)
}

/// May the caller take `action` on events at all? Returns the member.
pub fn require_events_type(state: &AppState, access: &Access, action: Action) -> Result<UserId> {
    authz::require_type(state, access, action, EVENT_RECORD, || Error::Forbidden)?;
    member_of(access)
}

/// May the caller take `action` on calendars at all? Returns the member.
pub fn require_calendars_type(state: &AppState, access: &Access, action: Action) -> Result<UserId> {
    authz::require_type(state, access, action, CALENDAR_RECORD, || Error::Forbidden)?;
    member_of(access)
}

/// May the caller take `action` on event `id`? 404 when it is not theirs.
/// Returns the member.
pub async fn require_event(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    id: EventId,
) -> Result<UserId> {
    let record = RecordRef::new(EVENT_RECORD, &id);
    authz::require(state, conn, access, action, &record, || Error::Forbidden).await?;
    member_of(access)
}

/// May the caller take `action` on calendar `id`? 404 when it is not theirs.
/// Returns the member.
pub async fn require_calendar(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    id: CalendarId,
) -> Result<UserId> {
    let record = RecordRef::new(CALENDAR_RECORD, &id);
    authz::require(state, conn, access, action, &record, || Error::Forbidden).await?;
    member_of(access)
}

/// `/api/v1/w/{workspace_id}/schedule/events/{event_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct EventPath {
    /// The event.
    pub event_id: EventId,
}

/// `/api/v1/w/{workspace_id}/schedule/calendars/{calendar_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CalendarPath {
    /// The calendar.
    pub calendar_id: CalendarId,
}

/// Which occurrence of a series.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OccurrenceQuery {
    /// For an event of a series: the name of the occurrence (its `occurrence`
    /// in a list). A series is never changed as a whole.
    pub occurrence: Option<String>,
}
