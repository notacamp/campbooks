//! `POST /api/v1/w/{workspace_id}/connections/{connection_id}/sync` — read
//! the connection's streams now and keep them at the fastest interval. The
//! web app calls it when a member opens what a connection feeds.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections,
        model::{ConnectionId, ConnectionPath, Health, StreamId},
        schedule,
    },
    events::CONNECTION_RECORD,
    scopes::ConnectorsWrite,
};

/// What was scheduled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncScheduled {
    /// How many streams will run now.
    pub streams: u32,
}

/// Sync a connection now.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}/sync",
    operation_id = "connectors.sync_now",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).automation().assistant().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The connection."),
    ),
    responses(
        (status = ACCEPTED, description = "Its streams run now; a run already waiting absorbs \
            this one.", body = SyncScheduled),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The member must connect it again first \
            (`needs_reauth`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Path(path): Path<ConnectionPath>,
) -> Result<(StatusCode, Json<SyncScheduled>)> {
    let scheduled = execute(&state, &deps, &workspace.access, path.connection_id).await?;
    Ok((StatusCode::ACCEPTED, Json(scheduled)))
}

/// Run connection `id`'s streams now.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
) -> Result<SyncScheduled> {
    let now = deps.clock.now();
    let mut tx = state.db.begin_for(access.workspace).await?;
    authz::require(
        state,
        &mut tx,
        access,
        Action::Write,
        &RecordRef::new(CONNECTION_RECORD, &id),
        || Error::Forbidden,
    )
    .await?;
    let connection = connections::load(&mut tx, id).await?.ok_or(Error::NotFound)?;
    if connection.health == Health::NeedsReauth {
        return Err(Error::Conflict(MESSAGES.reason("needs_reauth")));
    }
    let min = i32::try_from(deps.config.poll.min_seconds).unwrap_or(i32::MAX);
    let streams = sqlx::query_scalar!(
        r#"UPDATE connectors.streams SET interval_seconds = $2
            WHERE connection_id = $1 AND NOT paused
        RETURNING id AS "id: StreamId""#,
        id as ConnectionId,
        min,
    )
    .fetch_all(&mut *tx)
    .await?;
    for &stream in &streams {
        schedule::run_now(&mut tx, access.workspace, stream, now).await?;
    }
    tx.commit().await?;
    Ok(SyncScheduled {
        streams: u32::try_from(streams.len()).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests;
