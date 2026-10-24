//! `POST /api/v1/w/{workspace_id}/schedule/events` — make an event on one of
//! your calendars that can be written. On a provider calendar it is stored
//! at once, marked pending, and asked of connectors in the same transaction;
//! on your Campbooks calendar no provider ever hears of it.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use utoipa::ToSchema;

use crate::{
    domain::{
        moment::{Moment, Span},
        ops, outbound, publish,
        rows::{CalendarRow, calendar_of, ensure_campbooks_calendar, event_of},
        settings,
        views::{EventView, event_view},
        write::{MAX_NOTES, MAX_PLACE, MAX_TITLE, NewRow, insert, tidy},
    },
    model::{CalendarChange, CalendarId, EventId, EventKind, EventOrigin},
    scopes::ScheduleWrite,
};

/// An event to make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewEvent {
    /// The calendar; your Campbooks calendar when left out.
    pub calendar_id: Option<CalendarId>,
    /// What to call it.
    #[schema(example = "Dentist")]
    pub title: String,
    /// Its kind (default `none`).
    pub kind: Option<EventKind>,
    /// When it starts: an RFC 3339 time with an offset, or a `YYYY-MM-DD`
    /// date for an all-day event.
    #[schema(example = "2026-11-12T15:00:00+00:00")]
    pub starts: String,
    /// When it ends, in the same form (exclusive for a date). An hour (or a
    /// day) after the start when left out.
    pub ends: Option<String>,
    /// The IANA time zone it is written in (default: yours).
    pub time_zone: Option<String>,
    /// Where it is.
    pub place: Option<String>,
    /// Notes.
    pub notes: Option<String>,
    /// Whether it counts when free time is worked out (default yes).
    pub busy: Option<bool>,
    /// The email it is made from: a `conversations.message` you can read.
    /// Opening the event then leads back to that email.
    pub source: Option<RecordRef>,
    /// The conversation of that email (needed to open it).
    pub source_conversation: Option<String>,
}

/// The email a new event is made from, when the member can read it.
async fn checked_source(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    access: &campbooks_kernel::tenancy::Access,
    body: &NewEvent,
) -> Result<(Option<RecordRef>, Option<Uuid>)> {
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_source"));
    let Some(source) = &body.source else {
        return if body.source_conversation.is_some() {
            Err(invalid())
        } else {
            Ok((None, None))
        };
    };
    if source.record_type != "conversations.message" || source.id.parse::<Uuid>().is_err() {
        return Err(invalid());
    }
    let conversation = match &body.source_conversation {
        Some(id) => Some(id.parse::<Uuid>().map_err(|_| invalid())?),
        None => None,
    };
    // 404 when the message is not one the caller may read.
    campbooks_kernel::authz::require(state, conn, access, Action::Read, source, || {
        Error::Forbidden
    })
    .await?;
    Ok((Some(source.clone()), conversation))
}

/// Make an event.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/schedule/events",
    operation_id = "schedule.create_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.delete_event")
        .automation()
        .assistant()
        .records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = NewEvent,
    responses(
        (status = CREATED, description = "The event.", body = EventView),
        (status = BAD_REQUEST, description = "Times that do not read or do not follow each other \
            (`invalid_when`), a title that is empty or too long (`invalid_text`), a time zone that \
            is not an IANA name (`invalid_time_zone`), or a calendar that cannot be written \
            (`calendar_read_only`), or a source that is not an email (`invalid_source`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such calendar, or it is not yours, or the source \
            email is not one you can read.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Json(body): extract::Json<NewEvent>,
) -> Result<(StatusCode, Json<EventView>)> {
    let access = &workspace.access;
    let member = ops::require_events_type(&state, access, Action::Create)?;
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_text")));
    }
    let place = tidy(body.place.as_deref());
    let notes = tidy(body.notes.as_deref());
    if place
        .as_deref()
        .is_some_and(|p| p.chars().count() > MAX_PLACE)
        || notes
            .as_deref()
            .is_some_and(|n| n.chars().count() > MAX_NOTES)
    {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_text")));
    }
    let span = parse_span(&body.starts, body.ends.as_deref())?;
    if let Some(zone) = &body.time_zone {
        settings::check_zone(zone)?;
    }

    let mut tx = state.db.begin_for(access.workspace).await?;
    let settings = settings::load(&mut tx, member).await?;
    let (source, source_conversation) = checked_source(&state, &mut tx, access, &body).await?;
    let calendar = match body.calendar_id {
        Some(id) => {
            ops::require_calendar(&state, &mut tx, access, Action::Write, id).await?;
            calendar_of(&mut tx, member, id).await?
        }
        None => ensure_campbooks_calendar(&mut tx, access.workspace, member).await?,
    };
    ensure_writable(&calendar)?;
    let time_zone = body
        .time_zone
        .clone()
        .or_else(|| settings.time_zone.clone());
    let id = EventId::new();
    insert(
        &mut tx,
        &NewRow {
            id,
            workspace: access.workspace,
            calendar: calendar.id,
            kind: body.kind.unwrap_or(EventKind::None),
            title,
            span,
            time_zone: time_zone.as_deref().filter(|_| !span.start.is_day()),
            place: place.as_deref(),
            notes: notes.as_deref(),
            origin: EventOrigin::Member,
            found_by: None,
            source: source.as_ref(),
            item_key: None,
            ical_uid: None,
            busy: body.busy.unwrap_or(true),
            pushed_from: None,
            series_id: None,
            original_start: None,
            source_conversation,
            quote: None,
        },
    )
    .await?;
    let row = event_of(&mut tx, member, id).await?;
    if !calendar.is_campbooks() {
        let document = outbound::document(CalendarChange::Create, &calendar, &row, None, None)?;
        outbound::request(
            &state,
            &mut tx,
            outbound::Asked {
                workspace: access.workspace,
                member,
                calendar: &calendar,
                event: &row,
                change: CalendarChange::Create,
                document,
            },
        )
        .await?;
    }
    publish::created(&state, &mut tx, access.workspace, member, &row).await?;
    let row = event_of(&mut tx, member, id).await?;
    tx.commit().await?;
    let view = event_view(&row, settings.zone(), None).ok_or(Error::NotFound)?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// The span of a start and an optional end.
pub fn parse_span(starts: &str, ends: Option<&str>) -> Result<Span> {
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_when"));
    let start = Moment::parse(starts).ok_or_else(invalid)?;
    let end = match ends {
        Some(text) => Some(Moment::parse(text).ok_or_else(invalid)?),
        None => None,
    };
    Span::new(start, end).ok_or_else(invalid)
}

/// Events may be made and changed on `calendar`.
pub fn ensure_writable(calendar: &CalendarRow) -> Result<()> {
    if calendar.writable && (calendar.is_campbooks() || calendar.syncing) {
        Ok(())
    } else {
        Err(Error::BadRequest(
            crate::MESSAGES.reason("calendar_read_only"),
        ))
    }
}
