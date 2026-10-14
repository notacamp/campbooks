//! `POST /api/v1/w/{workspace_id}/identity/clients` — register an app of
//! the workspace's own (owners and admins, after a recent sign-in). A
//! confidential app's secret is in this answer and nowhere else, ever.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract::Json as JsonBody,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        accounts, client_names,
        clients::{is_registrable_redirect, third_party_may_hold},
        step_up,
        workspace_clients::{self, ClientType, NewClient, WorkspaceClient},
    },
    events::ClientRegistered,
    metadata::MAX_REDIRECT_URIS,
    scopes::IdentityClients,
};

/// What to register.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterClient {
    /// Its name, 1 to 100 characters.
    pub name: String,
    /// Confidential (server to server, with a secret) or public (a sign-in
    /// app).
    #[serde(rename = "type")]
    pub client_type: ClientType,
    /// The API scopes it may ask for (never `identity:*`).
    pub scopes: Vec<String>,
    /// A public app's redirect URIs: 1 to 10, `https` or loopback `http`.
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

/// The new app, with its secret for the only time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RegisteredClient {
    /// The app.
    pub client: WorkspaceClient,
    /// A confidential app's secret: shown now, never again.
    pub client_secret: Option<String>,
}

/// Register an app of the workspace's own.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/identity/clients",
    operation_id = "identity.register_client",
    tag = "identity",
    security(("bearer" = ["identity:clients"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.client"]).access("identity.client", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = RegisterClient,
    responses(
        (status = CREATED, description = "Registered; a confidential app's secret is shown this \
            once.", body = RegisteredClient),
        (status = BAD_REQUEST, description = "A bad name, `client_name_reserved` (it reads as \
            Campbooks), `client_scope_not_allowed` or `client_needs_redirect`.", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:clients`, the caller is not \
            an owner or admin (`not_workspace_admin`), or the sign-in is more than ten minutes \
            old (`step_up_required`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not a member of the workspace.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<IdentityClients>,
    JsonBody(body): JsonBody<RegisterClient>,
) -> Result<(StatusCode, Json<RegisteredClient>)> {
    let (issuer, account) = super::account(&deps, &workspace.principal)?;
    workspace_clients::require_admin(
        &state,
        &workspace.access,
        campbooks_kernel::authz::Action::Create,
    )?;
    let now = issuer.now();
    step_up::require_recent(&workspace.principal, now)?;
    let name = client_names::clean(&body.name);
    if name.is_empty() || body.name.trim().chars().count() > client_names::MAX_NAME {
        return Err(Error::BadRequest(
            MESSAGES
                .reason("invalid_client_metadata")
                .arg("reason", "the name must be 1 to 100 characters"),
        ));
    }
    if client_names::impersonates_campbooks(&name) {
        return Err(Error::BadRequest(MESSAGES.reason("client_name_reserved")));
    }
    let name = name.as_str();
    let mut scopes: Vec<String> = Vec::new();
    for scope in body.scopes {
        let known = issuer.api_scopes.iter().any(|s| s.name == scope);
        if !known || !third_party_may_hold(&scope) {
            return Err(Error::BadRequest(
                MESSAGES
                    .reason("client_scope_not_allowed")
                    .arg("scope", scope),
            ));
        }
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }
    let redirects_ok = match body.client_type {
        ClientType::Confidential => body.redirect_uris.is_empty(),
        ClientType::Public => {
            (1..=MAX_REDIRECT_URIS).contains(&body.redirect_uris.len())
                && body
                    .redirect_uris
                    .iter()
                    .all(|uri| is_registrable_redirect(uri))
        }
    };
    if !redirects_ok {
        return Err(Error::BadRequest(MESSAGES.reason("client_needs_redirect")));
    }
    let mut tx = state.db.begin_for(workspace.id).await?;
    let (client, client_secret) = workspace_clients::insert(
        &mut tx,
        &NewClient {
            workspace: *workspace.id.as_uuid(),
            created_by: account,
            name,
            client_type: body.client_type,
            scopes: &scopes,
            redirect_uris: &body.redirect_uris,
        },
        now,
    )
    .await?;
    accounts::audit(
        &mut tx,
        Some(account),
        "client_registered",
        serde_json::json!({ "client": client.client_id, "kind": "workspace", "workspace": workspace.id }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &ClientRegistered {
                workspace: workspace.id.to_string(),
                client: client.client_id.clone(),
                kind: body.client_type.as_str().to_owned(),
            },
        )
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(RegisteredClient {
            client,
            client_secret,
        }),
    ))
}
