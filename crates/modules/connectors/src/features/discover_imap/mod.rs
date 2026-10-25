//! `POST /api/v1/w/{workspace_id}/connections/imap/discover` — find the
//! IMAP and SMTP servers of an email address: from the bundled provider
//! database, the domain's own autoconfig document, its SRV records or its
//! MX host (`imap::discover`). An address at a provider Campbooks connects
//! natively is answered with that provider instead (Microsoft only while
//! this instance offers Microsoft mail; `not_supported_yet` otherwise). A
//! read: nothing is stored, and no server is signed in to.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    auth::{Principal, step_up},
    authz::{self, Action},
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::schedule::{self, Rate},
    events::CONNECTION_RECORD,
    features::start_connection::acting_member,
    imap::{
        Server, Source,
        discover::{self, Address, Discovery, Native, PasswordKind},
    },
    scopes::ConnectorsWrite,
};

/// Discovery calls each member may make per hour.
pub const DISCOVERIES_PER_HOUR: u32 = 30;

/// The address to find servers for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscoverImap {
    /// The mailbox's email address.
    pub address: String,
}

/// What was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    /// Its servers (`imap`, `smtp`, `username_hint`, `source`, `password`).
    Settings,
    /// Its provider is connected natively: connect with `provider` instead.
    UseProvider,
    /// Its provider (`provider`) will be connected natively; not yet.
    NotSupportedYet,
    /// Nothing found: ask the member for the servers.
    NotFound,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DiscoveredImap {
    /// What was found.
    pub kind: DiscoveryKind,
    /// The IMAP server (`settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imap: Option<Server>,
    /// The SMTP server (`settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp: Option<Server>,
    /// The username the servers most likely want (`settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username_hint: Option<String>,
    /// Where the settings came from (`settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// The provider database's id of the provider, for its app-password
    /// guide (`settings`, when known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_hint: Option<String>,
    /// Which password the provider takes (`settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<PasswordKind>,
    /// The native provider (`use_provider`, `not_supported_yet`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Native>,
}

impl DiscoveredImap {
    fn of(kind: DiscoveryKind) -> Self {
        Self {
            kind,
            imap: None,
            smtp: None,
            username_hint: None,
            source: None,
            provider_hint: None,
            password: None,
            provider: None,
        }
    }
}

impl From<Discovery> for DiscoveredImap {
    fn from(discovery: Discovery) -> Self {
        match discovery {
            Discovery::Settings(settings) => Self {
                imap: Some(settings.servers.imap),
                smtp: Some(settings.servers.smtp),
                username_hint: Some(settings.username),
                source: Some(settings.source),
                provider_hint: settings.provider_hint,
                password: Some(settings.password),
                ..Self::of(DiscoveryKind::Settings)
            },
            Discovery::UseProvider(native) => Self {
                provider: Some(native),
                ..Self::of(DiscoveryKind::UseProvider)
            },
            Discovery::NotSupportedYet(native) => Self {
                provider: Some(native),
                ..Self::of(DiscoveryKind::NotSupportedYet)
            },
            Discovery::NotFound => Self::of(DiscoveryKind::NotFound),
        }
    }
}

/// Find a mailbox's servers.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/imap/discover",
    operation_id = "connectors.discover_imap",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::body_query().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = DiscoverImap,
    responses(
        (status = OK, description = "What was found: servers, a native provider to connect \
            instead, or nothing.", body = DiscoveredImap),
        (status = BAD_REQUEST, description = "Not an email address (`imap_address_invalid`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token \
            (`connection_needs_user`), or a sign-in more than ten minutes old \
            (`step_up_required`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "IMAP is not turned on on this instance \
            (`provider_not_configured`).", body = Problem,
            content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "More than 30 in an hour (`rate_limited`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<DiscoverImap>,
) -> Result<Json<DiscoveredImap>> {
    let found = execute(
        &state,
        &deps,
        &workspace.principal,
        &workspace.access,
        input,
    )
    .await?;
    Ok(Json(found))
}

/// Find `input`'s servers for `principal`, the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    principal: &Principal,
    access: &Access,
    input: DiscoverImap,
) -> Result<DiscoveredImap> {
    let user = acting_member(access)?;
    // A connection is owned by the member who makes it (ADR 0017).
    authz::require_type(state, access, Action::Create, CONNECTION_RECORD, || {
        Error::Denied(MESSAGES.reason("connection_needs_user"))
    })?;
    admit(deps, principal)?;
    let address = Address::parse(&input.address)
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("imap_address_invalid")))?;
    spend(
        state,
        &format!("imap.discover:{user}"),
        Rate::per_hour(DISCOVERIES_PER_HOUR),
    )
    .await?;
    Ok(offered(deps, discover::discover(&deps.imap, &address).await).into())
}

/// `found` as this instance connects it: an Outlook or Microsoft 365
/// address goes to Microsoft's sign-in while Microsoft mail is offered
/// (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED`), and is not supported yet
/// otherwise.
fn offered(deps: &Deps, found: Discovery) -> Discovery {
    match found {
        Discovery::NotSupportedYet(Native::Microsoft) if deps.microsoft.is_some() => {
            Discovery::UseProvider(Native::Microsoft)
        }
        Discovery::UseProvider(Native::Microsoft) if deps.microsoft.is_none() => {
            Discovery::NotSupportedYet(Native::Microsoft)
        }
        other => other,
    }
}

/// Whether a member who may create a connection may find or connect an
/// IMAP mailbox: IMAP is turned on on this instance, and they signed in
/// within the last ten minutes (a stolen session cannot hand Campbooks a
/// guessed password).
pub fn admit(deps: &Deps, principal: &Principal) -> Result<()> {
    if !deps.imap_enabled() {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    }
    step_up::require_recent(
        principal,
        deps.clock.now(),
        MESSAGES.reason("step_up_required"),
    )
}

/// Spend one unit of the budget `key`, refilled at `rate`: 429
/// `rate_limited` when it is empty.
pub async fn spend(state: &AppState, key: &str, rate: Rate) -> Result<()> {
    match schedule::take(&state.db, key, 1, rate).await? {
        Ok(()) => Ok(()),
        Err(wait) => Err(Error::RateLimited {
            retry_after: wait.as_secs().max(1),
        }),
    }
}

#[cfg(test)]
mod tests;
