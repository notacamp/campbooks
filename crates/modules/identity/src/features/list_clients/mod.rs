//! `GET /api/v1/w/{workspace_id}/identity/clients` — the workspace's own
//! apps (owners and admins). Secrets are never shown again.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    deps::Deps,
    domain::workspace_clients::{self, WorkspaceClient},
    scopes::IdentityClients,
};

/// The workspace's apps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct WorkspaceClients {
    /// Oldest first.
    pub items: Vec<WorkspaceClient>,
}

/// List the workspace's own apps.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/identity/clients",
    operation_id = "identity.list_clients",
    tag = "identity",
    security(("bearer" = ["identity:clients"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.client"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The workspace's apps.", body = WorkspaceClients),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:clients`, or the caller \
            is not an owner or admin (`not_workspace_admin`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not a member of the workspace.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<IdentityClients>,
) -> Result<Json<WorkspaceClients>> {
    deps.issuer.as_ref().ok_or(Error::NotFound)?;
    workspace_clients::require_admin(&workspace.access)?;
    let mut tx = state.db.begin_for(workspace.id).await?;
    let items = workspace_clients::list(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(WorkspaceClients { items }))
}
