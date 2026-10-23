//! `PATCH /api/v1/w/{workspace_id}/schedule/events/{event_id}` — change one
//! event, or one occurrence of a series (`?occurrence=`, never the whole
//! series).
//!
//! The answer carries the event afterwards (`event`) and, at the top level
//! and named exactly like the fields of the request, the values of the
//! fields that changed as they were before: sent back as a `PATCH` to `id`,
//! they undo the change (`x-campbooks.undo` is this operation; an empty
//! string clears `place`, `notes` and `time_zone`). A change to an event on
//! a provider calendar is stored at once, marked pending, and asked of
//! connectors in the same transaction.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        listing::is_occurrence,
        moment::{Moment, Span},
        ops::{self, EventPath, OccurrenceQuery},
        outbound, publish,
        rows::{EventRow, calendar_of, event_of},
        settings,
        views::{EventView, event_view},
        write::{MAX_NOTES, MAX_PLACE, MAX_TITLE, NewRow, insert, tidy},
    },
    features::create_event::{ensure_writable, parse_span},
    model::{CalendarChange, EventId, EventKind, EventOrigin},
    scopes::ScheduleWrite,
};

/// What to change. Leave out what stays; an empty string clears `place`,
/// `notes` and `time_zone`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventPatch {
    /// A new title.
    pub title: Option<String>,
    /// A new kind.
    pub kind: Option<EventKind>,
    /// A new start (an RFC 3339 time with an offset, or a date for an all-day
    /// event). The event keeps its length unless `ends` is given too.
    pub starts: Option<String>,
    /// A new end, in the same form.
    pub ends: Option<String>,
    /// A new IANA time zone.
    pub time_zone: Option<String>,
    /// A new place.
    pub place: Option<String>,
    /// New notes.
    pub notes: Option<String>,
    /// Whether it counts when free time is worked out.
    pub busy: Option<bool>,
}

/// The event after the change, and what changed as it was before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UpdatedEvent {
    /// The event that was changed: the one to `PATCH` to undo it (changing
    /// one occurrence of a series makes an exception with an id of its own).
    pub id: EventId,
    /// The event afterwards.
    pub event: EventView,
    /// The previous title, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The previous kind, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EventKind>,
    /// The previous start, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<String>,
    /// The previous end, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends: Option<String>,
    /// The previous time zone (empty: none), if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// The previous place (empty: none), if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// The previous notes (empty: none), if they changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// The previous value of `busy`, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy: Option<bool>,
}

