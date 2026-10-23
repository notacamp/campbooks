//! `POST …/tasks/{task_id}/done`, `/not_needed` and `/reopen` — close a task
//! (done, or paid for a bill; or not needed after all) and open it again.

use crate::{
    domain::{
        OffsetQuery, Outcome, TaskChange, TaskHistoryDetail, TaskId, TaskRevision, TaskState,
        TaskView, access, actor_of, finish,
    },
    features::get::TaskPath,
    scopes::TasksWrite,
};
use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::NaiveDate;

/// Mark a task done (paid, for a bill).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/done",
    operation_id = "tasks.done",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.reopen")
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    request_body = TaskRevision,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
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
pub async fn done(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    let today = offset.today()?;
    set_state(&state, &workspace, path.task_id, body, today, TaskState::Done)
        .await
        .map(Json)
}

/// Mark a task not needed.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/not_needed",
    operation_id = "tasks.not_needed",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.reopen")
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    request_body = TaskRevision,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
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
pub async fn not_needed(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    let today = offset.today()?;
    set_state(&state, &workspace, path.task_id, body, today, TaskState::NotNeeded)
        .await
        .map(Json)
}

/// Open a closed task again, with its plan.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/reopen",
    operation_id = "tasks.reopen",
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
    request_body = TaskRevision,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or the task is with someone else now (`task_not_yours`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), or it is still open (`task_open`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn reopen(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    let today = offset.today()?;
    set_state(&state, &workspace, path.task_id, body, today, TaskState::Open)
        .await
        .map(Json)
}

/// Move task `id` to `to`: closing needs it open, opening needs it closed.
async fn set_state(
    state: &AppState,
    workspace: &Workspace<TasksWrite>,
    id: TaskId,
    body: TaskRevision,
    today: NaiveDate,
    to: TaskState,
) -> Result<TaskView> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = access::require_task(state, &mut tx, access, Action::Write, id, true).await?;
    access::expect_revision(&row, body.revision)?;
    if to == TaskState::Open {
        if row.is_open() {
            return Err(Error::Conflict(crate::MESSAGES.reason("task_open")));
        }
    } else {
        access::expect_open(&row)?;
    }
    sqlx::query(
        "UPDATE tasks.tasks SET state = $2,
                closed_at = CASE WHEN $2 = 'open' THEN NULL ELSE now() END,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(to.as_str())
    .execute(&mut *tx)
    .await?;
    let change = match to {
        TaskState::Open => TaskChange::Reopened,
        TaskState::Done => TaskChange::Done,
        TaskState::NotNeeded => TaskChange::NotNeeded,
    };
    let view = finish(
        state,
        &mut tx,
        access,
        id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change,
            detail: TaskHistoryDetail::default(),
            previous_assignee: None,
            previous_handed_by: None,
        },
        today,
    )
    .await?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests;
