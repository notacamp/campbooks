//! `POST /api/v1/w/{workspace_id}/identity/held-grants/{id}/adopt` — connect
//! the mailbox the caller's sign-up granted to this workspace, with no new
//! consent: the grant goes to the connectors module through the kernel's
//! grant-intake port, and is forgotten here once it is a connection.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    connections::ProviderGrant,
    tenancy::{Workspace, WorkspaceId},
};
use secrecy::SecretString;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{deps::Deps, domain::held_grants, scopes::IdentityWrite};

/// The connection the mailbox became.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Adopted {
    /// The connection's id (a `connectors.connection`).
    pub connection: String,
}

/// Connect a held mailbox to this workspace.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/identity/held-grants/{id}/adopt",
    operation_id = "identity.adopt_held_grant",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.held_grant"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("id" = Uuid, Path, description = "The held grant."),
    ),
    responses(
        (status = OK, description = "Connected.", body = Adopted),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not a member of the workspace, or the grant is not \
            the caller's, or gone.", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The connectors module refused it (e.g. \
            `account_connected_by_another_member`, `mailbox_unsupported`).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<IdentityWrite>,
    Path(path): Path<(WorkspaceId, Uuid)>,
) -> Result<Json<Adopted>> {
    let id = path.1;
    let (issuer, account) = super::account(&deps, &workspace.principal)?;
    let now = issuer.now();
    let mut tx = state.db.begin().await?;
    let taken = held_grants::take(&mut tx, &issuer.cipher, id, account, now)
        .await?
        .ok_or(Error::NotFound)?;
    let grant = ProviderGrant {
        workspace: workspace.id,
        subject: workspace.principal.subject.clone(),
        provider: taken.grant.provider.clone(),
        account_subject: taken.account_subject,
        address: taken.grant.address.clone(),
        address_verified: taken.address_verified,
        display_name: taken.grant.display_name.clone(),
        scopes: taken.tokens.scopes.clone(),
        access_token: SecretString::from(taken.tokens.access_token.clone()),
        refresh_token: taken.tokens.refresh_token.clone().map(SecretString::from),
        expires_in: taken.tokens.expires_in,
    };
    // The grant is deleted only once it is a connection: if the intake
    // refuses, the row stays (rolled back) and the person can retry.
    let connection = state.grants.adopt(&state, grant).await?;
    held_grants::delete(&mut tx, id, account).await?;
    tx.commit().await?;
    Ok(Json(Adopted { connection }))
}
