//! `POST /api/v1/identity/sign-out-everywhere` — end every sign-in of the
//! caller's account: every issuer session, every refresh token of every app
//! (Campbooks' own and others), and, through `identity.signed_out_everywhere`,
//! every web session. The access tokens already issued stop at once: the
//! account's revocation epoch moves to now (`accounts::revoke_tokens`),
//! which the API checks on every request. No recent sign-in is asked for:
//! whoever fears a stolen session must be able to end it at once.

use axum::{Extension, extract::State, http::StatusCode};
use campbooks_kernel::{AppState, Operation, Problem, Result, Risk, auth::Unbound};

use crate::{
    deps::Deps,
    domain::{accounts, sessions},
    events::SignedOutEverywhere,
    scopes::IdentityWrite,
};

/// Sign out everywhere.
#[utoipa::path(
    post,
    path = "/api/v1/identity/sign-out-everywhere",
    operation_id = "identity.sign_out_everywhere",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.session", "identity.refresh_family"]).access("identity.session", campbooks_kernel::authz::Action::Delete)))),
    responses(
        (status = NO_CONTENT, description = "Every session and refresh token of the account \
            ended, this one's included: sign in again."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Another identity provider serves this instance, or \
            the token is bound to a workspace.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityWrite>,
) -> Result<StatusCode> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Delete,
        "identity.session",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (issuer, account) = super::account(&deps, &principal)?;
    let now = issuer.now();
    let mut tx = state.db.begin().await?;
    // Held until the commit, so a token exchange minting for this account
    // either finishes first (and its tokens fall before the epoch) or sees
    // the epoch.
    accounts::lock(&mut tx, account).await?;
    let (sessions, families) = sessions::end_all(&mut tx, account, now).await?;
    accounts::revoke_tokens(&mut tx, account, now).await?;
    accounts::audit(
        &mut tx,
        Some(account),
        "signed_out_everywhere",
        serde_json::json!({ "sessions": sessions, "refresh_families": families }),
        now,
    )
    .await?;
    state
        .events
        .publish(&mut tx, &SignedOutEverywhere { account })
        .await?;
    tx.commit().await?;
    tracing::info!(sessions, families, "signed out everywhere");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
