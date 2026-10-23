//! `GET /api/v1/w/{workspace_id}/schedule/events` — the caller's events and
//! the occurrences of their series within a window of at most 365 days,
//! filtered on the server (by calendar, kind and a word of the title) so a
//! caller never pages through the rest.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveTime, TimeDelta, TimeZone as _, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        listing::{self, Filter},
        moment::Moment,
        ops, settings,
        views::{EventView, event_view},
    },
    model::{CalendarId, EventKind},
    scopes::ScheduleRead,
};

/// The longest window, in days.
pub const MAX_WINDOW_DAYS: i64 = 365;
/// Most events in one answer when `limit` is not given.
pub const DEFAULT_LIMIT: u32 = 200;
/// Most events in one answer.
pub const MAX_LIMIT: u32 = 500;

/// Which events.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventsQuery {
    /// Start of the window: an RFC 3339 time with an offset, or a date (the
    /// start of that day in your time zone).
    #[param(example = "2026-11-01")]
    pub from: String,
    /// End of the window, in the same form, after `from` and at most 365
    /// days later.
    #[param(example = "2026-11-08")]
    pub to: String,
    /// Only these calendars: ids separated by commas.
    pub calendars: Option<String>,
    /// Only these kinds: kinds separated by commas (`travel,stay`).
    pub kind: Option<String>,
    /// Only events whose title contains this text, ignoring case and
    /// accents.
    pub q: Option<String>,
    /// At most this many, earliest first (1 to 500, default 200).
    pub limit: Option<u32>,
}

/// Events, earliest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EventList {
    /// The events and occurrences.
    pub events: Vec<EventView>,
    /// Whether more matched than `limit` allowed.
    pub truncated: bool,
}

/// The start of a window edge: a time, or midnight of a date in `zone`.
fn edge(text: &str, zone: Tz) -> Option<DateTime<Utc>> {
    match Moment::parse(text)? {
        Moment::At(at) => Some(at),
        Moment::On(day) => zone
            .from_local_datetime(&day.and_time(NaiveTime::MIN))
            .earliest()
            .map(|local| local.with_timezone(&Utc)),
    }
}

/// The window the query asks for.
fn window(query: &EventsQuery, zone: Tz) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_window"));
    let from = edge(&query.from, zone).ok_or_else(invalid)?;
    let to = edge(&query.to, zone).ok_or_else(invalid)?;
    if to <= from || to - from > TimeDelta::days(MAX_WINDOW_DAYS) {
        return Err(invalid());
    }
    Ok((from, to))
}

/// The filter the query asks for.
fn filter(query: &EventsQuery) -> Result<Filter> {
    let kinds = match query.kind.as_deref().filter(|text| !text.is_empty()) {
        None => None,
        Some(text) => Some(
            text.split(',')
                .map(|kind| {
                    kind.trim()
                        .parse::<EventKind>()
                        .map(|kind| kind.as_str().to_owned())
                        .map_err(|()| Error::BadRequest(crate::MESSAGES.reason("invalid_kind")))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
    };
    let calendars = match query.calendars.as_deref().filter(|text| !text.is_empty()) {
        None => None,
        Some(text) => Some(
            text.split(',')
                .map(|id| id.trim().parse::<CalendarId>().map_err(|_| Error::NotFound))
                .collect::<Result<Vec<_>>>()?,
        ),
    };
    Ok(Filter {
        calendars,
        kinds,
        q: query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_owned),
        busy_only: false,
    })
}

/// List your events and occurrences in a window.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/schedule/events",
    operation_id = "schedule.list_events",
    tag = "schedule",
    security(("bearer" = ["schedule:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), EventsQuery),
    responses(
        (status = OK, description = "The events.", body = EventList),
        (status = BAD_REQUEST, description = "A window that is not valid or longer than 365 days \
            (`invalid_window`), or a kind that does not exist (`invalid_kind`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "A calendar that is not yours.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleRead>,
    extract::Query(query): extract::Query<EventsQuery>,
) -> Result<Json<EventList>> {
    let access = &workspace.access;
    let member = ops::require_events_type(&state, access, Action::Read)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let settings = settings::load(&mut tx, member).await?;
    let zone = settings.zone();
    let (from, to) = window(&query, zone)?;
    let filter = filter(&query)?;
    if let Some(asked) = &filter.calendars {
        let own: Vec<CalendarId> =
            sqlx::query_scalar("SELECT id FROM schedule.calendars WHERE member_id = $1")
                .bind(member)
                .fetch_all(&mut *tx)
                .await?;
        if asked.iter().any(|id| !own.contains(id)) {
            return Err(Error::NotFound);
        }
    }
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
    let listed = listing::list(&mut tx, member, &filter, from, to).await?;
    tx.commit().await?;
    let truncated = listed.len() > limit;
    let events = listed
        .iter()
        .take(limit)
        .filter_map(|item| event_view(&item.row, zone, item.occurrence.as_ref()))
        .collect();
    Ok(Json(EventList { events, truncated }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(from: &str, to: &str) -> EventsQuery {
        EventsQuery {
            from: from.to_owned(),
            to: to.to_owned(),
            calendars: None,
            kind: None,
            q: None,
            limit: None,
        }
    }

    #[test]
    fn a_window_is_at_most_a_year() {
        let lisbon = "Europe/Lisbon".parse::<Tz>().unwrap();
        assert!(window(&query("2026-11-01", "2027-11-01"), lisbon).is_ok());
        assert!(window(&query("2026-11-01", "2028-11-01"), lisbon).is_err());
        assert!(window(&query("2026-11-08", "2026-11-01"), lisbon).is_err());
        assert!(window(&query("soon", "later"), lisbon).is_err());
    }

    #[test]
    fn a_date_edge_is_midnight_in_the_members_zone() {
        let madrid = "Europe/Madrid".parse::<Tz>().unwrap();
        let (from, _) = window(&query("2026-11-01", "2026-11-02"), madrid).unwrap();
        assert_eq!(from.to_rfc3339(), "2026-10-31T23:00:00+00:00");
    }

    #[test]
    fn kinds_and_calendars_are_comma_separated() {
        let mut q = query("2026-11-01", "2026-11-02");
        q.kind = Some("travel, stay".to_owned());
        assert_eq!(
            filter(&q).unwrap().kinds.unwrap(),
            ["travel".to_owned(), "stay".to_owned()]
        );
        q.kind = Some("holiday".to_owned());
        assert!(filter(&q).is_err());
    }
}
