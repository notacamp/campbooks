//! `POST /api/v1/identity/passkeys` — add the passkey the browser just made
//! (for a challenge from `identity.start_passkey_registration`) as a way
//! into the caller's account. Needs a recent sign-in.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk, auth::Unbound, extract::Json as JsonBody,
};
use serde::Deserialize;
use utoipa::ToSchema;

use super::list_passkeys::PasskeyView;
use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        accounts,
        passkeys::{self, Binding, Purpose},
        step_up,
    },
    events::WayInAdded,
    protocol::passkey,
    scopes::IdentityWrite,
    webauthn::RegistrationCredential,
};

/// The new passkey.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterPasskey {
    /// `PublicKeyCredential.toJSON()` of `navigator.credentials.create`.
    pub credential: RegistrationCredential,
    /// What to call it (default `Passkey`).
    pub name: Option<String>,
}

/// Add a passkey.
#[utoipa::path(
    post,
    path = "/api/v1/identity/passkeys",
    operation_id = "identity.register_passkey",
    tag = "identity",
    security(("bearer" = ["identity:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["identity.passkey"]).access("identity.passkey", campbooks_kernel::authz::Action::Write)))),
    request_body = RegisterPasskey,
    responses(
        (status = CREATED, description = "Added: a way in from now on.", body = PasskeyView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:write`, the sign-in is \
            more than ten minutes old (`step_up_required`), or the passkey did not verify \
            (`passkey_not_accepted`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "This instance offers no passkeys, or another \
            identity provider serves it.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "`passkey_expired` (no registration waits: it took \
            more than five minutes, or was used), or `passkey_already_registered`.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Unbound { principal, .. }: Unbound<IdentityWrite>,
    JsonBody(input): JsonBody<RegisterPasskey>,
) -> Result<(StatusCode, Json<PasskeyView>)> {
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
    let refused = |refusal: crate::webauthn::Refusal| {
        tracing::info!(check = refusal.0, "a new passkey was not accepted");
        Error::Denied(MESSAGES.reason("passkey_not_accepted"))
    };
    let challenge = input.credential.challenge().map_err(refused)?;
    passkeys::take(
        &state.db,
        &challenge,
        Purpose::Register,
        Binding::Account(account),
        now,
    )
    .await?
    .ok_or_else(|| Error::Conflict(MESSAGES.reason("passkey_expired")))?;
    let new = rp
        .verify_registration(&challenge, &input.credential)
        .map_err(refused)?;
    let name = input
        .name
        .as_deref()
        .and_then(passkeys::clean_name)
        .unwrap_or_else(|| "Passkey".to_owned());
    let mut tx = state.db.begin().await?;
    let id = passkeys::insert(&mut tx, account, &new, &name, now)
        .await?
        .ok_or_else(|| Error::Conflict(MESSAGES.reason("passkey_already_registered")))?;
    accounts::audit(
        &mut tx,
        Some(account),
        "way_in_added",
        serde_json::json!({ "method": "passkey", "passkey": id, "attestation": new.attestation_format }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &WayInAdded {
                account,
                method: "passkey".to_owned(),
                provider: None,
                via: "passkey".to_owned(),
            },
        )
        .await?;
    tx.commit().await?;
    let added = passkeys::list(&state.db, account)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(Error::NotFound)?;
    Ok((StatusCode::CREATED, Json(PasskeyView::from(added))))
}
