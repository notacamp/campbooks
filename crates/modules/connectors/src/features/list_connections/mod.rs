//! `GET /api/v1/w/{workspace_id}/connections` — the connections the caller
//! may see: their own, or all of the workspace's for owners, admins and
//! client tokens. Never a credential.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{connections, model::ConnectionView},
    scopes::ConnectorsRead,
};

/// The caller's connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConnectionList {
    /// Oldest first.
    pub connections: Vec<ConnectionView>,
}

/// List connections.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/connections",
    operation_id = "connectors.list_connections",
    tag = "connectors",
    security(("bearer" = ["connectors:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["connectors.connection"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The connections you may see.", body = ConnectionList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConnectorsRead>,
) -> Result<Json<ConnectionList>> {
    execute(&state, &workspace.access).await.map(Json)
}

/// The connections `access` may see.
pub async fn execute(state: &AppState, access: &Access) -> Result<ConnectionList> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut views = Vec::new();
    for connection in connections::list(&mut tx).await? {
        if connections::visible(access, connection.granted_by) {
            views.push(connections::view(&mut tx, connection, access.user).await?);
        }
    }
    tx.commit().await?;
    Ok(ConnectionList { connections: views })
}

#[cfg(test)]
mod tests;
