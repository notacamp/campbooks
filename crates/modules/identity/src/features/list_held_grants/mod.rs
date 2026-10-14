//! `GET /api/v1/identity/held-grants` — the mailboxes the caller's sign-up
//! granted, waiting for a workspace (never their tokens).

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{AppState, Operation, Problem, Result, auth::Unbound};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{deps::Deps, domain::held_grants, scopes::IdentityRead};

/// A mailbox waiting to be connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct HeldGrant {
    /// Its id.
    pub id: Uuid,
    /// `google`.
    pub provider: String,
    /// The mailbox address.
    pub address: String,
    /// The provider's display name.
    pub display_name: Option<String>,
    /// When it stops waiting.
    pub expires_at: DateTime<Utc>,
}

/// The caller's held mailbox grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct HeldGrants {
    /// Oldest first.
    pub items: Vec<HeldGrant>,
}

/// List the mailboxes waiting to be connected.
#[utoipa::path(
    get,
    path = "/api/v1/identity/held-grants",
    operation_id = "identity.list_held_grants",
    tag = "identity",
    security(("bearer" = ["identity:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.held_grant"]).access("identity.held_grant", campbooks_kernel::authz::Action::Read)))),
    responses(
        (status = OK, description = "The caller's held grants.", body = HeldGrants),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:read`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Another identity provider serves this instance.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityRead>,
) -> Result<Json<HeldGrants>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Read,
        "identity.held_grant",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (issuer, account) = super::account(&deps, &principal)?;
    let items = held_grants::list(&state.db, account, issuer.now())
        .await?
        .into_iter()
        .map(|grant| HeldGrant {
            id: grant.id,
            provider: grant.provider,
            address: grant.address,
            display_name: grant.display_name,
            expires_at: grant.expires_at,
        })
        .collect();
    Ok(Json(HeldGrants { items }))
}
