//! `POST /api/v1/identity/passkeys/registrations` — start adding a passkey
//! to the caller's account: the options for `navigator.credentials.create`,
//! with a single-use, five-minute challenge bound to the account. Needs a
//! recent sign-in.

use std::time::Duration;

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::Unbound,
    rate_limit::{Limit, RateKey},
};

use crate::{
    deps::Deps,
    domain::{
        accounts,
        passkeys::{self, Binding, Ceremony, Purpose},
        step_up,
    },
    protocol::passkey,
    scopes::IdentityWrite,
    webauthn::CreationOptions,
};

/// Registrations one account may start per five minutes (a challenge's
/// lifetime): at most this many waiting at once.
const REGISTRATION_LIMIT: Limit =
    Limit::new("identity.passkey_registration", 10, Duration::from_mins(5));

/// Start adding a passkey.
#[utoipa::path(
    post,
    path = "/api/v1/identity/passkeys/registrations",
    operation_id = "identity.start_passkey_registration",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.webauthn_challenge"]).access("identity.passkey", campbooks_kernel::authz::Action::Write)))),
    responses(
        (status = CREATED, description = "Options for `navigator.credentials.create`: a \
            discoverable passkey, user verification required, attestation `none`, excluding \
            the caller's passkeys.", body = CreationOptions),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`, or the sign-in \
            is more than ten minutes old (`step_up_required`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "This instance offers no passkeys (not https), or \
            another identity provider serves it.", body = Problem,
            content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "`rate_limited`: ten registrations are \
            already waiting for this account.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityWrite>,
) -> Result<(StatusCode, Json<CreationOptions>)> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Write,
        "identity.passkey",
        || campbooks_kernel::Error::Forbidden,
    )?;
    let (issuer, account) = super::account(&deps, &principal)?;
    let rp = passkey::relying_party(&issuer).ok_or(Error::NotFound)?;
    let now = issuer.now();
    step_up::require_recent(&principal, now)?;
    // At most ten registrations waiting per account (a challenge lives five
    // minutes).
    state
        .rate_limits
        .check(
            &state.db,
            &REGISTRATION_LIMIT,
            &RateKey::text(&format!("account:{account}")),
        )
        .await?;
    let profile = accounts::profile(&state.db, account)
        .await?
        .ok_or(Error::NotFound)?;
    let display_name = profile
        .display_name
        .clone()
        .or_else(|| profile.primary_email.clone())
        .unwrap_or_else(|| "Campbooks".to_owned());
    // Saved under the address when there is one, so a person with several
    // accounts can tell their passkeys apart.
    let name = profile
        .primary_email
        .unwrap_or_else(|| display_name.clone());
    let exclude = passkeys::descriptors(&state.db, account).await?;
    let challenge = passkeys::start(
        &state.db,
        Purpose::Register,
        Binding::Account(account),
        &Ceremony::default(),
        now,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(rp.creation_options(&challenge, account, &name, &display_name, exclude)),
    ))
}
