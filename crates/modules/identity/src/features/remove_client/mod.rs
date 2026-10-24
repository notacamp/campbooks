//! `DELETE /api/v1/w/{workspace_id}/identity/clients/{client_id}` — remove
//! one of the workspace's apps (owners and admins). Its refresh tokens and
//! every consent people gave it end at once; access tokens live out their
//! ten minutes.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};

use super::rotate_client_secret::ClientPath;
use crate::{
    deps::Deps,
    domain::{accounts, workspace_clients},
    events::{ClientRemoved, ConsentRevoked},
    scopes::IdentityClients,
    tracking,
};

/// Remove one of the workspace's apps.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/identity/clients/{client_id}",
    operation_id = "identity.remove_client",
    tag = "identity",
    security(("bearer" = ["identity:clients"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.client"]).access("identity.client", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("client_id" = String, Path, description = "The app."),
    ),
    responses(
        (status = NO_CONTENT, description = "Removed; its tokens stop working."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:clients`, or the caller is \
            not an owner or admin (`not_workspace_admin`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such app in the workspace.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<IdentityClients>,
    Path(path): Path<ClientPath>,
) -> Result<StatusCode> {
    let (issuer, account) = super::account(&deps, &workspace.principal)?;
    workspace_clients::require_admin(
        &state,
        &workspace.access,
        campbooks_kernel::authz::Action::Delete,
    )?;
    let now = issuer.now();
    let mut tx = state.db.begin_for(workspace.id).await?;
    if !workspace_clients::remove(&mut tx, &path.client_id).await? {
        return Err(Error::NotFound);
    }
    let ended =
        workspace_clients::revoke_everything(&mut tx, &path.client_id, "client_removed", now)
            .await?;
    accounts::audit(
        &mut tx,
        Some(account),
        "client_removed",
        serde_json::json!({ "client": path.client_id, "workspace": workspace.id }),
        now,
    )
    .await?;
    state
        .tracker
        .track(
            &mut tx,
            &tracking::ClientRemoved {
                workspace: workspace.id,
                client_id: path.client_id.clone(),
                cause: tracking::ClientRemoval::Removed,
                consents_revoked: i64::try_from(ended.len()).unwrap_or(i64::MAX),
            },
        )
        .await?;
    for (person, bound) in ended {
        state
            .events
            .publish(
                &mut tx,
                &ConsentRevoked {
                    account: person,
                    client: path.client_id.clone(),
                    workspace: bound.map(|w| w.to_string()),
                    reason: "client_removed".to_owned(),
                },
            )
            .await?;
    }
    state
        .events
        .publish(
            &mut tx,
            &ClientRemoved {
                workspace: workspace.id.to_string(),
                client: path.client_id,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
