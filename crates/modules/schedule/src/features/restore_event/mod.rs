//! `POST /api/v1/w/{workspace_id}/schedule/events/{event_id}/restore` — the
//! undo of a delete. Within the undo window nothing reached the provider and
//! nothing is asked of it; after it, a provider event is asked to be made
//! again. Its own undo is `schedule.delete_event`.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
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
        ops::{self, EventPath},
        outbound, publish,
        rows::{calendar_of, event_of},
        settings,
        views::{EventView, event_view},
    },
    model::{CalendarChange, EventId},
    scopes::ScheduleWrite,
};

/// The token a delete gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    /// The `undo_token` of `schedule.delete_event`.
    pub undo_token: String,
}

/// Bring a deleted event back.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}/restore",
    operation_id = "schedule.restore_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.delete_event")
        .automation()
        .assistant()
        .records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event."),
    ),
    request_body = Restore,
    responses(
        (status = OK, description = "The event, back.", body = EventView),
        (status = BAD_REQUEST, description = "The token is not the one of the delete \
            (`invalid_undo`), or the event is not deleted.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such event, or it is not yours.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "The delete is on its way to the provider \
            (`change_pending`): try again in a moment.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(path): extract::Path<EventPath>,
    extract::Json(body): extract::Json<Restore>,
) -> Result<Json<EventView>> {
    let access = &workspace.access;
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_undo"));
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Write, path.event_id).await?;
    let row = event_of(&mut tx, member, path.event_id).await?;
    let token = body.undo_token.parse::<Uuid>().map_err(|_| invalid())?;
    if row.deleted_at.is_none() || row.undo_token != Some(token) {
        return Err(invalid());
    }
    let calendar = calendar_of(&mut tx, member, row.calendar_id).await?;
    let asked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM schedule.change_requests
          WHERE event_id = $1 AND state = 'waiting'",
    )
    .bind(row.id)
    .fetch_one(&mut *tx)
    .await?;
    if asked > 0 {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    // Back, with the delete not asked of the provider (it had not been).
    sqlx::query(
        "UPDATE schedule.events
            SET deleted_at = NULL, undo_token = NULL,
                outbound_pending = CASE WHEN outbound_pending = 'delete' THEN NULL
                                        ELSE outbound_pending END,
                updated_at = now()
          WHERE id = $1 OR series_id = $1",
    )
    .bind(row.id)
    .execute(&mut *tx)
    .await?;
    // The member wants this item again.
    for key in &row.item_keys {
        sqlx::query("DELETE FROM schedule.removed_items WHERE member_id = $1 AND item_key = $2")
            .bind(member)
            .bind(key)
            .execute(&mut *tx)
            .await?;
    }
    let restored = event_of(&mut tx, member, row.id).await?;
    // The delete reached the provider already: make the event again there.
    if !calendar.is_campbooks() && restored.provider_event_id.is_none() {
        let document =
            outbound::document(CalendarChange::Create, &calendar, &restored, None, None)?;
        outbound::request(
            &state,
            &mut tx,
            outbound::Asked {
                workspace: access.workspace,
                member,
                calendar: &calendar,
                event: &restored,
                change: CalendarChange::Create,
                document,
            },
        )
        .await?;
    }
    publish::created(&state, &mut tx, access.workspace, member, &restored).await?;
    let zone = settings::load(&mut tx, member).await?.zone();
    let after = event_of(&mut tx, member, row.id).await?;
    tx.commit().await?;
    event_view(&after, zone, None)
        .map(Json)
        .ok_or(Error::NotFound)
}
