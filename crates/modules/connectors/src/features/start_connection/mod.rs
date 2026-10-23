//! `POST /api/v1/w/{workspace_id}/connections/authorizations` — start
//! connecting an outside account. The consent itself goes through the
//! identity module's front door (ADR 0014): this checks what the connectors
//! module owns (a member, offered capabilities, a connection of theirs to
//! extend) and hands over through the kernel's consent-door port. The
//! provider redirects to the identity module, which links the account as a
//! way in and hands the grant back through the grant-intake port.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::Principal,
    authz::{self, Action},
    connections::ConsentRequest,
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
        connections,
        model::{Capability, ConnectionId, Provider},
    },
    events::CONNECTION_RECORD,
    google,
    scopes::ConnectorsWrite,
};

/// What to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartConnection {
    /// The provider.
    pub provider: Provider,
    /// The capabilities to ask for (`messages`, and `calendar` for Google,
    /// and for Zoho while its flag is on).
    pub capabilities: Vec<Capability>,
    /// An existing connection of yours to re-authorise; the same account
    /// must consent.
    #[serde(default)]
    pub connection: Option<ConnectionId>,
}

/// A started consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StartedConnection {
    /// Where to send the browser: this instance's front door
    /// (`/auth/start/{provider}?link=…`), which comes back with
    /// `connected=<connection id>` or `link_error=<code>`.
    pub authorize_url: String,
    /// When the consent must be started by.
    pub expires_at: DateTime<Utc>,
}

/// Start connecting an account.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/authorizations",
    operation_id = "connectors.start_connection",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).secret_once().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = StartConnection,
    responses(
        (status = CREATED, description = "Send the browser to `authorize_url`.", body = StartedConnection),
        (status = BAD_REQUEST, description = "No capability, or one that is not offered \
            (`capabilities_required`, `capability_not_offered`), or an IMAP mailbox, which is \
            connected with its password instead (`imap_connects_with_a_password`).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token \
            (`connection_needs_user`), or a sign-in more than ten minutes old for a new \
            connection (`step_up_required`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or no such connection of yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The provider is not configured on this instance \
            (`provider_not_configured`), or this instance has no front door for consents \
            (`mailbox_unsupported`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<StartConnection>,
) -> Result<(StatusCode, Json<StartedConnection>)> {
    let started = execute(
        &state,
        &deps,
        &workspace.principal,
        &workspace.access,
        input,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(started)))
}

/// Start connecting an account for `principal`, the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    principal: &Principal,
    access: &Access,
    input: StartConnection,
) -> Result<StartedConnection> {
    let user = acting_member(access)?;
    // A connection is owned by the member who makes it (ADR 0017).
    authz::require_type(state, access, Action::Create, CONNECTION_RECORD, || {
        Error::Denied(MESSAGES.reason("connection_needs_user"))
    })?;
    let configured = match input.provider {
        Provider::Google => deps.google.is_some(),
        Provider::Zoho => deps.zoho.is_some(),
        // An IMAP mailbox has no consent page: it is connected with its
        // password (`connect_imap`).
        Provider::Imap if deps.imap_enabled() => {
            return Err(Error::BadRequest(
                MESSAGES.reason("imap_connects_with_a_password"),
            ));
        }
        Provider::Imap => false,
    };
    if !configured || deps.cipher.is_none() {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    }
    let mut capabilities = input.capabilities.clone();
    capabilities.sort_unstable();
    capabilities.dedup();
    if capabilities.is_empty() {
        return Err(Error::BadRequest(MESSAGES.reason("capabilities_required")));
    }
    for &capability in &capabilities {
        // Zoho Calendar is unverified against a live grant: it is asked for
        // only while its flag is on, and a conflict with the instance's
        // settings (not a mistake in the request) while it is off.
        if input.provider == Provider::Zoho
            && capability == Capability::Calendar
            && !deps.config.zoho_calendar
        {
            return Err(Error::Conflict(
                MESSAGES
                    .reason("capability_not_offered")
                    .arg("capability", capability.as_str()),
            ));
        }
        let offered = match input.provider {
            Provider::Google => !google::capability_scopes(capability).is_empty(),
            Provider::Zoho => matches!(capability, Capability::Messages | Capability::Calendar),
            Provider::Imap => false,
        };
        if !offered {
            return Err(Error::BadRequest(
                MESSAGES
                    .reason("capability_not_offered")
                    .arg("capability", capability.as_str()),
            ));
        }
    }
    if let Some(connection) = input.connection {
        let mut tx = state.db.begin_for(access.workspace).await?;
        let existing = connections::load(&mut tx, connection).await?;
        tx.commit().await?;
        // Only the grantor re-authorises their own grant.
        if !existing.is_some_and(|c| c.granted_by == user && c.provider == input.provider) {
            return Err(Error::NotFound);
        }
    }
    let started = state
        .consent
        .start(
            state,
            ConsentRequest {
                principal: principal.clone(),
                workspace: access.workspace,
                provider: input.provider.as_str().to_owned(),
                connection: input.connection.map(|c| c.to_string()),
                capabilities: capabilities.iter().map(|c| c.as_str().to_owned()).collect(),
            },
        )
        .await?;
    tracing::info!(workspace = %access.workspace, %user, "connection started");
    Ok(StartedConnection {
        authorize_url: started.url,
        expires_at: started.expires_at,
    })
}

/// The calling member: connections are a member's grant, so client tokens
/// cannot start one.
pub fn acting_member(access: &Access) -> Result<UserId> {
    access
        .user
        .ok_or_else(|| Error::Denied(MESSAGES.reason("connection_needs_user")))
}

#[cfg(test)]
mod tests;
