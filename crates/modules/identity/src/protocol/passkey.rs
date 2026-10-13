//! Signing in and signing up with a passkey, behind the web client's
//! sign-in screens (ADR 0014, slice S3).
//!
//! Each ceremony is two calls from the same browser, both with the CSRF
//! header:
//!
//! 1. `POST /auth/passkey/{sign-in|sign-up}/options`: a single-use,
//!    five-minute challenge, bound to this browser by a `SameSite=Strict`
//!    ceremony cookie, and the options for `navigator.credentials`.
//! 2. `POST /auth/passkey/{sign-in|sign-up}`: the browser's credential. It
//!    is verified ([`crate::webauthn`]), the issuer session starts, and the
//!    answer says where to go: back to the client with a code when an
//!    authorization request waits, else `return_to`.
//!
//! Sign-in finds the account by the credential id alone (a discoverable
//! passkey: nobody types an address). Sign-up creates an account with no
//! provider at all, which the signup policy allows except on an
//! invite-only instance: a passkey proves no address, so it cannot match an
//! invitation. Every failure to verify answers the same
//! `passkey_not_accepted`; the server logs which check failed, never the
//! credential.

use std::time::Duration;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::{
    Error, Operation, Result, Risk,
    config::SignupPolicy,
    crypto::random_token,
    extract::Json as JsonBody,
    rate_limit::{ClientIp, Limit, RateKey},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    Ctx,
    sign_in::{continue_after_sign_in, session_cookie},
};
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::{
        accounts,
        passkeys::{self, Binding, Ceremony, Purpose},
        sessions::{self, Started},
    },
    events::{AccountCreated, SignedIn, WayInAdded},
    web::{self, SameSite, with_cookie},
    webauthn::{
        AuthenticationCredential, CEREMONY_TIMEOUT, CreationOptions, Refusal,
        RegistrationCredential, RelyingParty, RequestOptions, Stored,
    },
};

/// The ceremony cookie over `https`: ties a passkey ceremony to the browser
/// that started it. `Strict`: only the sign-in screens' own calls read it.
pub const CEREMONY_COOKIE: &str = "__Host-campbooks_webauthn";

/// Ceremonies one address may start or finish.
const CEREMONY_LIMIT: Limit = Limit::new("identity.passkey", 30, Duration::from_mins(1));

/// The name a passkey made at sign-up is given.
const FIRST_PASSKEY_NAME: &str = "Passkey";

/// Whether the instance offers passkeys: the issuer's origin is a valid
/// relying party (https, or `localhost` in development).
pub fn relying_party(issuer: &Issuer) -> Option<RelyingParty> {
    RelyingParty::for_origin(&issuer.url)
}

/// The relying party, or 404 when this instance cannot offer passkeys.
fn rp(issuer: &Issuer) -> Result<RelyingParty> {
    relying_party(issuer).ok_or(Error::NotFound)
}

/// Whether the signup policy lets a passkey alone create an account. An
/// invite-only instance needs an address with an invitation, which a
/// passkey never proves.
pub fn signup_allows_passkey(ctx: &Ctx) -> bool {
    ctx.state.config.signup.policy != SignupPolicy::InviteOnly
}

/// The authentication methods a passkey sign-in reports (RFC 8176): a
/// synced passkey is a software-protected key (`swk`), a device-bound one a
/// hardware key (`hwk`); user verification was required (`user`).
pub fn amr(backup_eligible: bool) -> [&'static str; 2] {
    [if backup_eligible { "swk" } else { "hwk" }, "user"]
}

fn not_accepted(refusal: Refusal) -> Error {
    tracing::info!(check = refusal.0, "a passkey was not accepted");
    Error::Denied(MESSAGES.reason("passkey_not_accepted"))
}

fn expired() -> Error {
    Error::Conflict(MESSAGES.reason("passkey_expired"))
}

/// The CSRF check, the rate limit, and the issuer.
async fn guard<'a>(ctx: &'a Ctx, headers: &HeaderMap, ip: ClientIp) -> Result<&'a Issuer> {
    let issuer = ctx.issuer()?;
    if !web::csrf_passes(headers, &issuer.url) {
        return Err(Error::Denied(MESSAGES.reason("csrf_check_failed")));
    }
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &CEREMONY_LIMIT, &RateKey::ip(ip))
        .await?;
    Ok(issuer)
}

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

