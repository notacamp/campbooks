//! `POST …/tasks/{task_id}/hold` and `/release` — time held for a task on
//! the caller's calendar (campbooks-cloud#362). Holding makes a busy block of
//! kind `hold` on one of their writable calendars through the kernel's
//! `Holds` port, in this transaction, and plans the task for the block's day;
//! holding again moves it. Release takes the block off and leaves the task
//! with no plan. Each is the other's undo.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    schedule::HoldRequest,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use sqlx::types::Uuid;
use utoipa::ToSchema;

use crate::{
    domain::{
        OffsetQuery, Outcome, TaskChange, TaskHistoryDetail, TaskHistoryWhat, TaskId, TaskPlan,
        TaskRevision, TaskVerb, TaskView, access, actor_of, finish,
        holds::{clear, local_day},
        resources::TASK_RECORD,
        tracked::{self, HoldFacts},
    },
    features::get::TaskPath,
    scopes::TasksWrite,
    tracking,
};

/// The shortest and longest time held, in minutes.
pub const MINUTES: std::ops::RangeInclusive<i64> = 15..=240;

/// How far in the past a start may be (the slot the member picked began a
/// moment ago), in seconds.
const STARTED_GRACE_SECONDS: i64 = 120;

/// The kind of calendar a block is on, as its tracked event says it.
const fn calendar_kind(at_provider: bool) -> tracking::HoldCalendar {
    if at_provider {
        tracking::HoldCalendar::Provider
    } else {
        tracking::HoldCalendar::Campbooks
    }
}

/// The time to hold.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HoldTime {
    /// The revision you saw.
    pub revision: i64,
    /// When it starts: an RFC 3339 time, not in the past.
    pub starts: DateTime<Utc>,
    /// When it ends: 15 to 240 minutes after the start.
    pub ends: DateTime<Utc>,
    /// One of your calendars that can be written; your Campbooks calendar
    /// when left out.
    #[serde(default)]
    pub calendar_id: Option<Uuid>,
}

/// Hold time for a task on your calendar.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/hold",
    operation_id = "tasks.hold",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.release_hold")
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    request_body = HoldTime,
    responses(
        (status = OK, description = "The task afterwards, held and planned for that day.",
            body = TaskView),
        (status = BAD_REQUEST, description = "A start in the past or a length outside 15 to 240 \
            minutes (`invalid_hold`), a bill (`pay_not_held`), a calendar you only follow \
            (`calendar_followed`), or a calendar that cannot be written (`calendar_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or the task is with someone else now (`task_not_yours`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task or calendar, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), it is closed (`task_closed`), or its block has a change still on its way to the provider (`change_pending`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn hold(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<HoldTime>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let today = offset.today()?;
    let minutes = tracked::minutes(body.starts, body.ends);
    let started = body.starts < Utc::now() - TimeDelta::seconds(STARTED_GRACE_SECONDS);
    if body.ends <= body.starts || !MINUTES.contains(&minutes) || started {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_hold")));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Write, path.task_id, true).await?;
    // Only the assignee's own calendar holds it.
    let Some(me) = access.user.filter(|me| *me == row.assignee) else {
        return Err(Error::NotFound);
    };
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    if row.verb == TaskVerb::Pay.as_str() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("pay_not_held")));
    }
    // One hold per task: holding again moves it.
    if let Some(event) = row.hold_event {
        state
            .holds
            .release(&state, &mut tx, access.workspace, me, event)
            .await?;
    }
    let held = state
        .holds
        .hold(
            &state,
            &mut tx,
            access.workspace,
            HoldRequest {
                member: me,
                task: RecordRef::new(TASK_RECORD, &path.task_id),
                title: &row.title,
                starts: body.starts,
                ends: body.ends,
                calendar: body.calendar_id,
            },
        )
        .await?;
    let day = local_day(&state, &mut tx, access.workspace, me, body.starts).await?;
    sqlx::query(
        "UPDATE tasks.tasks SET plan = 'date', plan_on = $2, later_until = NULL,
                hold_event = $3, hold_calendar = $4, held_from = $5, held_to = $6,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(path.task_id)
    .bind(day)
    .bind(held.event)
    .bind(held.calendar)
    .bind(body.starts)
    .bind(body.ends)
    .execute(&mut *tx)
    .await?;
    let moved = row.hold_event.is_some();
    let view = finish(
        &state,
        &mut tx,
        access,
        path.task_id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change: TaskChange::Planned,
            history: Some(TaskHistoryWhat::Held),
            detail: TaskHistoryDetail {
                plan: Some(TaskPlan::Date),
                plan_on: Some(day),
                hold_event: Some(held.event),
                held_from: Some(body.starts),
                held_to: Some(body.ends),
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        today,
    )
    .await?;
    tracked::held(
        &state,
        &mut tx,
        access.workspace,
        path.task_id,
        HoldFacts {
            minutes,
            moved,
            by: tracking::HoldActor::Member,
            calendar: Some(calendar_kind(held.at_provider)),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Release the time held for a task: the block goes from your calendar.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/release",
    operation_id = "tasks.release_hold",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.hold")
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    request_body = TaskRevision,
    responses(
        (status = OK, description = "The task afterwards, with no hold and no plan.",
            body = TaskView),
        (status = BAD_REQUEST, description = "No time is held for it (`not_held`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or the task is with someone else now (`task_not_yours`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), it is closed (`task_closed`), or its block has a change still on its way to the provider (`change_pending`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn release(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let today = offset.today()?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Write, path.task_id, true).await?;
    let Some(me) = access.user.filter(|me| *me == row.assignee) else {
        return Err(Error::NotFound);
    };
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    let (Some(event), Some(from), Some(to)) = (row.hold_event, row.held_from, row.held_to) else {
        return Err(Error::BadRequest(crate::MESSAGES.reason("not_held")));
    };
    state
        .holds
        .release(&state, &mut tx, access.workspace, me, event)
        .await?;
    clear(&mut tx, &row).await?;
    sqlx::query("UPDATE tasks.tasks SET revision = revision + 1, updated_at = now() WHERE id = $1")
        .bind(path.task_id)
        .execute(&mut *tx)
        .await?;
    let view = finish(
        &state,
        &mut tx,
        access,
        path.task_id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change: TaskChange::Planned,
            history: Some(TaskHistoryWhat::Released),
            detail: TaskHistoryDetail {
                plan: Some(TaskPlan::None),
                hold_event: Some(event),
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        today,
    )
    .await?;
    tracked::hold_released(
        &state,
        &mut tx,
        access.workspace,
        path.task_id,
        tracked::minutes(from, to),
        tracking::HoldActor::Member,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
