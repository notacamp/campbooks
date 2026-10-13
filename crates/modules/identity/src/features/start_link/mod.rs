//! `POST /api/v1/identity/links` — start a signed-in round trip to a
//! provider: add a way in (`link`), connect a mailbox (`connect`, which also
//! adds it as a way in), or reconnect a mailbox that needs it (`reconnect`).
//!
//! The answer is a same-origin URL holding a single-use, two-minute ticket.
//! It works only in a browser signed in to the issuer as the same account
//! (see `protocol::link`). Adding a way in and connecting need a sign-in in
//! the last ten minutes. Reconnecting a mailbox whose grant stopped working
//! does not: only the connection's own provider account may come back, and a
//! reconnect never adds a way in. Renewing a grant that still works needs the
//! recent sign-in too.

use std::time::Duration;

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::{Principal, Unbound},
    rate_limit::{Limit, RateKey},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        links::{self, Ticket},
        step_up,
    },
    scopes::IdentityWrite,
    upstream::{Provider, Purpose},
    web,
};

/// Round trips one account may start.
const LINK_LIMIT: Limit = Limit::new("identity.start_link", 20, Duration::from_mins(10));

/// What the round trip is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkPurpose {
    /// Add a way in.
    Link,
    /// Connect a mailbox in a workspace (it becomes a way in too).
    Connect,
    /// Re-authorise a mailbox connection of yours.
    Reconnect,
}

/// The round trip to start.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartLink {
    /// What for.
    pub purpose: LinkPurpose,
    /// `google`, `microsoft` or `zoho`; for `link` and `connect`. A
    /// reconnect uses the connection's.
    #[serde(default)]
    pub provider: Option<String>,
    /// The workspace, for `connect` and `reconnect`.
    #[serde(default)]
    pub workspace_id: Option<WorkspaceId>,
    /// The connection, for `reconnect`.
    #[serde(default)]
    pub connection_id: Option<String>,
    /// For `connect`: also sign in with the account (default `true`).
    /// `false` for a shared mailbox (support@): it is connected, never
    /// added as a way in. Ignored by the other purposes.
    #[serde(default)]
    pub sign_in: Option<bool>,
    /// Same-origin path to come back to (default `/`), with `linked=`,
    /// `connected=` or `link_error=` added.
    #[serde(default)]
    pub return_to: Option<String>,
}

/// A started round trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct StartedLink {
    /// Send the browser here (same origin).
    pub url: String,
    /// It stops working then.
    pub expires_at: DateTime<Utc>,
}

/// Start adding a way in, connecting or reconnecting a mailbox.
#[utoipa::path(
    post,
    path = "/api/v1/identity/links",
    operation_id = "identity.start_link",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.link_ticket"])))),
    request_body = StartLink,
    responses(
        (status = CREATED, description = "Send the browser to `url`.", body = StartedLink),
        (status = BAD_REQUEST, description = "A field the purpose needs is missing \
            (`link_incomplete`), or `invalid_return_to`.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`, or adding a way \
            in or connecting needs a sign-in in the last ten minutes (`step_up_required`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Unknown or unconfigured provider, a workspace the \
            caller is not in, a connection the caller did not grant, or another identity \
            provider serves this instance.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "This instance cannot connect that provider's \
            mailbox (`mailbox_unsupported`).", body = Problem,
            content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "`rate_limited`.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityWrite>,
    Json(input): Json<StartLink>,
) -> Result<(StatusCode, Json<StartedLink>)> {
    let started = execute(&state, &deps, &principal, input).await?;
    Ok((StatusCode::CREATED, Json(started)))
}

/// Start the round trip `input` describes for `principal`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    principal: &Principal,
    input: StartLink,
) -> Result<StartedLink> {
    let (issuer, account) = super::account(deps, principal)?;
    let now = issuer.now();
    let return_to = web::return_to(input.return_to.as_deref())
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("invalid_return_to")))?;
    let incomplete = || Error::BadRequest(MESSAGES.reason("link_incomplete"));
    let configured = |name: &str| {
        Provider::parse(name)
            .filter(|p| issuer.upstream(*p).is_some())
            .ok_or(Error::NotFound)
    };
    let ticket = match input.purpose {
        LinkPurpose::Link => {
            let provider = configured(input.provider.as_deref().ok_or_else(incomplete)?)?;
            step_up::require_recent(principal, now)?;
            Ticket {
                account_id: account,
                provider,
                purpose: Purpose::Link,
                workspace_id: None,
                connection_id: None,
                expected_subject: None,
                login_hint: None,
                return_to,
                adds_way_in: true,
            }
        }
        LinkPurpose::Connect => {
            let provider = configured(input.provider.as_deref().ok_or_else(incomplete)?)?;
            let workspace = input.workspace_id.ok_or_else(incomplete)?;
            step_up::require_recent(principal, now)?;
            state
                .membership
                .resolve(&state.db, principal, workspace)
                .await?
                .ok_or(Error::NotFound)?;
            if state.grants.mailbox_scopes(provider.as_str()).is_empty() {
                return Err(Error::Conflict(MESSAGES.reason("mailbox_unsupported")));
            }
            Ticket {
                account_id: account,
                provider,
                purpose: Purpose::Connect,
                workspace_id: Some(*workspace.as_uuid()),
                connection_id: None,
                expected_subject: None,
                login_hint: None,
                return_to,
                adds_way_in: input.sign_in.unwrap_or(true),
            }
        }
        LinkPurpose::Reconnect => {
            let workspace = input.workspace_id.ok_or_else(incomplete)?;
            let connection = input.connection_id.ok_or_else(incomplete)?;
            let target = state
                .grants
                .reconnect_target(state, &principal.subject, workspace, &connection)
                .await?;
            // A grant that still works is renewed only after a recent
            // sign-in, like adding one.
            if !target.needs_reauth {
                step_up::require_recent(principal, now)?;
            }
            Ticket {
                account_id: account,
                provider: configured(&target.provider)?,
                purpose: Purpose::Reconnect,
                workspace_id: Some(*workspace.as_uuid()),
                connection_id: Some(connection),
                expected_subject: Some(target.account_subject),
                login_hint: Some(target.address),
                return_to,
                adds_way_in: false,
            }
        }
    };
    state
        .rate_limits
        .check(
            &state.db,
            &LINK_LIMIT,
            &RateKey::text(&format!("account:{account}")),
        )
        .await?;
    let (token, expires_at) = links::issue(&state.db, &ticket, now).await?;
    tracing::info!(
        purpose = ticket.purpose.as_str(),
        provider = ticket.provider.as_str(),
        "a signed-in round trip was started"
    );
    Ok(StartedLink {
        url: format!(
            "/auth/start/{}?link={}",
            ticket.provider.as_str(),
            url::form_urlencoded::byte_serialize(token.as_bytes()).collect::<String>()
        ),
        expires_at,
    })
}
