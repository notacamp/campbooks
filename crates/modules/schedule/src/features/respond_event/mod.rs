//! `POST /api/v1/w/{workspace_id}/schedule/events/{event_id}/response` —
//! answer an invitation (accept, tentative, decline). The answer is stored at
//! once. For an event on a provider calendar it reaches the provider only when
//! the undo window has passed (`schedule.release_response`, as a delete does),
//! so an answer taken back in time (`previous_response`, sent back to the
//! same operation) never reaches it: within the window a new answer replaces
//! the one waiting, and "not answered" withdraws it. Once the provider has an
//! answer it is never told "not answered" (no provider takes it back).

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
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{
        invitations,
        ops::{self, EventPath},
        outbound,
        rows::{calendar_by_id, calendar_of, event_by_id, event_of},
        settings, tracked,
        views::{EventView, event_view},
    },
    features::delete_event::UNDO_WINDOW_SECONDS,
    model::{CalendarChange, EventId, Response},
    scopes::ScheduleWrite,
    tracking,
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
    // An answer still in its undo window is replaced; any other change
    // waiting for the provider (an answer already asked of it among them)
    // has to finish first.
    let in_window = row.outbound_pending.as_deref() == Some(CalendarChange::Respond.as_str())
        && !asked_of_provider(&mut tx, row.id).await?;
    if row.is_pending() && !in_window {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let calendar = calendar_of(&mut tx, member, row.calendar_id).await?;
    let previous = row
        .response
        .as_deref()
        .and_then(|r| r.parse().ok())
        .unwrap_or(Response::NeedsAction);
    // To the provider after the undo window: an answer, never "not
    // answered" (which withdraws one still waiting).
    let to_provider = !calendar.is_campbooks()
        && row.provider_event_id.is_some()
        && answer.response != Response::NeedsAction;
    sqlx::query(
        "UPDATE schedule.events
            SET response = $2, notice = NULL,
                outbound_pending = CASE WHEN $3 THEN 'respond'
                                        WHEN $4 THEN NULL
                                        ELSE outbound_pending END,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(answer.response.as_str())
    .bind(to_provider)
    .bind(in_window)
    .execute(&mut *tx)
    .await?;
    if to_provider {
        let options = EnqueueOptions::for_workspace(access.workspace.to_string())
            .unique(format!("schedule.release_response:{}", row.id))
            .run_at(Utc::now() + TimeDelta::seconds(UNDO_WINDOW_SECONDS));
        campbooks_kernel::jobs::enqueue(&mut tx, &ReleaseResponse { event: row.id }, options)
            .await?;
    }
    invitations::announce(&state, &mut tx, access.workspace, row.id).await?;
    // Another answer than the one it had: the same again tracks nothing.
    if answer.response != previous
        && let Some(calendar_event) = tracked::calendar_event(&mut tx, row.id).await?
    {
        state
            .tracker
            .track(
                &mut tx,
                &tracking::CalendarEventResponded {
                    workspace: access.workspace,
                    calendar_event,
                    before: tracked::response(Some(previous.as_str())),
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

/// Whether a change of event `id` was already asked of the provider and has
/// not finished.
async fn asked_of_provider(conn: &mut PgConnection, id: EventId) -> Result<bool> {
    let asked: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM schedule.change_requests
                         WHERE event_id = $1 AND state = 'waiting')",
    )
    .bind(id)
    .fetch_one(conn)
    .await?;
    Ok(asked)
}

/// Asks connectors to give an event's answer to the provider once its undo
/// window has passed, if the answer still waits (it was not taken back).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReleaseResponse {
    /// The event.
    pub event: EventId,
}

impl Job for ReleaseResponse {
    const NAME: &'static str = "schedule.release_response";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("release_response enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(row) = event_by_id(tx, self.event).await? else {
            return Ok(());
        };
        // Taken back meanwhile, or already asked.
        if row.outbound_pending.as_deref() != Some(CalendarChange::Respond.as_str())
            || asked_of_provider(tx, row.id).await?
        {
            return Ok(());
        }
        let calendar = calendar_by_id(tx, row.calendar_id).await?;
        let answer = row
            .response
            .as_deref()
            .filter(|answer| *answer != Response::NeedsAction.as_str());
        let Some(answer) = answer.filter(|_| {
            !calendar.is_campbooks() && row.provider_event_id.is_some() && row.deleted_at.is_none()
        }) else {
            // Nothing to give the provider any more.
            sqlx::query(
                "UPDATE schedule.events SET outbound_pending = NULL, updated_at = now()
                  WHERE id = $1",
            )
            .bind(row.id)
            .execute(&mut **tx)
            .await?;
            return Ok(());
        };
        let document =
            outbound::document(CalendarChange::Respond, &calendar, &row, None, Some(answer))?;
        outbound::request(
            &state,
            tx,
            outbound::Asked {
                workspace,
                member: calendar.member_id,
                calendar: &calendar,
                event: &row,
                change: CalendarChange::Respond,
                document,
            },
        )
        .await?;
        Ok(())
    }
}
