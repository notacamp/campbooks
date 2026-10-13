//! `DELETE /api/v1/w/{workspace_id}/connections/{connection_id}` —
//! disconnect: revoke the grant at the provider, delete the credentials,
//! streams and index, and publish `connectors.connection_removed`. The
//! grantor, owners and admins may; undoing it needs a new consent.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};

use crate::{
    deps::Deps,
    domain::{
        connections,
        model::{ConnectionId, ConnectionPath},
        removal,
    },
    features::start_connection::acting_member,
    scopes::ConnectorsWrite,
};

/// Disconnect a connection.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}",
    operation_id = "connectors.disconnect",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    // Stops syncing for good; reconnecting needs the member's consent.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["connectors.connection"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The connection."),
    ),
    responses(
        (status = NO_CONTENT, description = "Disconnected."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a client token \
            (`connection_needs_user`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Path(path): Path<ConnectionPath>,
) -> Result<StatusCode> {
    execute(&state, &deps, &workspace.access, path.connection_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Disconnect connection `id` on behalf of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
) -> Result<()> {
    let user = acting_member(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    connections::load(&mut tx, id)
        .await?
        .filter(|c| connections::visible(access, c.granted_by))
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    removal::revoke(state, deps, access.workspace, id).await;
    let mut tx = state.db.begin_for(access.workspace).await?;
    removal::delete(&mut tx, state, access.workspace, id).await?;
    tx.commit().await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %id, "disconnected");
    Ok(())
}

#[cfg(test)]
mod tests;
