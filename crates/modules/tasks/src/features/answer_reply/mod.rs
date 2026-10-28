//! `POST …/tasks/{task_id}/reply_answered` and `/reply_unanswered` — answer
//! "You replied. Is this done?" with **Not yet**, and take that back
//! (campbooks-cloud#396). Done is the existing `tasks.done`.
//!
//! Not yet stamps `reply_answered_at` (now, or the reply's own time when a
//! provider's clock runs ahead of ours): the question is asked again only
//! after a newer reply. Taking it back clears the stamp, so the question
//! returns. Both are the member's own act: Scout never answers for them.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        OffsetQuery, Outcome, TaskActor, TaskChange, TaskHistoryDetail, TaskId, TaskRevision,
        TaskView, access, finish,
    },
    features::get::TaskPath,
    scopes::TasksWrite,
};

/// Answer "Is this done?" with Not yet: the task stays open and the
/// question is not asked again until the member replies again.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/reply_answered",
    operation_id = "tasks.answer_reply",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.unanswer_reply")
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
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), it is closed (`task_closed`) or it asks nothing (`no_reply_question`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn answer(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    set(&state, &workspace, path.task_id, body, offset, true)
        .await
        .map(Json)
}

/// Take back a Not yet: the question is asked again.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/reply_unanswered",
    operation_id = "tasks.unanswer_reply",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
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
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), it is closed (`task_closed`) or no Not yet was given (`no_reply_question`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unanswer(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    set(&state, &workspace, path.task_id, body, offset, false)
        .await
        .map(Json)
}

/// Answer (`answered`) or take the answer back for task `id`.
async fn set(
    state: &AppState,
    workspace: &Workspace<TasksWrite>,
    id: TaskId,
    body: TaskRevision,
    offset: OffsetQuery,
    answered: bool,
) -> Result<TaskView> {
    let access = &workspace.access;
    let today = offset.today()?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = access::require_task(state, &mut tx, access, Action::Write, id, true).await?;
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    // Only a question that is asked can be answered; only an answer given
    // can be taken back.
    let askable = if answered {
        row.asks_done()
    } else {
        row.reply_answered_at.is_some() && row.reply().is_some()
    };
    if !askable {
        return Err(Error::Conflict(crate::MESSAGES.reason("no_reply_question")));
    }
    sqlx::query(
        "UPDATE tasks.tasks
            SET reply_answered_at = CASE WHEN $2 THEN greatest(now(), replied_at) ELSE NULL END,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(answered)
    .execute(&mut *tx)
    .await?;
    let view = finish(
        state,
        &mut tx,
        access,
        id,
        &Outcome {
            actor: TaskActor::Member,
            change: if answered {
                TaskChange::ReplyAnswered
            } else {
                TaskChange::ReplyUnanswered
            },
            history: None,
            detail: TaskHistoryDetail::default(),
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        today,
    )
    .await?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests;
