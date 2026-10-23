//! `POST …/tasks/{task_id}/hand_off` and `/take_back` — give a task to one
//! teammate, and have it back. Tasks stay personal: the assignee holds it,
//! and the member who handed it off still sees it and may take it back.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        Change, HistoryDetail, Outcome, TaskId, TaskRevision, TaskView, access, actor_of, finish,
    },
    features::get::TaskPath,
    scopes::TasksWrite,
};

/// Who to hand it to.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HandOffTo {
    /// The revision you saw.
    pub revision: i64,
    /// A member of the workspace.
    pub to: UserId,
}

/// Hand an open task to a teammate.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/hand_off",
    operation_id = "tasks.hand_off",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.take_back")
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
    ),
    request_body = HandOffTo,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
        (status = BAD_REQUEST, description = "Not a member of the workspace (`not_a_member`), or yourself (`hand_off_to_other`).",
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
pub async fn hand_off(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Json(body): extract::Json<HandOffTo>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let Some(me) = access.user else {
        return Err(Error::Forbidden);
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Write, path.task_id, true).await?;
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    if body.to == me {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("hand_off_to_other"),
        ));
    }
    // Only a member of this workspace; the check holds the membership until
    // the transaction ends.
    let role = state
        .membership
        .role_held(&mut tx, access.workspace, body.to)
        .await?;
    if role.is_none() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("not_a_member")));
    }
    sqlx::query(
        "UPDATE tasks.tasks SET assignee = $2, handed_by = $3,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(path.task_id)
    .bind(body.to)
    .bind(me)
    .execute(&mut *tx)
    .await?;
    let view = finish(
        &state,
        &mut tx,
        access,
        path.task_id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change: Change::HandedOff,
            detail: HistoryDetail {
                member: Some(body.to),
                ..HistoryDetail::default()
            },
            previous_assignee: Some(row.assignee),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Take back a task you handed to someone, while it is open.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}/take_back",
    operation_id = "tasks.take_back",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Share)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
    ),
    request_body = TaskRevision,
    responses(
        (status = OK, description = "The task afterwards.", body = TaskView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you did not hand it off (`task_not_yours`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The task changed since you saw it (`task_changed`), it is closed (`task_closed`) or was not handed to anyone (`not_handed_off`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn take_back(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Json(body): extract::Json<TaskRevision>,
) -> Result<Json<TaskView>> {
    let access = &workspace.access;
    let Some(me) = access.user else {
        return Err(Error::Forbidden);
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Share, path.task_id, true).await?;
    access::expect_revision(&row, body.revision)?;
    access::expect_open(&row)?;
    match row.handed_by {
        None => return Err(Error::Conflict(crate::MESSAGES.reason("not_handed_off"))),
        Some(by) if by != me => {
            return Err(Error::Denied(crate::MESSAGES.reason("task_not_yours")));
        }
        Some(_) => {}
    }
    sqlx::query(
        "UPDATE tasks.tasks SET assignee = $2, handed_by = NULL,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(path.task_id)
    .bind(me)
    .execute(&mut *tx)
    .await?;
    let view = finish(
        &state,
        &mut tx,
        access,
        path.task_id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change: Change::TakenBack,
            detail: HistoryDetail {
                member: Some(row.assignee),
                ..HistoryDetail::default()
            },
            previous_assignee: Some(row.assignee),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
