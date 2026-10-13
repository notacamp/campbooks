//! `DELETE /api/v1/identity/passkeys/{id}` — stop signing in with a
//! passkey. Needs a recent sign-in, and never removes the account's last
//! way in (provider identities and passkeys count alike). Every session
//! signed in with it ends, with its refresh tokens.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Scoped};

use super::PasskeyPath;
use crate::{
    MESSAGES,
    deps::Deps,
    domain::{accounts, passkeys, sessions, step_up, ways_in},
    events::WayInRemoved,
    scopes::IdentityWrite,
};

/// Remove a passkey.
#[utoipa::path(
    delete,
    path = "/api/v1/identity/passkeys/{id}",
    operation_id = "identity.remove_passkey",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.passkey"])))),
    params(("id" = uuid::Uuid, Path, description = "The passkey.")),
    responses(
        (status = NO_CONTENT, description = "Removed; the sessions signed in with it ended. \
            The authenticator still holds the credential: the person can delete it there."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`, or the sign-in \
            is more than ten minutes old (`step_up_required`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not the caller's passkey, or gone.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "It is the account's last way in (`last_way_in`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Scoped { principal, .. }: Scoped<IdentityWrite>,
    Path(path): Path<PasskeyPath>,
) -> Result<StatusCode> {
    let (issuer, account) = super::account(&deps, &principal)?;
    let now = issuer.now();
    step_up::require_recent(&principal, now)?;
    let mut tx = state.db.begin().await?;
    // Locks the account: two removals can't both see one more left.
    let count = ways_in::lock_and_count(&mut tx, account).await?;
    if !passkeys::owned(&mut tx, path.id, account).await? {
        return Err(Error::NotFound);
    }
    if count <= 1 {
        return Err(Error::Conflict(MESSAGES.reason("last_way_in")));
    }
    // Whoever signed in with it is signed out, before it goes.
    let ended = sessions::end_signed_in_with_passkey(&mut tx, path.id, now).await?;
    if !passkeys::delete(&mut tx, path.id, account).await? {
        return Err(Error::NotFound);
    }
    accounts::audit(
        &mut tx,
        Some(account),
        "way_in_removed",
        serde_json::json!({ "method": "passkey", "passkey": path.id, "sessions_ended": ended }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &WayInRemoved {
                account,
                method: "passkey".to_owned(),
                provider: None,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
