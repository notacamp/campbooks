//! `GET /api/v1/identity/ways-in` — how the caller can sign in: their
//! provider ways in, and the providers this instance offers. Passkeys are
//! listed by `identity.list_passkeys`.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{AppState, Operation, Problem, Result, auth::Scoped};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{deps::Deps, domain::accounts, scopes::IdentityRead};

/// A provider account the caller signs in with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct WayIn {
    /// Its id.
    pub id: Uuid,
    /// `provider` (passkeys are listed with the passkeys).
    pub kind: String,
    /// `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The address the provider gave when it was added (shown, never used
    /// to find an account).
    pub email: Option<String>,
    /// `sign_up`, `link` or `connect`.
    pub linked_via: String,
    /// When it was added.
    pub linked_at: DateTime<Utc>,
    /// When it was last used to sign in.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// The caller's ways in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct WaysIn {
    /// Oldest first.
    pub items: Vec<WayIn>,
    /// The providers this instance offers, in display order.
    pub providers: Vec<String>,
}

/// List how you sign in.
#[utoipa::path(
    get,
    path = "/api/v1/identity/ways-in",
    operation_id = "identity.list_ways_in",
    tag = "identity",
    security(("bearer" = ["identity:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.provider_identity"]).access("identity.provider_identity", campbooks_kernel::authz::Action::Read)))),
    responses(
        (status = OK, description = "The caller's provider ways in.", body = WaysIn),
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
    Scoped { principal, .. }: Scoped<IdentityRead>,
) -> Result<Json<WaysIn>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Read,
        "identity.provider_identity",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (issuer, account) = super::account(&deps, &principal)?;
    let items = accounts::ways_in(&state.db, account)
        .await?
        .into_iter()
        .map(|way| WayIn {
            id: way.id,
            kind: "provider".to_owned(),
            provider: way.provider,
            email: way.email,
            linked_via: way.linked_via,
            linked_at: way.linked_at,
            last_used_at: way.last_used_at,
        })
        .collect();
    Ok(Json(WaysIn {
        items,
        providers: issuer
            .upstreams
            .iter()
            .map(|u| u.provider().as_str().to_owned())
            .collect(),
    }))
}