/// A ceremony's options with the cookie binding it to this browser.
fn with_ceremony_cookie(issuer: &Issuer, cookie: &str, body: impl Serialize) -> Response {
    with_cookie(
        web::no_store(Json(body).into_response()),
        issuer.cookies.set(
            CEREMONY_COOKIE,
            cookie,
            CEREMONY_TIMEOUT.num_seconds(),
            SameSite::Strict,
        ),
    )
}

/// Where the browser goes, the session cookie, and the ceremony cookie
/// cleared.
fn signed_in_response(
    issuer: &Issuer,
    status: StatusCode,
    redirect: String,
    started: &Started,
) -> Response {
    let response = web::no_store((status, Json(SignedInWith { redirect })).into_response());
    let response = with_cookie(response, session_cookie(issuer, &started.token));
    with_cookie(
        response,
        issuer.cookies.clear(CEREMONY_COOKIE, SameSite::Strict),
    )
}

/// Where to go once signed in.
#[derive(Debug, Serialize, ToSchema)]
pub struct SignedInWith {
    /// Navigate here: the client's redirect URI with a code, or `return_to`.
    pub redirect: String,
}

/// What a sign-in ceremony continues.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct SignInStart {
    /// The waiting authorization request (`/sign-in?request=`), completed
    /// afterwards.
    pub request: Option<String>,
    /// Otherwise, a same-origin path to land on.
    pub return_to: Option<String>,
}

/// Start signing in with a passkey.
#[utoipa::path(
    post,
    path = "/auth/passkey/sign-in/options",
    tag = "identity",
    operation_id = "identity.start_passkey_sign_in",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.webauthn_challenge"])))),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    request_body = SignInStart,
    security(()),
    responses(
        (status = 200, description = "Options for `navigator.credentials.get` (any of this \
            site's passkeys, user verification required), with a ceremony cookie.",
            body = RequestOptions, headers(("set-cookie" = String))),
        (status = 400, description = "`invalid_return_to`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 403, description = "`csrf_check_failed`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 404, description = "This instance offers no passkeys (not https), or another \
            identity provider serves it."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn sign_in_options(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    JsonBody(body): JsonBody<SignInStart>,
) -> Result<Response> {
    let issuer = guard(&ctx, &headers, ip).await?;
    let rp = rp(issuer)?;
    let return_to = web::return_to(body.return_to.as_deref())
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("invalid_return_to")))?;
    let cookie = random_token();
    let challenge = passkeys::start(
        &ctx.state.db,
        Purpose::SignIn,
        Binding::Browser(&cookie),
        &Ceremony {
            request_id: body.request,
            return_to: Some(return_to),
            ..Ceremony::default()
        },
        issuer.now(),
    )
    .await?;
    Ok(with_ceremony_cookie(
        issuer,
        &cookie,
        rp.request_options(&challenge),
    ))
}

