//! `POST /api/v1/w/{workspace_id}/connections/authorizations` — start
//! connecting an outside account: the provider's authorize URL, bound to the
//! calling member and workspace. Google sends the browser back to the web
//! app's callback page, which completes it with
//! `connectors.complete_connection`.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    crypto::{self, Cipher},
    db::BeginFor as _,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections,
        model::{AuthorizationId, Capability, ConnectionId, Provider},
    },
    google::{self, IDENTITY_SCOPES},
    scopes::ConnectorsWrite,
};

/// How long a started consent may take.
pub const AUTHORIZATION_LIFETIME: TimeDelta = TimeDelta::minutes(10);

/// What to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartConnection {
    /// The provider.
    pub provider: Provider,
    /// The capabilities to ask for (`messages` is offered today).
    pub capabilities: Vec<Capability>,
    /// An existing connection of yours to extend (incremental consent);
    /// the same account must consent.
    #[serde(default)]
    pub connection: Option<ConnectionId>,
}

/// A started consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StartedConnection {
    /// The started authorization.
    pub authorization: AuthorizationId,
    /// Where to send the browser.
    pub authorize_url: String,
    /// When the consent must be completed by.
    pub expires_at: DateTime<Utc>,
}

/// Start connecting an account.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/authorizations",
    operation_id = "connectors.start_connection",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["connectors.connection"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = StartConnection,
    responses(
        (status = CREATED, description = "Send the browser to `authorize_url`.", body = StartedConnection),
        (status = BAD_REQUEST, description = "No capability, or one that is not offered \
            (`capabilities_required`, `capability_not_offered`).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a client token \
            (`connection_needs_user`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or no such connection of yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The provider is not configured on this instance \
            (`provider_not_configured`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<StartConnection>,
) -> Result<(StatusCode, Json<StartedConnection>)> {
    let started = execute(&state, &deps, &workspace.access, input).await?;
    Ok((StatusCode::CREATED, Json(started)))
}

/// Start connecting an account for the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    input: StartConnection,
) -> Result<StartedConnection> {
    let user = acting_member(access)?;
    let (Some(google), Some(cipher)) = (&deps.google, &deps.cipher) else {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    };
    let mut capabilities = input.capabilities.clone();
    capabilities.sort_unstable();
    capabilities.dedup();
    if capabilities.is_empty() {
        return Err(Error::BadRequest(MESSAGES.reason("capabilities_required")));
    }
    let mut scopes: Vec<&str> = IDENTITY_SCOPES.to_vec();
    for &capability in &capabilities {
        let scope = google::capability_scope(capability).ok_or_else(|| {
            Error::BadRequest(
                MESSAGES
                    .reason("capability_not_offered")
                    .arg("capability", capability.as_str()),
            )
        })?;
        scopes.push(scope);
    }

    let now = deps.clock.now();
    let id = AuthorizationId::new();
    let state_token = crypto::random_token();
    let verifier = crypto::random_token();
    let expires_at = now + AUTHORIZATION_LIFETIME;
    let mut tx = state.db.begin_for(access.workspace).await?;
    if let Some(connection) = input.connection {
        let existing = connections::load(&mut tx, connection).await?;
        // Only the grantor extends their own grant.
        if !existing.is_some_and(|c| c.granted_by == user && c.provider == input.provider) {
            return Err(Error::NotFound);
        }
    }
    // Expired consents of this workspace go first.
    sqlx::query!(
        "DELETE FROM connectors.authorizations WHERE expires_at <= $1",
        now
    )
    .execute(&mut *tx)
    .await?;
    let capability_names: Vec<String> =
        capabilities.iter().map(|c| c.as_str().to_owned()).collect();
    sqlx::query!(
        r#"INSERT INTO connectors.authorizations
               (id, workspace_id, user_id, provider, capabilities, connection_id, state_hash,
                pkce_verifier, expires_at, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        id as AuthorizationId,
        access.workspace as WorkspaceId,
        user as UserId,
        input.provider.as_str(),
        &capability_names,
        input.connection as Option<ConnectionId>,
        crypto::hash(&state_token),
        seal_verifier(cipher, id, &verifier),
        expires_at,
        now,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let authorize_url =
        google.authorize_url(&scopes, &state_token, &crypto::pkce_challenge(&verifier));
    tracing::info!(workspace = %access.workspace, %user, authorization = %id, "connection started");
    Ok(StartedConnection {
        authorization: id,
        authorize_url,
        expires_at,
    })
}

/// The calling member: connections are a member's grant, so client tokens
/// cannot start or complete one.
pub fn acting_member(access: &Access) -> Result<UserId> {
    access
        .user
        .ok_or_else(|| Error::Denied(MESSAGES.reason("connection_needs_user")))
}

/// Associated data of a sealed PKCE verifier.
fn verifier_context(id: AuthorizationId) -> Vec<u8> {
    format!("connectors.authorizations:pkce_verifier:{id}").into_bytes()
}

/// Seal the PKCE verifier of authorization `id`.
pub fn seal_verifier(cipher: &Cipher, id: AuthorizationId, verifier: &str) -> Vec<u8> {
    cipher.seal(verifier, &verifier_context(id))
}

/// Open the PKCE verifier of authorization `id`.
pub fn open_verifier(cipher: &Cipher, id: AuthorizationId, sealed: &[u8]) -> Option<String> {
    cipher.open(sealed, &verifier_context(id)).ok()
}
