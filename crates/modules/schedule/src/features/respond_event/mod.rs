//! `POST /api/v1/w/{workspace_id}/schedule/events/{event_id}/response` —
//! answer an invitation (accept, tentative, decline). The answer is stored at
//! once; for an event on a provider calendar it is asked of connectors in the
//! same transaction. The answer before is `previous_response`, so the same
//! operation undoes it.

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
        ops::{self, EventPath},
        outbound,
        rows::{calendar_of, event_of},
        settings,
        views::{EventView, event_view},
    },
    model::{CalendarChange, EventId, Response},
    scopes::ScheduleWrite,
};

/// Your answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventAnswer {
    /// `accepted`, `tentative` or `declined` (a provider is never told
    /// `needs_action`: it only takes an answer back to "not answered" on
    /// events kept on Campbooks).
    pub response: Response,
}

/// Your answer, and the one before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AnsweredEvent {
    /// The event afterwards.
    pub event: EventView,
    /// The answer before this one: send it back as `response` to undo.
    pub previous_response: Response,
}

/// Answer an invitation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}/response",
    operation_id = "schedule.respond_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.respond_event")
        .automation()
        .assistant()
        .records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event."),
    ),
    request_body = EventAnswer,
    responses(
        (status = OK, description = "The event afterwards, and your earlier answer.",
            body = AnsweredEvent),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event, or it is not yours.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "An earlier change still waits for the provider \
            (`change_pending`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(path): extract::Path<EventPath>,
    extract::Json(answer): extract::Json<EventAnswer>,
) -> Result<Json<AnsweredEvent>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Write, path.event_id).await?;
    let row = event_of(&mut tx, member, path.event_id).await?;
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(Error::NotFound);
    }
    if row.is_pending() {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let calendar = calendar_of(&mut tx, member, row.calendar_id).await?;
    let previous = row
        .response
        .as_deref()
        .and_then(|r| r.parse().ok())
        .unwrap_or(Response::NeedsAction);
    sqlx::query(
        "UPDATE schedule.events SET response = $2, notice = NULL, updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(answer.response.as_str())
    .execute(&mut *tx)
    .await?;
    let updated = event_of(&mut tx, member, row.id).await?;
    if !calendar.is_campbooks()
        && updated.provider_event_id.is_some()
        && answer.response != Response::NeedsAction
    {
        let document = outbound::document(
            CalendarChange::Respond,
            &calendar,
            &updated,
            None,
            Some(answer.response.as_str()),
        )?;
        outbound::request(
            &state,
            &mut tx,
            outbound::Asked {
                workspace: access.workspace,
                member,
                calendar: &calendar,
                event: &updated,
                change: CalendarChange::Respond,
                document,
            },
        )
        .await?;
    }
    let zone = settings::load(&mut tx, member).await?.zone();
    let after = event_of(&mut tx, member, row.id).await?;
    tx.commit().await?;
    let event = event_view(&after, zone, None).ok_or(Error::NotFound)?;
    Ok(Json(AnsweredEvent {
        event,
        previous_response: previous,
    }))
}
