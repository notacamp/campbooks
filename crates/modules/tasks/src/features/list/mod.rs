//! `GET /api/v1/w/{workspace_id}/tasks` — the caller's tasks, overdue first,
//! then by due date and time, then undated.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        MAX_OFFSET_MINUTES, SELECT_COLUMNS, TaskId, TaskRow, TaskState, TaskView,
        resources::TASK_RECORD, today,
    },
    scopes::TasksRead,
};

/// Whose tasks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskWhose {
    /// Assigned to the caller (the default).
    #[default]
    Mine,
    /// Open tasks the caller handed to someone else.
    HandedOff,
}

/// Which tasks.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// `open` (the default), `done` or `not_needed`. Ignored when `ids`
    /// names the tasks, unless given.
    #[serde(default)]
    #[param(inline)]
    pub state: Option<TaskState>,
    /// Due on or after this day.
    #[serde(default)]
    pub due_from: Option<NaiveDate>,
    /// Due on or before this day.
    #[serde(default)]
    pub due_to: Option<NaiveDate>,
    /// Only open tasks whose due day has passed.
    #[serde(default)]
    pub overdue: Option<bool>,
    /// Only tasks with no due day.
    #[serde(default)]
    pub undated: Option<bool>,
    /// Only tasks from this record type (with `source_id`).
    #[serde(default)]
    pub source_type: Option<String>,
    /// Only tasks from this record.
    #[serde(default)]
    pub source_id: Option<String>,
    /// `mine` (default) or `handed_off`.
    #[serde(default)]
    #[param(inline)]
    pub whose: Option<TaskWhose>,
    /// 1 to 100 task ids, separated by commas: Home's batch read. Tasks the
    /// caller cannot see are left out.
    #[serde(default)]
    pub ids: Option<String>,
    /// The caller's offset from UTC, in minutes (-840 to 840; default 0):
    /// where their day starts, for what is overdue.
    #[serde(default)]
    pub utc_offset: i32,
}

/// The caller's tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskList {
    /// In order: overdue first, then by due date and time, then undated.
    pub tasks: Vec<TaskView>,
}

/// Tasks by the filters: `$2` the caller; `$3` ids (or none); `$4` whose
/// (`mine`, `handed_off` or `any`); `$5` state; `$6`/`$7` due window; `$8`
/// overdue; `$9` the caller's today; `$10` undated; `$11`/`$12` source.
const LIST_TAIL: &str = " FROM tasks.tasks
      WHERE workspace_id = $1
        AND ($3::uuid[] IS NULL OR id = ANY($3))
        AND (($4 = 'mine' AND assignee = $2)
             OR ($4 = 'handed_off' AND handed_by = $2 AND assignee <> $2 AND state = 'open')
             OR ($4 = 'any' AND (assignee = $2 OR handed_by = $2)))
        AND ($5::text IS NULL OR state = $5)
        AND ($6::date IS NULL OR due_on >= $6)
        AND ($7::date IS NULL OR due_on <= $7)
        AND ($8::bool IS NOT TRUE OR (state = 'open' AND due_on < $9))
        AND ($10::bool IS NOT TRUE OR due_on IS NULL)
        AND ($11::text IS NULL OR source_type = $11)
        AND ($12::text IS NULL OR source_id = $12)
      ORDER BY (state = 'open' AND due_on IS NOT NULL AND due_on < $9) DESC,
               due_on ASC NULLS LAST, due_at ASC NULLS LAST, created_at ASC, id
      LIMIT 500";

/// List your tasks.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/tasks",
    operation_id = "tasks.list",
    tag = "tasks",
    security(("bearer" = ["tasks:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListQuery),
    responses(
        (status = OK, description = "The caller's tasks.", body = TaskList),
        (status = BAD_REQUEST, description = "Ids that are not 1 to 100 task ids (`invalid_ids`), or an offset beyond 14 hours (`invalid_offset`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<TasksRead>,
    extract::Query(query): extract::Query<ListQuery>,
) -> Result<Json<TaskList>> {
    if query.utc_offset.abs() > MAX_OFFSET_MINUTES {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_offset")));
    }
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Read, TASK_RECORD, || {
        Error::Forbidden
    })?;
    let ids = query.ids.as_deref().map(parse_ids).transpose()?;
    let Some(user) = access.user else {
        return Ok(Json(TaskList { tasks: Vec::new() }));
    };
    let now = Utc::now();
    let today = today(now, query.utc_offset);
    // Asking for tasks by id looks at every task the caller can see, in any
    // state; otherwise the open tasks assigned to them.
    let by_ids = ids.is_some();
    let whose = match (query.whose, by_ids) {
        (Some(TaskWhose::HandedOff), _) => "handed_off",
        (_, true) => "any",
        _ => "mine",
    };
    let state_filter = match (query.state, by_ids) {
        (Some(state), _) => Some(state),
        (None, true) => None,
        (None, false) => Some(TaskState::Open),
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    let sql = format!("SELECT {SELECT_COLUMNS}{LIST_TAIL}");
    let rows: Vec<TaskRow> = sqlx::query_as(&sql)
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(ids)
        .bind(whose)
        .bind(state_filter.map(TaskState::as_str))
        .bind(query.due_from)
        .bind(query.due_to)
        .bind(query.overdue)
        .bind(today)
        .bind(query.undated)
        .bind(query.source_type.as_deref())
        .bind(query.source_id.as_deref())
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let tasks = rows
        .iter()
        .map(|row| row.view(now, today))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(TaskList { tasks }))
}

/// 1 to 100 task ids separated by commas.
fn parse_ids(raw: &str) -> Result<Vec<TaskId>> {
    let ids = raw
        .split(',')
        .map(|id| id.trim().parse::<TaskId>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::BadRequest(crate::MESSAGES.reason("invalid_ids")))?;
    if ids.is_empty() || ids.len() > 100 {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_ids")));
    }
    Ok(ids)
}

#[cfg(test)]
mod tests;
