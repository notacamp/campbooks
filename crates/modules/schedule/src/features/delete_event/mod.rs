//! `DELETE /api/v1/w/{workspace_id}/schedule/events/{event_id}` — delete an
//! event, with an undo. The event is hidden at once; for an event on a
//! provider calendar the delete reaches the provider only when the undo
//! window has passed (`schedule.release_delete`), so an undone delete never
//! touches the provider. The answer's `undo_token` goes to
//! `schedule.restore_event`.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Job, JobContext, JobError, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    jobs::EnqueueOptions,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use utoipa::ToSchema;

use crate::{
    domain::{
        ops::{self, EventPath},
        outbound, publish,
        rows::{calendar_by_id, calendar_of, event_by_id, event_of},
    },
    model::{CalendarChange, EventId},
    scopes::ScheduleWrite,
};

/// How long a delete can be undone before it reaches the provider, in
/// seconds.
pub const UNDO_WINDOW_SECONDS: i64 = 10;

/// A deleted event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DeletedEvent {
    /// The event.
    pub id: EventId,
    /// Sent to `schedule.restore_event` to bring it back.
    pub undo_token: String,
}

/// Delete an event (a series is deleted whole), with an undo.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/schedule/events/{event_id}",
    operation_id = "schedule.delete_event",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("schedule.restore_event")
        .automation()
        .assistant()
        .records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("event_id" = EventId, Path, description = "The event."),
    ),
    responses(
        (status = OK, description = "The event is deleted; the token undoes it.",
            body = DeletedEvent),
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
) -> Result<Json<DeletedEvent>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member = ops::require_event(&state, &mut tx, access, Action::Delete, path.event_id).await?;
    let row = event_of(&mut tx, member, path.event_id).await?;
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(Error::NotFound);
    }
    if row.is_pending() {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let calendar = calendar_of(&mut tx, member, row.calendar_id).await?;
    let token = Uuid::now_v7();
    sqlx::query(
        "UPDATE schedule.events SET deleted_at = now(), undo_token = $2, updated_at = now()
          WHERE id = $1 OR series_id = $1",
    )
    .bind(row.id)
    .bind(token)
    .execute(&mut *tx)
    .await?;
    // What Scout added and the member removed is not added again.
    if row.origin == "scout"
        && let Some(message) = row
            .source_id
            .as_deref()
            .and_then(|id| id.parse::<Uuid>().ok())
    {
        for key in &row.item_keys {
            sqlx::query(
                "INSERT INTO schedule.removed_items
                         (id, workspace_id, member_id, message_id, item_key, kind)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (workspace_id, member_id, item_key) DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(access.workspace)
            .bind(member)
            .bind(message)
            .bind(key)
            .bind(&row.kind)
            .execute(&mut *tx)
            .await?;
        }
    }
    if !calendar.is_campbooks() && row.provider_event_id.is_some() {
        sqlx::query("UPDATE schedule.events SET outbound_pending = 'delete' WHERE id = $1")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        let options = EnqueueOptions::for_workspace(access.workspace.to_string())
            .unique(format!("schedule.release_delete:{}", row.id))
            .run_at(Utc::now() + TimeDelta::seconds(UNDO_WINDOW_SECONDS));
        campbooks_kernel::jobs::enqueue(&mut tx, &ReleaseDelete { event: row.id }, options).await?;
    }
    publish::cancelled(&state, &mut tx, access.workspace, member, &row).await?;
    tx.commit().await?;
    Ok(Json(DeletedEvent {
        id: row.id,
        undo_token: token.to_string(),
    }))
}

/// Asks connectors to delete an event at the provider once its undo window
/// has passed, if it is still deleted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReleaseDelete {
    /// The event.
    pub event: EventId,
}

impl Job for ReleaseDelete {
    const NAME: &'static str = "schedule.release_delete";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("release_delete enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(row) = event_by_id(tx, self.event).await? else {
            return Ok(());
        };
        // Undone meanwhile, or already asked.
        if row.deleted_at.is_none() || row.outbound_pending.as_deref() != Some("delete") {
            return Ok(());
        }
        let asked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM schedule.change_requests
              WHERE event_id = $1 AND state = 'waiting'",
        )
        .bind(row.id)
        .fetch_one(&mut **tx)
        .await?;
        if asked > 0 {
            return Ok(());
        }
        let calendar = calendar_by_id(tx, row.calendar_id).await?;
        let document = outbound::document(CalendarChange::Delete, &calendar, &row, None, None)?;
        outbound::request(
            &state,
            tx,
            outbound::Asked {
                workspace,
                member: calendar.member_id,
                calendar: &calendar,
                event: &row,
                change: CalendarChange::Delete,
                document,
            },
        )
        .await?;
        Ok(())
    }
}