/// Change an event or one occurrence of a series.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}",
    operation_id = "schedule.update_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.update_event")
        .automation()
        .assistant()
        .records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event."),
        OccurrenceQuery,
    ),
    request_body = EventPatch,
    responses(
        (status = OK, description = "The event afterwards, and what changed as it was.",
            body = UpdatedEvent),
        (status = BAD_REQUEST, description = "Nothing to change (`nothing_to_change`), times that \
            do not read or follow each other (`invalid_when`), a text that is empty or too long \
            (`invalid_text`), a time zone that is not an IANA name (`invalid_time_zone`), a \
            whole series (`series_not_editable`) or a calendar that cannot be written \
            (`calendar_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event or occurrence, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "An earlier change still waits for the provider \
            (`change_pending`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(path): extract::Path<EventPath>,
    extract::Query(occurrence): extract::Query<OccurrenceQuery>,
    extract::Json(patch): extract::Json<EventPatch>,
) -> Result<Json<UpdatedEvent>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Write, path.event_id).await?;
    if patch == EventPatch::default() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("nothing_to_change"),
        ));
    }
    let mut row = event_of(&mut tx, member, path.event_id).await?;
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(Error::NotFound);
    }
    let calendar = calendar_of(&mut tx, member, row.calendar_id).await?;
    ensure_writable(&calendar)?;
    let settings = settings::load(&mut tx, member).await?;
    let zone = settings.zone();
    if row.is_pending() {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }

    // One occurrence of a series: an exception of its own, made now.
    let mut series: Option<EventRow> = None;
    let mut new_exception = false;
    if row.is_series() {
        let key = occurrence
            .occurrence
            .clone()
            .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("series_not_editable")))?;
        let existing: Option<EventId> = sqlx::query_scalar(
            "SELECT id FROM schedule.events
              WHERE series_id = $1 AND original_start = $2 AND deleted_at IS NULL",
        )
        .bind(row.id)
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
        let chosen = if let Some(id) = existing {
            event_of(&mut tx, member, id).await?
        } else {
            let found = is_occurrence(&row, &key).ok_or(Error::NotFound)?;
            let id = EventId::new();
            insert(
                &mut tx,
                &NewRow {
                    id,
                    workspace: access.workspace,
                    calendar: row.calendar_id,
                    kind: row.kind.parse().unwrap_or(EventKind::None),
                    title: &row.title,
                    span: found.span,
                    time_zone: row.time_zone.as_deref(),
                    place: row.place.as_deref(),
                    notes: row.notes.as_deref(),
                    origin: EventOrigin::Member,
                    found_by: None,
                    source: row.source().as_ref(),
                    item_key: None,
                    ical_uid: None,
                    busy: row.busy,
                    pushed_from: None,
                    series_id: Some(row.id),
                    original_start: Some(&key),
                    source_conversation: row.source_conversation,
                },
            )
            .await?;
            new_exception = true;
            event_of(&mut tx, member, id).await?
        };
        series = Some(std::mem::replace(&mut row, chosen));
        if row.is_pending() {
            return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
        }
    } else if occurrence.occurrence.is_some() && row.series_id.is_none() {
        return Err(Error::NotFound);
    }

    // The new values, and the old ones of what changes.
    let before = row.span().ok_or(Error::NotFound)?;
    let span = match (&patch.starts, &patch.ends) {
        (None, None) => before,
        (Some(starts), ends) => {
            let start = Moment::parse(starts).filter(|m| m.is_day() == before.start.is_day());
            let start =
                start.ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_when")))?;
            match ends {
                Some(ends) => parse_span(starts, Some(ends.as_str()))?,
                None => before
                    .shifted_to(start)
                    .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_when")))?,
            }
        }
        (None, Some(ends)) => {
            let end = Moment::parse(ends)
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_when")))?;
            Span::new(before.start, Some(end))
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_when")))?
        }
    };
    if span.start.is_day() != before.start.is_day() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_when")));
    }
    let invalid_text = || Error::BadRequest(crate::MESSAGES.reason("invalid_text"));
    let title = match &patch.title {
        Some(title) => {
            let title = title.trim();
            if title.is_empty() || title.chars().count() > MAX_TITLE {
                return Err(invalid_text());
            }
            title.to_owned()
        }
        None => row.title.clone(),
    };
    let text = |patch: &Option<String>, current: &Option<String>, max: usize| match patch {
        Some(text) => {
            let text = tidy(Some(text.as_str()));
            if text.as_deref().is_some_and(|t| t.chars().count() > max) {
                Err(invalid_text())
            } else {
                Ok(text)
            }
        }
        None => Ok(current.clone()),
    };
    let place = text(&patch.place, &row.place, MAX_PLACE)?;
    let notes = text(&patch.notes, &row.notes, MAX_NOTES)?;
    let time_zone = match &patch.time_zone {
        Some(zone) => {
            let zone = zone.trim();
            if zone.is_empty() {
                None
            } else {
                settings::check_zone(zone)?;
                Some(zone.to_owned())
            }
        }
        None => row.time_zone.clone(),
    };
    let kind = patch
        .kind
        .unwrap_or_else(|| row.kind.parse().unwrap_or(EventKind::None));
    let busy = patch.busy.unwrap_or(row.busy);

    let old_kind: EventKind = row.kind.parse().unwrap_or(EventKind::None);
    let mut undo = UndoValues::default();
    if title != row.title {
        undo.title = Some(row.title.clone());
    }
    if kind != old_kind {
        undo.kind = Some(old_kind);
    }
    if span != before {
        undo.starts = Some(before.start.render(zone));
        undo.ends = Some(before.end.render(zone));
    }
    if time_zone != row.time_zone {
        undo.time_zone = Some(row.time_zone.clone().unwrap_or_default());
    }
    if place != row.place {
        undo.place = Some(row.place.clone().unwrap_or_default());
    }
    if notes != row.notes {
        undo.notes = Some(row.notes.clone().unwrap_or_default());
    }
    if busy != row.busy {
        undo.busy = Some(row.busy);
    }
    let provider_text_changed = undo.title.is_some()
        || undo.starts.is_some()
        || undo.time_zone.is_some()
        || undo.place.is_some()
        || undo.notes.is_some();

    let (starts_at, ends_at, starts_on, ends_on) = span.columns();
    sqlx::query(
        "UPDATE schedule.events
            SET title = $2, kind = $3, place = $4, notes = $5, time_zone = $6, busy = $7,
                starts_at = $8, ends_at = $9, starts_on = $10, ends_on = $11, notice = NULL,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(&title)
    .bind(kind.as_str())
    .bind(&place)
    .bind(&notes)
    .bind(&time_zone)
    .bind(busy)
    .bind(starts_at)
    .bind(ends_at)
    .bind(starts_on)
    .bind(ends_on)
    .execute(&mut *tx)
    .await?;
    let updated = event_of(&mut tx, member, row.id).await?;

    if !calendar.is_campbooks() && provider_text_changed {
        let exception_of_series = new_exception || updated.provider_event_id.is_none();
        let (change, document) = match (&series, exception_of_series) {
            (Some(series), true) => (
                CalendarChange::UpdateOccurrence,
                outbound::document(
                    CalendarChange::UpdateOccurrence,
                    &calendar,
                    &updated,
                    Some(series),
                    None,
                )?,
            ),
            _ => (
                CalendarChange::Update,
                outbound::document(CalendarChange::Update, &calendar, &updated, None, None)?,
            ),
        };
        outbound::request(
            &state,
            &mut tx,
            outbound::Asked {
                workspace: access.workspace,
                member,
                calendar: &calendar,
                event: &updated,
                change,
                document,
            },
        )
        .await?;
    }
    if new_exception {
        publish::created(&state, &mut tx, access.workspace, member, &updated).await?;
    } else {
        publish::changed(&state, &mut tx, access.workspace, member, &updated).await?;
    }
    let after = event_of(&mut tx, member, row.id).await?;
    tx.commit().await?;
    let event = event_view(&after, zone, None).ok_or(Error::NotFound)?;
    Ok(Json(UpdatedEvent {
        id: after.id,
        event,
        title: undo.title,
        kind: undo.kind,
        starts: undo.starts,
        ends: undo.ends,
        time_zone: undo.time_zone,
        place: undo.place,
        notes: undo.notes,
        busy: undo.busy,
    }))
}

/// The previous values of what a change changed.
#[derive(Default)]
struct UndoValues {
    title: Option<String>,
    kind: Option<EventKind>,
    starts: Option<String>,
    ends: Option<String>,
    time_zone: Option<String>,
    place: Option<String>,
    notes: Option<String>,
    busy: Option<bool>,
}
