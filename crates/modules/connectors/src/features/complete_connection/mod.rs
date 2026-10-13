//! `POST /api/v1/w/{workspace_id}/connections/authorizations/complete` —
//! finish a consent: called by the web app's callback page, as the member
//! who started it, with the `state` and `code` Google redirected with.
//!
//! The authorization is single use and bound to that member and workspace:
//! anyone else gets 404 and the code is never exchanged, so a consent can
//! never land in someone else's workspace. Completing exchanges the code
//! (PKCE), reads the account, stores the credentials sealed, creates the
//! streams of newly granted capabilities and starts syncing them.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk, crypto,
    db::BeginFor as _,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections::parse_capabilities,
        grants::{ProviderGrant, create_or_refresh_connection},
        model::{AuthorizationId, Capability, ConnectionId, ConnectionView, Provider},
    },
    features::start_connection::{acting_member, open_verifier},
    google::{self, GoogleError},
    scopes::ConnectorsWrite,
};

/// What Google redirected the browser with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteConnection {
    /// The `state` query parameter.
    pub state: String,
    /// The `code` query parameter.
    pub code: String,
}

/// Complete a connection.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/authorizations/complete",
    operation_id = "connectors.complete_connection",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["connectors.connection"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = CompleteConnection,
    responses(
        (status = OK, description = "The connection, syncing.", body = ConnectionView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope; a client token \
            (`connection_needs_user`); no capability granted (`consent_incomplete`); or no \
            offline access (`no_offline_access`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such started connection of yours in this \
            workspace, or it expired.", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The code was refused (`authorization_code_invalid`), \
            another account consented (`account_mismatch`), another member connected this \
            account (`account_connected_by_another_member`), or the provider is not \
            configured.", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The provider could not be reached \
            (`provider_unavailable`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<CompleteConnection>,
) -> Result<Json<ConnectionView>> {
    execute(&state, &deps, &workspace.access, input)
        .await
        .map(Json)
}

/// A started consent, taken (single use).
struct Started {
    id: AuthorizationId,
    provider: Provider,
    capabilities: Vec<Capability>,
    connection: Option<ConnectionId>,
    verifier: Vec<u8>,
}

/// Complete the consent `input` for the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    input: CompleteConnection,
) -> Result<ConnectionView> {
    let user = acting_member(access)?;
    let (Some(google), Some(cipher)) = (&deps.google, &deps.cipher) else {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    };
    let now = deps.clock.now();
    let started = take(state, access.workspace, user, &input.state, now)
        .await?
        .ok_or(Error::NotFound)?;
    let verifier = open_verifier(cipher, started.id, &started.verifier).ok_or(Error::NotFound)?;
    let grant = google
        .exchange(&input.code, &verifier)
        .await
        .map_err(|err| match err {
            GoogleError::InvalidGrant => {
                Error::Conflict(MESSAGES.reason("authorization_code_invalid"))
            }
            other => provider_error(&other),
        })?;
    let account = google
        .account(&grant.access_token)
        .await
        .map_err(|err| provider_error(&err))?;
    let granted: Vec<Capability> = google::granted_capabilities(&grant.scopes())
        .into_iter()
        .filter(|c| started.capabilities.contains(c))
        .collect();
    if granted.is_empty() {
        return Err(Error::Denied(MESSAGES.reason("consent_incomplete")));
    }
    let view = create_or_refresh_connection(
        state,
        deps,
        &ProviderGrant {
            workspace: access.workspace,
            user,
            provider: started.provider,
            account_subject: account.sub,
            address: account.email.trim().to_lowercase(),
            display_name: account.name,
            capabilities: granted,
            tokens: grant,
            extending: started.connection,
        },
    )
    .await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %view.id, "connection completed");
    Ok(view)
}

/// A provider failure, as the member sees it.
fn provider_error(err: &GoogleError) -> Error {
    tracing::warn!(error = %err, "completing a connection failed at the provider");
    Error::Unavailable(MESSAGES.reason("provider_unavailable"))
}

/// Take the caller's unexpired authorization with `state`, consuming it.
async fn take(
    state: &AppState,
    workspace: WorkspaceId,
    user: UserId,
    state_token: &str,
    now: DateTime<Utc>,
) -> Result<Option<Started>> {
    let mut tx = state.db.begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"DELETE FROM connectors.authorizations
            WHERE state_hash = $1 AND user_id = $2
        RETURNING id AS "id: AuthorizationId", provider, capabilities,
                  connection_id AS "connection: ConnectionId", pkce_verifier, expires_at"#,
        crypto::hash(state_token),
        user as UserId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row.filter(|row| row.expires_at > now) else {
        return Ok(None);
    };
    Ok(Some(Started {
        id: row.id,
        provider: Provider::parse(&row.provider)?,
        capabilities: parse_capabilities(&row.capabilities)?,
        connection: row.connection,
        verifier: row.pkce_verifier,
    }))
}

#[cfg(test)]
mod tests;
