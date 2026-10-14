//! `POST /api/v1/w/{workspace_id}/identity/clients/{client_id}/secret` — a
//! new secret for a confidential app (owners and admins, after a recent
//! sign-in). The old one stops working at once; tokens already minted with
//! it live out their ten minutes.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};

use super::register_client::RegisteredClient;
use crate::{
    deps::Deps,
    domain::{accounts, step_up, workspace_clients},
    scopes::IdentityClients,
};

/// The app a path names.
#[derive(Debug, serde::Deserialize)]
pub struct ClientPath {
    /// The app's client id.
    pub client_id: String,
}

/// Rotate an app's secret.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/identity/clients/{client_id}/secret",
    operation_id = "identity.rotate_client_secret",
    tag = "identity",
    security(("bearer" = ["identity:clients"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.client"]).access("identity.client", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("client_id" = String, Path, description = "The app."),
    ),
    responses(
        (status = OK, description = "The new secret, shown this once.", body = RegisteredClient),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:clients`, the caller is not \
            an owner or admin, or the sign-in is more than ten minutes old \
            (`step_up_required`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such app in the workspace.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "A sign-in app has no secret (`client_is_public`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<IdentityClients>,
    Path(path): Path<ClientPath>,
) -> Result<Json<RegisteredClient>> {
    let (issuer, account) = super::account(&deps, &workspace.principal)?;
    workspace_clients::require_admin(
        &state,
        &workspace.access,
        campbooks_kernel::authz::Action::Write,
    )?;
    let now = issuer.now();
    step_up::require_recent(&workspace.principal, now)?;
    let mut tx = state.db.begin_for(workspace.id).await?;
    let (client, secret) = workspace_clients::rotate(&mut tx, &path.client_id, now)
        .await?
        .ok_or(Error::NotFound)?;
    accounts::audit(
        &mut tx,
        Some(account),
        "client_secret_rotated",
        serde_json::json!({ "client": client.client_id, "workspace": workspace.id }),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(RegisteredClient {
        client,
        client_secret: Some(secret),
    }))
}
