//! `GET /api/v1/w/{workspace_id}/connections/{connection_id}` — one
//! connection with its streams. 404 unless the caller may see it.

use axum::{
    Json,
    extract::{Path, State},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::{self, Action},
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};

use crate::{
    domain::{
        connections,
        model::{ConnectionId, ConnectionPath, ConnectionView},
    },
    events::CONNECTION_RECORD,
    scopes::ConnectorsRead,
};

/// Read a connection.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}",
    operation_id = "connectors.get_connection",
    tag = "connectors",
    security(("bearer" = ["connectors:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The connection."),
    ),
    responses(
        (status = OK, description = "The connection.", body = ConnectionView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConnectorsRead>,
    Path(path): Path<ConnectionPath>,
) -> Result<Json<ConnectionView>> {
    execute(&state, &workspace.access, path.connection_id)
        .await
        .map(Json)
}

/// Connection `id`, if `access` may see it.
pub async fn execute(
    state: &AppState,
    access: &Access,
    id: ConnectionId,
) -> Result<ConnectionView> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    authz::require(
        state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(CONNECTION_RECORD, &id),
        || Error::Forbidden,
    )
    .await?;
    let connection = connections::load(&mut tx, id).await?.ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection).await?;
    tx.commit().await?;
    Ok(view)
}