/// Take the ceremony a credential's challenge names, for this browser.
async fn take_ceremony(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    challenge: Result<String, Refusal>,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Ceremony> {
    let cookie = issuer
        .cookies
        .read(headers, CEREMONY_COOKIE)
        .ok_or_else(expired)?;
    let challenge = challenge.map_err(not_accepted)?;
    passkeys::take(
        &ctx.state.db,
        &challenge,
        purpose,
        Binding::Browser(&cookie),
        now,
    )
    .await?
    .ok_or_else(expired)
}

/// Finish signing in with a passkey.
#[utoipa::path(
    post,
    path = "/auth/passkey/sign-in",
    tag = "identity",
    operation_id = "identity.finish_passkey_sign_in",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.session"])))),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    request_body(content = AuthenticationCredential,
        description = "`PublicKeyCredential.toJSON()` of the assertion."),
    security(()),
    responses(
        (status = 200, description = "Signed in.", body = SignedInWith,
            headers(("set-cookie" = String))),
        (status = 403, description = "`csrf_check_failed`, or `passkey_not_accepted` (it did \
            not verify, or it is no longer a way into any account).",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "This instance offers no passkeys, or another identity \
            provider serves it."),
        (status = 409, description = "`passkey_expired`: no ceremony waits for this browser \
            (it took more than five minutes, or was used).", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn sign_in(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    JsonBody(credential): JsonBody<AuthenticationCredential>,
) -> Result<Response> {
    let issuer = guard(&ctx, &headers, ip).await?;
    let rp = rp(issuer)?;
    let now = issuer.now();
    let ceremony = take_ceremony(
        &ctx,
        issuer,
        &headers,
        credential.challenge(),
        Purpose::SignIn,
        now,
    )
    .await?;
    let challenge = credential.challenge().map_err(not_accepted)?;
    let credential_id = credential.credential_id().map_err(not_accepted)?;
    let mut tx = ctx.state.db.begin().await?;
    let Some(passkey) = passkeys::by_credential(&mut tx, &credential_id).await? else {
        return Err(not_accepted(Refusal("passkey_unknown")));
    };
    // The user handle is the account id the passkey was made for.
    if credential.user_handle().map_err(not_accepted)? != passkey.account_id {
        return Err(not_accepted(Refusal("user_handle_mismatch")));
    }
    let verified = rp
        .verify_assertion(
            &challenge,
            &credential,
            &Stored {
                credential_id: &passkey.credential_id,
                public_key: &passkey.public_key,
                alg: i64::from(passkey.alg),
                backup_eligible: passkey.backup_eligible,
            },
        )
        .map_err(not_accepted)?;
    let reported = i64::from(verified.sign_count);
    if (reported != 0 || passkey.sign_count != 0) && reported <= passkey.sign_count {
        // A counter that does not move forward may mean a cloned
        // authenticator. Synced passkeys report 0, and a refusal would lock
        // out the person, so it is recorded instead (§7.2 step 21).
        tracing::warn!(passkey = %passkey.id, "a passkey's signature counter did not increase");
        accounts::audit(
            &mut tx,
            Some(passkey.account_id),
            "passkey_sign_count_regressed",
            serde_json::json!({ "passkey": passkey.id, "stored": passkey.sign_count, "reported": reported }),
            now,
        )
        .await?;
    }
    passkeys::record_use(
        &mut tx,
        passkey.id,
        verified.sign_count,
        verified.backup_state,
        now,
    )
    .await?;
    let started = sessions::start_with_passkey(
        &mut tx,
        passkey.account_id,
        passkey.id,
        &amr(passkey.backup_eligible),
        user_agent(&headers),
        now,
    )
    .await?;
    announce_sign_in(&ctx, &mut tx, passkey.account_id, passkey.id, now).await?;
    tx.commit().await?;
    let redirect = continue_after_sign_in(
        &ctx,
        issuer,
        ceremony.request_id.as_deref(),
        ceremony.return_to.as_deref().unwrap_or("/"),
        &started.session,
        now,
    )
    .await?;
    Ok(signed_in_response(
        issuer,
        StatusCode::OK,
        redirect,
        &started,
    ))
}

/// Record a passkey sign-in in the audit trail and announce it.
async fn announce_sign_in(
    ctx: &Ctx,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: Uuid,
    passkey: Uuid,
    now: DateTime<Utc>,
) -> Result<()> {
    accounts::audit(
        tx,
        Some(account),
        "signed_in",
        serde_json::json!({ "method": "passkey", "passkey": passkey }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            tx,
            &SignedIn {
                account,
                method: "passkey".to_owned(),
                provider: None,
            },
        )
        .await?;
    Ok(())
}

/// A new account with a passkey alone.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SignUpStart {
    /// What to call the person (shown in the app; the passkey is saved
    /// under it).
    pub display_name: String,
    /// The waiting authorization request, completed afterwards.
    pub request: Option<String>,
    /// Otherwise, a same-origin path to land on.
    pub return_to: Option<String>,
}

/// Start creating an account with a passkey.
#[utoipa::path(
    post,
    path = "/auth/passkey/sign-up/options",
    tag = "identity",
    operation_id = "identity.start_passkey_sign_up",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.webauthn_challenge"])))),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    request_body = SignUpStart,
    security(()),
    responses(
        (status = 200, description = "Options for `navigator.credentials.create` (a \
            discoverable passkey, user verification required, attestation `none`), with a \
            ceremony cookie.", body = CreationOptions, headers(("set-cookie" = String))),
        (status = 400, description = "`display_name_required` or `invalid_return_to`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 403, description = "`csrf_check_failed`, or `signup_invite_only` (this \
            instance takes invited people only, and a passkey proves no address).",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "This instance offers no passkeys, or another identity \
            provider serves it."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn sign_up_options(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    JsonBody(body): JsonBody<SignUpStart>,
) -> Result<Response> {
    let issuer = guard(&ctx, &headers, ip).await?;
    let rp = rp(issuer)?;
    if !signup_allows_passkey(&ctx) {
        return Err(Error::Denied(MESSAGES.reason("signup_invite_only")));
    }
    let display_name = passkeys::clean_name(&body.display_name)
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("display_name_required")))?;
    let return_to = web::return_to(body.return_to.as_deref())
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("invalid_return_to")))?;
    let account = Uuid::now_v7();
    let cookie = random_token();
    let challenge = passkeys::start(
        &ctx.state.db,
        Purpose::SignUp,
        Binding::Browser(&cookie),
        &Ceremony {
            user_handle: Some(account),
            display_name: Some(display_name.clone()),
            request_id: body.request,
            return_to: Some(return_to),
            ..Ceremony::default()
        },
        issuer.now(),
    )
    .await?;
    let options = rp.creation_options(&challenge, account, &display_name, &display_name, vec![]);
    Ok(with_ceremony_cookie(issuer, &cookie, options))
}

