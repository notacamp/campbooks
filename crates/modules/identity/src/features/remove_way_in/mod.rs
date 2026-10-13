//! `DELETE /api/v1/identity/ways-in/{id}` — stop signing in with a provider
//! account. Needs a recent sign-in, and never removes the account's last way
//! in (provider identities and passkeys count alike). Every session signed
//! in with it ends, with its refresh tokens. It never disconnects a mailbox:
//! connections are the connectors module's, with their own lifecycle.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Unbound};
use uuid::Uuid;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{accounts, sessions, step_up, ways_in},
    events::WayInRemoved,
    scopes::IdentityWrite,
};

/// The way in a path names (by field, so routes nested under the web BFF's
/// `/bff/api/{*path}` still resolve).
#[derive(Debug, serde::Deserialize)]
pub struct WayInPath {
    /// The way in.
    pub id: Uuid,
}

/// Remove a way in.
#[utoipa::path(
    delete,
    path = "/api/v1/identity/ways-in/{id}",
    operation_id = "identity.remove_way_in",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.provider_identity"])))),
    params(("id" = Uuid, Path, description = "The way in.")),
    responses(
        (status = NO_CONTENT, description = "Removed; the sessions signed in with it ended. \
            Mailbox connections are untouched."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`, or the sign-in \
            is more than ten minutes old (`step_up_required`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not the caller's way in, or gone.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "It is the account's last way in (`last_way_in`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityWrite>,
    Path(path): Path<WayInPath>,
) -> Result<StatusCode> {
    let (issuer, account) = super::account(&deps, &principal)?;
    let now = issuer.now();
    step_up::require_recent(&principal, now)?;
    let mut tx = state.db.begin().await?;
    // Locks the account: two removals can't both see one more left.
    let count = ways_in::lock_and_count(&mut tx, account).await?;
    let owned: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM identity.provider_identities WHERE id = $1 AND account_id = $2",
    )
    .bind(path.id)
    .bind(account)
    .fetch_optional(&mut *tx)
    .await?;
    if owned.is_none() {
        return Err(Error::NotFound);
    }
    if count <= 1 {
        return Err(Error::Conflict(MESSAGES.reason("last_way_in")));
    }
    // Whoever signed in with it is signed out, before the link to it goes.
    let ended = sessions::end_signed_in_with(&mut tx, path.id, now).await?;
    let provider = accounts::remove(&mut tx, path.id, account)
        .await?
        .ok_or(Error::NotFound)?;
    accounts::audit(
        &mut tx,
        Some(account),
        "way_in_removed",
        serde_json::json!({ "method": "provider", "provider": provider, "way_in": path.id, "sessions_ended": ended }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &WayInRemoved {
                account,
                method: "provider".to_owned(),
                provider: Some(provider),
            },
        )
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
