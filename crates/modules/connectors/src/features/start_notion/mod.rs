//! `POST /api/v1/w/{workspace_id}/connections/notion/authorizations` — start
//! connecting a member's Notion account. Notion returns no ID token, so the
//! consent does not go through the identity module's front door: this
//! records it and answers where to send the browser (`notion_connect`).
//!
//! Only a member starts one (a client token never does), for a connection
//! of theirs when it is to connect again. The answer is this instance's
//! `/auth/start/notion?ticket=…`: the ticket is the consent sealed with the
//! instance key, good once and for ten minutes.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections,
        model::{ConnectionId, Provider},
        notion as notion_db,
    },
    events::CONNECTION_RECORD,
    features::{
        notion_connect::{safe_return_to, ticket},
        start_connection::acting_member,
    },
    notion::START_PATH,
    scopes::ConnectorsWrite,
};

/// What to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartNotion {
    /// An existing Notion connection of yours to connect again; the same
    /// Notion account must consent.
    #[serde(default)]
    pub connection: Option<ConnectionId>,
    /// A path of this app to come back to, with `connected=<connection>` or
    /// `link_error=<code>` added.
    #[serde(default)]
    pub return_to: Option<String>,
}

/// A started consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StartedNotion {
    /// Where to send the browser: this instance's `/auth/start/notion`, which
    /// goes on to Notion and comes back to `return_to`.
    pub authorize_url: String,
    /// When the consent must be started by.
    pub expires_at: DateTime<Utc>,
}

/// Start connecting Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/notion/authorizations",
    operation_id = "connectors.start_notion",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).secret_once().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = StartNotion,
    responses(
        (status = CREATED, description = "Send the browser to `authorize_url`.", body = StartedNotion),
        (status = BAD_REQUEST, description = "A `return_to` that is not a path of this app (`invalid_return_to`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a client token (`connection_needs_user`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or no such Notion connection of yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Notion is not set up on this instance (`provider_not_configured`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<StartNotion>,
) -> Result<(StatusCode, Json<StartedNotion>)> {
    let started = execute(&state, &deps, &workspace.access, input).await?;
    Ok((StatusCode::CREATED, Json(started)))
}

/// Start connecting Notion for the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    input: StartNotion,
) -> Result<StartedNotion> {
    let user = acting_member(access)?;
    // A connection is owned by the member who makes it (ADR 0017).
    authz::require_type(state, access, Action::Create, CONNECTION_RECORD, || {
        Error::Denied(MESSAGES.reason("connection_needs_user"))
    })?;
    if deps.notion.is_none() || deps.cipher.is_none() {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    }
    if input
        .return_to
        .as_deref()
        .is_some_and(|path| !safe_return_to(path))
    {
        return Err(Error::BadRequest(MESSAGES.reason("invalid_return_to")));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    if let Some(connection) = input.connection {
        let existing = connections::load(&mut tx, connection).await?;
        // Only the grantor connects their own Notion again.
        if !existing.is_some_and(|c| c.granted_by == user && c.provider == Provider::Notion) {
            return Err(Error::NotFound);
        }
    }
    let started = notion_db::start(
        &mut tx,
        access.workspace,
        user,
        input.connection,
        input.return_to.as_deref(),
        deps.clock.now(),
    )
    .await?;
    tx.commit().await?;
    let ticket = ticket(state, access.workspace, started.id)?;
    tracing::info!(workspace = %access.workspace, %user, "notion connection started");
    Ok(StartedNotion {
        authorize_url: format!("{START_PATH}?ticket={ticket}"),
        expires_at: started.expires_at,
    })
}

#[cfg(test)]
mod tests;
