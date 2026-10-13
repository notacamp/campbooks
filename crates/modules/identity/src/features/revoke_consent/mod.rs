//! `DELETE /api/v1/identity/consents/{id}` — take an app's access away:
//! every refresh token it holds for the caller stops working at once, and
//! its access tokens within ten minutes. The next authorization asks again.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Scoped};
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{accounts, consents},
    events::ConsentRevoked,
    scopes::IdentityWrite,
};

/// The consent a path names (by field, so routes nested under the web BFF's
/// `/bff/api/{*path}` still resolve).
#[derive(Debug, serde::Deserialize)]
pub struct ConsentPath {
    /// The consent.
    pub id: Uuid,
}

/// Revoke an app's access.
#[utoipa::path(
    delete,
    path = "/api/v1/identity/consents/{id}",
    operation_id = "identity.revoke_consent",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.consent"])))),
    params(("id" = Uuid, Path, description = "The consent.")),
    responses(
        (status = NO_CONTENT, description = "Revoked; the app's tokens stop working."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not the caller's consent, or already revoked.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Scoped { principal, .. }: Scoped<IdentityWrite>,
    Path(path): Path<ConsentPath>,
) -> Result<StatusCode> {
    let (issuer, account) = super::account(&deps, &principal)?;
    let now = issuer.now();
    let mut tx = state.db.begin().await?;
    let revoked = consents::revoke(&mut tx, path.id, account, now)
        .await?
        .ok_or(Error::NotFound)?;
    accounts::audit(
        &mut tx,
        Some(account),
        "consent_revoked",
        serde_json::json!({ "client": revoked.client_id, "workspace": revoked.bound_workspace_id }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &ConsentRevoked {
                account,
                client: revoked.client_id,
                workspace: revoked.bound_workspace_id.map(|w| w.to_string()),
                reason: "revoked".to_owned(),
            },
        )
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
