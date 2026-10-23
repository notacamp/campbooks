//! `GET /api/v1/w/{workspace_id}/tasks/{task_id}` — one task, with what
//! changed in it.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{HistoryRow, OffsetQuery, TaskHistoryEntry, TaskId, TaskView, access},
    scopes::TasksRead,
};

/// `/api/v1/w/{workspace_id}/tasks/{task_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct TaskPath {
    /// The task.
    pub task_id: TaskId,
}

/// A task and its history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    /// The task.
    pub task: TaskView,
    /// What changed in it, oldest first.
    pub history: Vec<TaskHistoryEntry>,
}

/// Read a task.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/tasks/{task_id}",
    operation_id = "tasks.get",
    tag = "tasks",
    security(("bearer" = ["tasks:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("task_id" = TaskId, Path, description = "The task."),
        OffsetQuery,
    ),
    responses(
        (status = OK, description = "The task and its history.", body = TaskDetail),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such task, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<TasksRead>,
    extract::Path(path): extract::Path<TaskPath>,
    extract::Query(offset): extract::Query<OffsetQuery>,
) -> Result<Json<TaskDetail>> {
    let access = &workspace.access;
    let today = offset.today()?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_task(&state, &mut tx, access, Action::Read, path.task_id, false).await?;
    let history: Vec<HistoryRow> = sqlx::query_as(
        "SELECT id, at, actor, actor_member, what, detail FROM tasks.history
          WHERE task_id = $1 ORDER BY at, id",
    )
    .bind(path.task_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(TaskDetail {
        task: row.view(Utc::now(), today)?,
        history: history
            .into_iter()
            .map(HistoryRow::entry)
            .collect::<Result<Vec<_>>>()?,
    }))
}

#[cfg(test)]
mod tests;
