//! `DELETE /api/v1/identity/held-grants/{id}` — the person does not want
//! this mailbox connected: forget its tokens now, revoking a Zoho one at its
//! data centre (Zoho caps the refresh tokens a person holds per app).

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Unbound};

use crate::{deps::Deps, domain::held_grants, scopes::IdentityWrite, upstream::Provider};

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
    Unbound { principal, .. }: Unbound<IdentityWrite>,
    Path(path): Path<super::HeldGrantPath>,
) -> Result<StatusCode> {
    let (issuer, account) = super::account(&deps, &principal)?;
    let mut tx = state.db.begin().await?;
    let taken = held_grants::take(&mut tx, &issuer.cipher, path.id, account, issuer.now())
        .await?
        .ok_or(Error::NotFound)?;
    held_grants::delete(&mut tx, path.id, account).await?;
    tx.commit().await?;
    let upstream = Provider::parse(&taken.grant.provider).and_then(|p| issuer.upstream(p));
    if let (Some(upstream), Some(refresh)) = (upstream, taken.tokens.refresh_token.as_deref())
        && let Err(error) = upstream.revoke_unkept(&taken.issuer, refresh).await
    {
        tracing::warn!(%error, "could not revoke a discarded mailbox grant");
    }
    Ok(StatusCode::NO_CONTENT)
}