/// Finish creating an account with a passkey.
#[utoipa::path(
    post,
    path = "/auth/passkey/sign-up",
    tag = "identity",
    operation_id = "identity.finish_passkey_sign_up",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.account", "identity.passkey", "identity.session"])))),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    request_body(content = RegistrationCredential,
        description = "`PublicKeyCredential.toJSON()` of the new passkey."),
    security(()),
    responses(
        (status = 201, description = "Created and signed in.", body = SignedInWith,
            headers(("set-cookie" = String))),
        (status = 403, description = "`csrf_check_failed`, `passkey_not_accepted`, or \
            `signup_invite_only`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 404, description = "This instance offers no passkeys, or another identity \
            provider serves it."),
        (status = 409, description = "`passkey_expired`, or `passkey_already_registered`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn sign_up(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    JsonBody(credential): JsonBody<RegistrationCredential>,
) -> Result<Response> {
    let issuer = guard(&ctx, &headers, ip).await?;
    let rp = rp(issuer)?;
    let now = issuer.now();
    let ceremony = take_ceremony(
        &ctx,
        issuer,
        &headers,
        credential.challenge(),
        Purpose::SignUp,
        now,
    )
    .await?;
    // The policy may have changed since the options were given.
    if !signup_allows_passkey(&ctx) {
        return Err(Error::Denied(MESSAGES.reason("signup_invite_only")));
    }
    let challenge = credential.challenge().map_err(not_accepted)?;
    let new = rp
        .verify_registration(&challenge, &credential)
        .map_err(not_accepted)?;
    let (Some(account), Some(display_name)) = (ceremony.user_handle, ceremony.display_name) else {
        return Err(Error::internal("a sign-up ceremony without its account"));
    };
    let mut tx = ctx.state.db.begin().await?;
    passkeys::create_account(&mut tx, account, &display_name, now).await?;
    let Some(passkey) = passkeys::insert(&mut tx, account, &new, FIRST_PASSKEY_NAME, now).await?
    else {
        return Err(Error::Conflict(
            MESSAGES.reason("passkey_already_registered"),
        ));
    };
    let started = sessions::start_with_passkey(
        &mut tx,
        account,
        passkey,
        &amr(new.backup_eligible),
        user_agent(&headers),
        now,
    )
    .await?;
    accounts::audit(
        &mut tx,
        Some(account),
        "account_created",
        serde_json::json!({ "method": "passkey", "passkey": passkey, "attestation": new.attestation_format }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            &mut tx,
            &AccountCreated {
                account,
                provider: "passkey".to_owned(),
            },
        )
        .await?;
    ctx.state
        .events
        .publish(
            &mut tx,
            &WayInAdded {
                account,
                method: "passkey".to_owned(),
                provider: None,
                via: "sign_up".to_owned(),
            },
        )
        .await?;
    announce_sign_in(&ctx, &mut tx, account, passkey, now).await?;
    tx.commit().await?;
    let redirect = continue_after_sign_in(
        &ctx,
        issuer,
        ceremony.request_id.as_deref(),
        ceremony.return_to.as_deref().unwrap_or("/"),
        &started.session,
        now,
    )
    .await?;
    Ok(signed_in_response(
        issuer,
        StatusCode::CREATED,
        redirect,
        &started,
    ))
}
