//! `PATCH /api/v1/identity/passkeys/{id}` — rename one of the caller's
//! passkeys.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk, auth::Scoped, extract::Json as JsonBody,
};
use serde::Deserialize;
use utoipa::ToSchema;

use super::{PasskeyPath, list_passkeys::PasskeyView};
use crate::{MESSAGES, deps::Deps, domain::passkeys, scopes::IdentityWrite};

/// The new name.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenamePasskey {
    /// 1 to 64 characters.
    pub name: String,
}

/// Rename a passkey.
#[utoipa::path(
    patch,
    path = "/api/v1/identity/passkeys/{id}",
    operation_id = "identity.rename_passkey",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.passkey"]).access("identity.passkey", campbooks_kernel::authz::Action::Write)))),
    params(("id" = uuid::Uuid, Path, description = "The passkey.")),
    request_body = RenamePasskey,
    responses(
        (status = OK, description = "Renamed.", body = PasskeyView),
        (status = BAD_REQUEST, description = "`passkey_name_required`.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not the caller's passkey, or gone.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Scoped { principal, .. }: Scoped<IdentityWrite>,
    Path(path): Path<PasskeyPath>,
    JsonBody(input): JsonBody<RenamePasskey>,
) -> Result<Json<PasskeyView>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Write,
        "identity.passkey",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (_, account) = super::account(&deps, &principal)?;
    let name = passkeys::clean_name(&input.name)
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("passkey_name_required")))?;
    let renamed = passkeys::rename(&state.db, path.id, account, &name)
        .await?
        .ok_or(Error::NotFound)?;
    Ok(Json(PasskeyView::from(renamed)))
}
