//! `PATCH /api/v1/w/{workspace_id}/tasks/{task_id}` — change a task's title,
//! verb and due date, all at once: what is sent is what the task becomes,
//! so an undo is the same call with the values it had.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        OffsetQuery, Outcome, TaskChange, TaskEditedField, TaskHistoryDetail, TaskId, TaskVerb,
        TaskView, access, actor_of, clean_title, finish,
    },
    features::get::TaskPath,
    scopes::TasksWrite,
};

/// The task as it should become.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskUpdate {
    /// The revision you saw.
    pub revision: i64,
    /// What to do: 1 to 200 characters.
    pub title: String,
    /// Do or Pay.
    pub verb: TaskVerb,
    /// The day it is due; none clears it.
    #[serde(default)]
    pub due_on: Option<NaiveDate>,
    /// The time it is due; needs `due_on`.
    #[serde(default)]
    pub due_at: Option<DateTime<Utc>>,
}

/// Change a task's title, verb or due date.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}",
    operation_id = "tasks.update",
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
    request_body = TaskUpdate,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
        (status = BAD_REQUEST, description = "A blank or too long title (`invalid_title`), or a due time without a day.",
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
    extract::Json(body): extract::Json<TaskUpdate>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let today = offset.today()?;
    let title = clean_title(&body.title)?;
    if body.due_at.is_some() && body.due_on.is_none() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("due_time_needs_day"),
        ));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Write, path.task_id, true).await?;
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    let mut fields = Vec::new();
    if row.title != title {
        fields.push(TaskEditedField::Title);
    }
    if row.verb != body.verb.as_str() {
        fields.push(TaskEditedField::TaskVerb);
    }
    let due_changed = row.due_on != body.due_on || row.due_at != body.due_at;
    if due_changed {
        fields.push(TaskEditedField::Due);
    }
    if fields.is_empty() {
        tx.commit().await?;
        return Ok(Json(row.view(Utc::now(), today)?));
    }
    sqlx::query(
        "UPDATE tasks.tasks SET title = $2, verb = $3, due_on = $4, due_at = $5,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(path.task_id)
    .bind(&title)
    .bind(body.verb.as_str())
    .bind(body.due_on)
    .bind(body.due_at)
    .execute(&mut *tx)
    .await?;
    let change = if fields == [TaskEditedField::Due] {
        TaskChange::DueChanged
    } else {
        TaskChange::Edited
    };
    let view = finish(
        &state,
        &mut tx,
        access,
        path.task_id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change,
            history: None,
            detail: TaskHistoryDetail {
                fields,
                due_on: body.due_on,
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
