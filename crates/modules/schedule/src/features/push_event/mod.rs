//! "Add to my calendar": `POST …/schedule/events/{event_id}/push` copies an
//! event of the member's Campbooks calendar to one of their provider
//! calendars that can be written, and `DELETE …/push` takes the provider
//! copy away again, keeping the Campbooks event.
//!
//! The copy is a new event (`pushed_from` names the Campbooks event) asked of
//! connectors in the same transaction; the Campbooks event is hidden while
//! the copy stands, so the member keeps one event. Scout never does this on
//! its own: the member presses it, or has set a per-kind calendar.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{
        ops::{self, EventPath},
        outbound, publish,
        rows::{CalendarRow, EventRow, calendar_of, event_of},
        settings,
        views::{EventView, event_view},
    },
    features::create_event::ensure_writable,
    model::{CalendarChange, CalendarId, EventId},
    scopes::ScheduleWrite,
};

/// Where to add it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PushTo {
    /// One of your provider calendars that can be written.
    pub calendar_id: CalendarId,
}

/// Add an event of your Campbooks calendar to one of your provider calendars.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}/push",
    operation_id = "schedule.push_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.unpush_event")
        .automation()
        .assistant()
        .records(["schedule.event", "schedule.calendar"]).access("schedule.event", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event on your Campbooks calendar."),
    ),
    request_body = PushTo,
    responses(
        (status = OK, description = "The event on the provider calendar.", body = EventView),
        (status = BAD_REQUEST, description = "The event is not on your Campbooks calendar \
            (`not_on_campbooks`), or the calendar cannot be written (`calendar_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event or calendar, or not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "An earlier change still waits (`change_pending`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn push(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(path): extract::Path<EventPath>,
    extract::Json(body): extract::Json<PushTo>,
) -> Result<Json<EventView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Write, path.event_id).await?;
    ops::require_calendar(&state, &mut tx, access, Action::Write, body.calendar_id).await?;
    let row = event_of(&mut tx, member, path.event_id).await?;
    let target = calendar_of(&mut tx, member, body.calendar_id).await?;
    let copy = push_row(&state, &mut tx, access.workspace, member, &row, &target).await?;
    let mut settings = settings::load(&mut tx, member).await?;
    settings.last_push_calendar = Some(target.id);
    settings::save(&mut tx, access.workspace, member, &settings).await?;
    let zone = settings.zone();
    tx.commit().await?;
    event_view(&copy, zone, None)
        .map(Json)
        .ok_or(Error::NotFound)
}

/// Copy `row`, an event of the member's Campbooks calendar, to `target`, and
/// ask connectors to make it there. Returns the copy.
pub async fn push_row(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: WorkspaceId,
    member: UserId,
    row: &EventRow,
    target: &CalendarRow,
) -> Result<EventRow> {
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(Error::NotFound);
    }
    if row.is_pending() {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let own = calendar_of(tx, member, row.calendar_id).await?;
    if !own.is_campbooks() || target.is_campbooks() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("not_on_campbooks"),
        ));
    }
    ensure_writable(target)?;
    let id = EventId::new();
    copy(tx, id, target.id, row.id).await?;
    sqlx::query("UPDATE schedule.events SET pushed_to = $2, updated_at = now() WHERE id = $1")
        .bind(row.id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let made = event_of(tx, member, id).await?;
    let document = outbound::document(CalendarChange::Create, target, &made, None, None)?;
    outbound::request(
        state,
        tx,
        outbound::Asked {
            workspace,
            member,
            calendar: target,
            event: &made,
            change: CalendarChange::Create,
            document,
        },
    )
    .await?;
    publish::created(state, tx, workspace, member, &made).await?;
    publish::cancelled(state, tx, workspace, member, row).await?;
    event_of(tx, member, id).await
}

/// Insert event `id` on `calendar` as a copy of `from`.
async fn copy(
    conn: &mut PgConnection,
    id: EventId,
    calendar: CalendarId,
    from: EventId,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO schedule.events
             (id, workspace_id, calendar_id, kind, title, starts_at, ends_at, starts_on, ends_on,
              all_day, time_zone, place, notes, status, recurrence, source_type, source_id,
              origin, found_by, pushed_from, busy, response, source_conversation)
         SELECT $1, workspace_id, $2, kind, title, starts_at, ends_at, starts_on, ends_on,
                all_day, time_zone, place, notes, status, recurrence, source_type, source_id,
                origin, found_by, id, busy, response, source_conversation
           FROM schedule.events WHERE id = $3",
    )
    .bind(id)
    .bind(calendar)
    .bind(from)
    .execute(conn)
    .await?;
    Ok(())
}

/// The event, back on the Campbooks calendar, and where it was removed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Unpushed {
    /// The Campbooks event: the one to push again to undo this.
    pub id: EventId,
    /// The provider calendar the copy was removed from: send it back as
    /// `calendar_id` to `schedule.push_event` to undo.
    pub calendar_id: CalendarId,
    /// The event afterwards.
    pub event: EventView,
}

/// Take an event added to a provider calendar away again.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}/push",
    operation_id = "schedule.unpush_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.push_event")
        .automation()
        .assistant()
        .records(["schedule.event", "schedule.calendar"]).access("schedule.event", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event on the provider calendar."),
    ),
    responses(
        (status = OK, description = "The event, back on your Campbooks calendar.",
            body = Unpushed),
        (status = BAD_REQUEST, description = "The event was not added from your Campbooks \
            calendar (`not_pushed`).", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event, or it is not yours.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "The event has not reached the provider yet \
            (`change_pending`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unpush(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(path): extract::Path<EventPath>,
) -> Result<Json<Unpushed>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Write, path.event_id).await?;
    let copy = event_of(&mut tx, member, path.event_id).await?;
    if copy.deleted_at.is_some() {
        return Err(Error::NotFound);
    }
    let original_id = copy
        .pushed_from
        .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("not_pushed")))?;
    let original = event_of(&mut tx, member, original_id).await?;
    if copy.is_pending() || copy.provider_event_id.is_none() {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let calendar = calendar_of(&mut tx, member, copy.calendar_id).await?;
    sqlx::query("UPDATE schedule.events SET deleted_at = now(), updated_at = now() WHERE id = $1")
        .bind(copy.id)
        .execute(&mut *tx)
        .await?;
    let document = outbound::document(CalendarChange::Delete, &calendar, &copy, None, None)?;
    outbound::request(
        &state,
        &mut tx,
        outbound::Asked {
            workspace: access.workspace,
            member,
            calendar: &calendar,
            event: &copy,
            change: CalendarChange::Delete,
            document,
        },
    )
    .await?;
    sqlx::query("UPDATE schedule.events SET pushed_to = NULL, updated_at = now() WHERE id = $1")
        .bind(original.id)
        .execute(&mut *tx)
        .await?;
    publish::cancelled(&state, &mut tx, access.workspace, member, &copy).await?;
    let back = event_of(&mut tx, member, original.id).await?;
    publish::created(&state, &mut tx, access.workspace, member, &back).await?;
    let zone = settings::load(&mut tx, member).await?.zone();
    tx.commit().await?;
    let event = event_view(&back, zone, None).ok_or(Error::NotFound)?;
    Ok(Json(Unpushed {
        id: back.id,
        calendar_id: copy.calendar_id,
        event,
    }))
}
