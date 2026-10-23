//! `GET /api/v1/w/{workspace_id}/schedule/events/{event_id}` — one event.
//! Someone else's event answers 404, like one that does not exist.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        ops::{self, EventPath},
        rows::event_of,
        settings,
        views::{EventView, event_view},
    },
    model::EventId,
    scopes::ScheduleRead,
};

/// Read one event.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}",
    operation_id = "schedule.get_event",
    tag = "schedule",
    security(("bearer" = ["schedule:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event."),
    ),
    responses(
        (status = OK, description = "The event.", body = EventView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event, or it is not yours.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleRead>,
    extract::Path(path): extract::Path<EventPath>,
) -> Result<Json<EventView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Read, path.event_id).await?;
    let row = event_of(&mut tx, member, path.event_id).await?;
    let zone = settings::load(&mut tx, member).await?.zone();
    tx.commit().await?;
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(Error::NotFound);
    }
    event_view(&row, zone, None)
        .map(Json)
        .ok_or(Error::NotFound)
}
