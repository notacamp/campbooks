//! `POST …/tasks/{task_id}/plan` — when the member will do it: a day, Later
//! until a time, or no plan.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        OffsetQuery, Outcome, TaskChange, TaskHistoryDetail, TaskId, TaskPlan, TaskView, access,
        actor_of, finish,
    },
    features::get::TaskPath,
    scopes::TasksWrite,
};

/// The plan the task should have.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanTask {
    /// The revision you saw.
    pub revision: i64,
    /// `none`, `date` or `later`.
    pub plan: TaskPlan,
    /// The day, for `date`.
    #[serde(default)]
    pub plan_on: Option<NaiveDate>,
    /// Later than now and less than a year away, for `later`.
    #[serde(default)]
    pub later_until: Option<DateTime<Utc>>,
}

/// The furthest Later goes.
const LONGEST: TimeDelta = TimeDelta::days(366);

/// Plan a task: a day, Later, or nothing.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/plan",
    operation_id = "tasks.plan",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    request_body = PlanTask,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
        (status = BAD_REQUEST, description = "A `date` plan without a day (`plan_needs_day`), or a `later` time in the past or over a year away (`invalid_later`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or the task is with someone else now (`task_not_yours`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), or it is closed (`task_closed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<PlanTask>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let today = offset.today()?;
    let now = Utc::now();
    let (plan_on, later_until) = match body.plan {
        TaskPlan::None => (None, None),
        TaskPlan::Date => (
            Some(
                body.plan_on
                    .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("plan_needs_day")))?,
            ),
            None,
        ),
        TaskPlan::Later => {
            let until = body
                .later_until
                .filter(|until| *until > now && *until <= now + LONGEST)
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_later")))?;
            (None, Some(until))
        }
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Write, path.task_id, true).await?;
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    sqlx::query(
        "UPDATE tasks.tasks SET plan = $2, plan_on = $3, later_until = $4,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(path.task_id)
    .bind(body.plan.as_str())
    .bind(plan_on)
    .bind(later_until)
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
            history: None,
            detail: TaskHistoryDetail {
                plan: Some(body.plan),
                plan_on,
                later_until,
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        today,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
