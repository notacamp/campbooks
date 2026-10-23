//! `POST /api/v1/w/{workspace_id}/tasks` — a member writes a task, or makes
//! one from a conversation they can read.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    records::is_record_type,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        Change, HistoryDetail, Outcome, TaskId, TaskView, Verb, actor_of, clean_sender,
        clean_title, finish, resources::TASK_RECORD,
    },
    scopes::TasksWrite,
};

/// A new task.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTask {
    /// What to do: 1 to 200 characters.
    pub title: String,
    /// Do (the default) or Pay.
    #[serde(default)]
    pub verb: Option<Verb>,
    /// The day it is due.
    #[serde(default)]
    pub due_on: Option<NaiveDate>,
    /// The time it is due; needs `due_on`, its local day.
    #[serde(default)]
    pub due_at: Option<DateTime<Utc>>,
    /// The record it comes from (a conversation the caller can read).
    #[serde(default)]
    pub source: Option<RecordRef>,
    /// The address of the source's sender, kept to recognise the same
    /// sender's other tasks.
    #[serde(default)]
    pub source_sender: Option<String>,
}

/// Make a task.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tasks",
    operation_id = "tasks.create",
    tag = "tasks",
    security(("bearer" = ["tasks:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("tasks.not_needed")
        .automation()
        .assistant()
        .records(["tasks.task"]).access("tasks.task", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = NewTask,
    responses(
        (status = CREATED, description = "The task.", body = TaskView),
        (status = BAD_REQUEST, description = "A blank or too long title (`invalid_title`), a due time without a day, or a bad sender address.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or a source you cannot see.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<TasksWrite>,
    extract::Json(body): extract::Json<NewTask>,
) -> Result<(StatusCode, Json<TaskView>)> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Create, TASK_RECORD, || {
        Error::Forbidden
    })?;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let title = clean_title(&body.title)?;
    if body.due_at.is_some() && body.due_on.is_none() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("due_time_needs_day"),
        ));
    }
    let sender = body
        .source_sender
        .as_deref()
        .map(clean_sender)
        .transpose()?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    // A source needs `read` on it, as the caller: otherwise it is not there.
    let mut container = None;
    let source = body.source;
    if let Some(source) = &source {
        if !is_record_type(&source.record_type) {
            return Err(Error::NotFound);
        }
        let facts = authz::facts_of(&state, &mut tx, access, source).await?;
        authz::authorize(&state, &mut tx, access, Action::Read, source)
            .await?
            .or_else(|| Error::NotFound)?;
        container = facts
            .and_then(|facts| facts.container)
            .map(|container| container.id);
    } else if body.source_sender.is_some() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_source_sender"),
        ));
    }
    let verb = body.verb.unwrap_or(Verb::Do);
    let id = TaskId::new();
    sqlx::query(
        "INSERT INTO tasks.tasks
             (id, workspace_id, title, verb, origin, due_on, due_at, assignee, created_by,
              source_type, source_id, source_container, source_sender)
         VALUES ($1, $2, $3, $4, 'own', $5, $6, $7, $7, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(&title)
    .bind(verb.as_str())
    .bind(body.due_on)
    .bind(body.due_at)
    .bind(creator.as_uuid())
    .bind(source.as_ref().map(|source| source.record_type.clone()))
    .bind(source.as_ref().map(|source| source.id.clone()))
    .bind(container)
    .bind(sender)
    .execute(&mut *tx)
    .await?;
    let view = finish(
        &state,
        &mut tx,
        access,
        id,
        &Outcome {
            actor: actor_of(workspace.invoked_by),
            change: Change::Created,
            detail: HistoryDetail {
                due_on: body.due_on,
                ..HistoryDetail::default()
            },
            previous_assignee: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(view)))
}

#[cfg(test)]
mod tests;
