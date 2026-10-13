//! `DELETE /api/v1/identity/held-grants/{id}` — the person does not want
//! this mailbox connected: forget its tokens now.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Scoped};

use crate::{deps::Deps, domain::held_grants, scopes::IdentityWrite};

/// Discard a held mailbox grant.
#[utoipa::path(
    delete,
    path = "/api/v1/identity/held-grants/{id}",
    operation_id = "identity.discard_held_grant",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.held_grant"])))),
    params(("id" = uuid::Uuid, Path, description = "The held grant.")),
    responses(
        (status = NO_CONTENT, description = "Discarded."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not the caller's, or gone.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Scoped { principal, .. }: Scoped<IdentityWrite>,
    Path(path): Path<super::HeldGrantPath>,
) -> Result<StatusCode> {
    let (_, account) = super::account(&deps, &principal)?;
    let mut conn = state.db.acquire().await?;
    if held_grants::delete(&mut conn, path.id, account).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Error::NotFound)
    }
}
