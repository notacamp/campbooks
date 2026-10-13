//! `GET /api/v1/identity/passkeys` — the caller's passkeys (never their
//! keys).

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{AppState, Operation, Problem, Result, auth::Scoped};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::passkeys::{self, Passkey},
    scopes::IdentityRead,
};

/// A passkey, as its owner sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PasskeyView {
    /// Its id.
    pub id: Uuid,
    /// What its owner calls it.
    pub name: String,
    /// Whether it may sync between devices (a password manager or a
    /// platform account keeps it), as opposed to staying on one device.
    pub synced: bool,
    /// When it was added.
    pub created_at: DateTime<Utc>,
    /// When it last signed in.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<Passkey> for PasskeyView {
    fn from(passkey: Passkey) -> Self {
        Self {
            id: passkey.id,
            name: passkey.name,
            synced: passkey.backup_eligible,
            created_at: passkey.created_at,
            last_used_at: passkey.last_used_at,
        }
    }
}

/// The caller's passkeys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Passkeys {
    /// Oldest first.
    pub items: Vec<PasskeyView>,
}

/// List the caller's passkeys.
#[utoipa::path(
    get,
    path = "/api/v1/identity/passkeys",
    operation_id = "identity.list_passkeys",
    tag = "identity",
    security(("bearer" = ["identity:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.passkey"]).access("identity.passkey", campbooks_kernel::authz::Action::Read)))),
    responses(
        (status = OK, description = "The caller's passkeys.", body = Passkeys),
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
) -> Result<Json<Passkeys>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Read,
        "identity.passkey",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (_, account) = super::account(&deps, &principal)?;
    let items = passkeys::list(&state.db, account)
        .await?
        .into_iter()
        .map(PasskeyView::from)
        .collect();
    Ok(Json(Passkeys { items }))
}
