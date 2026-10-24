//! Signing in with a provider, behind the web client's sign-in screens.
//!
//! 1. `GET /auth/requests/{id}`: what the sign-in screen shows for a waiting
//!    authorization request (the client, the providers on offer).
//! 2. `GET /auth/start/{provider}`: one round trip to the provider, asking in
//!    one consent for identity, the mailbox scopes this instance can use
//!    (the grant-intake port says which) and, with them, offline access.
//!    With `?link=<ticket>` it is a signed-in round trip instead (adding a
//!    way in, connecting or reconnecting a mailbox): see [`super::link`].
//! 3. `GET /auth/callback/{provider}`: back from the provider. A provider
//!    identity already linked signs that account in. An unknown one becomes
//!    a pending sign-up, and the browser goes to the new-account screen;
//!    nothing is created yet, and nothing is said about other accounts.
//! 4. `GET /auth/new-account` shows that pending sign-up, `POST` creates the
//!    account with that way in, holds the mailbox grant, and signs in.
//!
//! Errors at a sign-in callback send the browser back to the sign-in screen with
//! `?error=<code>`: `invalid_state`, `cancelled_at_provider`,
//! `provider_unavailable`, `login_failed`, `account_suspended`, and
//! `signup_invite_only` (an invite-only instance, and no pending invitation
//! for the provider's verified address: nothing is kept), and
//! `sign_in_not_allowed` (`CAMPBOOKS_SIGNUP__ALLOWED` is set and the
//! provider's verified address is not on it: asked first, for known
//! accounts and new ones alike, so it says nothing about accounts; nothing
//! is kept).

use std::time::Duration;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::{
    Error, Operation, Result, Risk,
    config::{SignupPolicy, TrackingSink},
    crypto::{pkce_challenge, random_token},
    rate_limit::{ClientIp, Limit, RateKey},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{Ctx, NEW_ACCOUNT_PATH, SIGN_IN_PATH, authorize, link, passkey};
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::{
        accounts::{self, Linked, ProviderIdentity},
        held_grants::{self, NewHeldGrant, ProviderTokens},
        sessions::{self, Session},
        tracked,
    },
    events::{AccountCreated, SignedIn},
    tracking::{self, SignInFailure, SignInMethod},
    upstream::{
        self, FLOW_LIFETIME, Flow, PendingSignUp, Provider, Purpose, SIGN_UP_LIFETIME,
        client::{AuthorizeRequest, SignedIn as Upstreamed, Tokens, Upstream, UpstreamError},
    },
    web::{
        self, SESSION_COOKIE, SIGN_UP_COOKIE, SameSite, UPSTREAM_COOKIE, see_other, with_cookie,
    },
};

/// Scopes every provider round trip asks for to know who signed in.
const IDENTITY_SCOPES: [&str; 3] = ["openid", "email", "profile"];

/// Round trips one address may start (`/auth/start`).
const START_LIMIT: Limit = Limit::new("identity.start_sign_in", 30, Duration::from_mins(1));
/// Callbacks one address may finish.
const CALLBACK_LIMIT: Limit = Limit::new("identity.finish_sign_in", 30, Duration::from_mins(1));

/// What the sign-in screen shows for a waiting authorization request.
#[derive(Debug, Serialize, ToSchema)]
pub struct RequestInfo {
    /// The client asking.
    pub client_id: String,
    /// The ways in this instance offers, in display order: the providers
    /// configured, then `passkey` when it can offer passkeys (https, or
    /// `localhost` in development).
    pub providers: Vec<String>,
    /// Whether a passkey alone may create a new account here (not on an
    /// invite-only instance).
    pub passkey_sign_up: bool,
}

pub(super) fn providers(issuer: &Issuer) -> Vec<String> {
    let mut ways: Vec<String> = issuer
        .upstreams
        .iter()
        .map(|upstream| upstream.provider().as_str().to_owned())
        .collect();
    if passkey::relying_party(issuer).is_some() {
        ways.push("passkey".to_owned());
    }
    ways
}

/// The waiting authorization request `id`: who asks, and how one can sign in.
#[utoipa::path(
    get,
    path = "/auth/requests/{id}",
    tag = "identity",
    operation_id = "identity.get_sign_in_request",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.authorize_request"]).access("identity.sign_in", campbooks_kernel::authz::Action::Read)))),
    params(("id" = String, Path, description = "The `request` the sign-in screen was given.")),
    security(()),
    responses(
        (status = 200, description = "The request.", body = RequestInfo),
        (status = 404, description = "No such request (or it expired, or another browser \
            made it), or another identity provider serves this instance."),
    ),
)]
pub async fn request(
    State(ctx): State<Ctx>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RequestInfo>> {
    let issuer = ctx.issuer()?;
    let browser = authorize::browser(issuer, &headers);
    let request = authorize::load(&ctx.state.db, &id, browser.as_deref(), issuer.now())
        .await?
        .ok_or(Error::NotFound)?;
    Ok(Json(RequestInfo {
        client_id: request.client_id,
        providers: providers(issuer),
        passkey_sign_up: passkey::relying_party(issuer).is_some()
            && passkey::signup_allows_passkey(&ctx)
            && !ctx.allow_list()?.is_restricted(),
    }))
}

/// `/auth/start/{provider}` parameters.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StartQuery {
    /// The waiting authorization request to complete afterwards.
    pub request: Option<String>,
    /// Otherwise, a same-origin path to land on.
    pub return_to: Option<String>,
    /// A signed-in round trip's ticket (from `identity.start_link`).
    pub link: Option<String>,
}

pub(super) fn sign_in_error(code: &str, request: Option<&str>) -> Response {
    match request {
        Some(request) => see_other(&format!(
            "{SIGN_IN_PATH}?error={code}&request={}",
            encode(request)
        )),
        None => see_other(&format!("{SIGN_IN_PATH}?error={code}")),
    }
}

/// `value` percent-encoded for a query string.
pub(super) fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Whether the instance's signup policy lets `identity` create an account.
/// On an invite-only instance only an address the provider verified, with a
/// pending invitation, may. Open, code and approval instances let everyone
/// create an account; their gate is creating a workspace (the workspaces
/// module asks for the code, or records the request for approval).
async fn may_create_account(ctx: &Ctx, identity: &ProviderIdentity) -> Result<bool> {
    if ctx.state.config.signup.policy != SignupPolicy::InviteOnly {
        return Ok(true);
    }
    match (&identity.email, identity.email_verified) {
        (Some(email), true) => {
            ctx.state
                .invitations
                .pending_for(&ctx.state.db, &email.to_lowercase())
                .await
        }
        _ => Ok(false),
    }
}

/// Start a round trip to `provider`.
#[utoipa::path(
    get,
    path = "/auth/start/{provider}",
    tag = "identity",
    operation_id = "identity.start_sign_in",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.upstream_flow"]).access("identity.sign_in", campbooks_kernel::authz::Action::Write)))),
    params(
        ("provider" = String, Path, description = "`google`, `microsoft` or `zoho`."),
        StartQuery,
    ),
    security(()),
    responses(
        (status = 303, description = "To the provider, with a round-trip cookie; or, for a \
            `link` ticket that is spent, expired or opened by a browser not signed in to its \
            account, back with `link_error=`.",
            headers(("location" = String), ("set-cookie" = String))),
        (status = 400, description = "`invalid_return_to`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 404, description = "Unknown or unconfigured provider, or another identity \
            provider serves this instance."),
    ),
)]
pub async fn start(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Query(query): Query<StartQuery>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let provider = Provider::parse(&provider).ok_or(Error::NotFound)?;
    let upstream = configured(issuer, provider)?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &START_LIMIT, &RateKey::ip(ip))
        .await?;
    if let Some(ticket) = query.link.as_deref() {
        return link::start(&ctx, issuer, &headers, upstream, ticket).await;
    }
    let return_to = web::return_to(query.return_to.as_deref())
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("invalid_return_to")))?;
    let flow = Flow::sign_in(provider, query.request, return_to);
    redirect_to_provider(&ctx, issuer, upstream, flow, Prompt::default()).await
}

/// The client of `provider`, when this instance has one; 404 otherwise.
pub(super) fn configured(issuer: &Issuer, provider: Provider) -> Result<&Upstream> {
    issuer.upstream(provider).ok_or(Error::NotFound)
}

/// How to ask the provider.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Prompt<'a> {
    /// Ask for consent again (a refresh token comes back).
    pub consent: bool,
    /// The account to suggest.
    pub login_hint: Option<&'a str>,
}

/// Store `flow` and send the browser to the provider. Identity scopes
/// always; the mailbox scopes (and offline access) for a sign-in, a connect
/// and a reconnect.
pub(super) async fn redirect_to_provider(
    ctx: &Ctx,
    issuer: &Issuer,
    upstream: &Upstream,
    flow: Flow,
    prompt: Prompt<'_>,
) -> Result<Response> {
    let now = issuer.now();
    let state = random_token();
    let browser = random_token();
    upstream::store_flow(&ctx.state.db, &issuer.cipher, &state, &browser, &flow, now).await?;
    let mut scopes: Vec<String> = IDENTITY_SCOPES.iter().map(|s| (*s).to_owned()).collect();
    let mut mailbox = if asks_mailbox(&flow) {
        ctx.state.grants.mailbox_scopes(flow.provider.as_str())
    } else {
        Vec::new()
    };
    // The scopes of the capabilities a connect or reconnect adds to the
    // mailbox (calendar). Not part of what the mailbox grant must hold: a
    // provider that lets the person leave them out still grants the mailbox.
    if !mailbox.is_empty() && !flow.capabilities.is_empty() {
        let extra = ctx
            .state
            .grants
            .capability_scopes(flow.provider.as_str(), &flow.capabilities);
        for scope in extra {
            if !mailbox.contains(&scope) {
                mailbox.push(scope);
            }
        }
    }
    let offline = !mailbox.is_empty();
    scopes.extend(mailbox);
    let location = upstream.authorize_url(&AuthorizeRequest {
        scopes: &scopes,
        state: &state,
        nonce: &flow.nonce,
        code_challenge: &pkce_challenge(&flow.code_verifier),
        consent: prompt.consent,
        offline,
        login_hint: prompt.login_hint,
    });
    Ok(with_cookie(
        see_other(&location),
        issuer.cookies.set(
            UPSTREAM_COOKIE,
            &browser,
            FLOW_LIFETIME.num_seconds(),
            SameSite::Lax,
        ),
    ))
}

/// Whether a round trip asks for the mailbox (and offline access): a
/// connect or reconnect, and a sign-in, except a first Zoho sign-in: Zoho
/// caps the refresh tokens a person holds per app, so one is asked for only
/// once the account turns out to be new (a second round trip, below), never
/// on each sign-in of a known account.
pub(super) fn asks_mailbox(flow: &Flow) -> bool {
    match flow.purpose {
        Purpose::Link => false,
        Purpose::SignIn => flow.provider != Provider::Zoho || flow.consent_retry,
        Purpose::Connect | Purpose::Reconnect => true,
    }
}

/// Revoke a refresh token Campbooks does not keep, where the provider
/// revokes one token alone (Zoho). Best effort: a failure is logged, never
/// the token.
pub(super) async fn drop_refresh(upstream: &Upstream, issuer: &str, refresh: Option<&str>) {
    let Some(refresh) = refresh else {
        return;
    };
    if let Err(error) = upstream.revoke_unkept(issuer, refresh).await {
        tracing::warn!(
            provider = upstream.provider().as_str(),
            %error,
            "could not revoke a refresh token Campbooks does not keep"
        );
    }
}

/// What the provider sends back.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CallbackQuery {
    /// The authorization code.
    pub code: Option<String>,
    /// The round trip's `state`.
    pub state: Option<String>,
    /// The provider's error (`access_denied` when the person cancelled).
    pub error: Option<String>,
    /// The provider's issuer (RFC 9207), when it sends one.
    pub iss: Option<String>,
    /// Zoho's accounts server of the person's data centre.
    #[serde(rename = "accounts-server")]
    pub data_centre: Option<String>,
}

/// Finish a round trip.
#[utoipa::path(
    get,
    path = "/auth/callback/{provider}",
    tag = "identity",
    operation_id = "identity.finish_sign_in",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.upstream_flow", "identity.session", "identity.pending_sign_up"]).access("identity.sign_in", campbooks_kernel::authz::Action::Write)))),
    params(
        ("provider" = String, Path, description = "`google`, `microsoft` or `zoho`."),
        CallbackQuery,
    ),
    security(()),
    responses(
        (status = 303, description = "Signed in (to the client, or `return_to`), to the \
            new-account screen, back to the provider for consent, or to the sign-in screen \
            with `?error=`. A signed-in round trip returns to its `return_to` with \
            `linked=`, `connected=` or `link_error=`.",
            headers(("location" = String), ("set-cookie" = String))),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 404, description = "Unknown or unconfigured provider, or another identity \
            provider serves this instance."),
    ),
)]
#[allow(
    clippy::too_many_lines,
    reason = "one flow, read top to bottom; its tracked events sit next to the changes they record"
)]
pub async fn callback(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let provider = Provider::parse(&provider).ok_or(Error::NotFound)?;
    let upstream = configured(issuer, provider)?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &CALLBACK_LIMIT, &RateKey::ip(ip))
        .await?;
    let now = issuer.now();
    let clear_flow = issuer.cookies.clear(UPSTREAM_COOKIE, SameSite::Lax);
    let browser = issuer.cookies.read(&headers, UPSTREAM_COOKIE);
    let flow = match &query.state {
        Some(state) => {
            upstream::take_flow(
                &ctx.state.db,
                &issuer.cipher,
                state,
                browser.as_deref(),
                provider,
                now,
            )
            .await?
        }
        None => None,
    };
    let Some(flow) = flow else {
        refused(
            &ctx,
            SignInMethod::Provider,
            Some(provider),
            "invalid_state",
        )
        .await?;
        return Ok(with_cookie(
            sign_in_error("invalid_state", None),
            clear_flow,
        ));
    };
    if let Some(error) = &query.error {
        let code = if error == "access_denied" {
            "cancelled_at_provider"
        } else {
            "login_failed"
        };
        return Ok(with_cookie(fail(&ctx, &flow, code).await?, clear_flow));
    }
    let Some(code) = &query.code else {
        return Ok(with_cookie(
            fail(&ctx, &flow, "login_failed").await?,
            clear_flow,
        ));
    };
    let signed_in = match identify(upstream, code, &query, &flow).await {
        Ok(found) => found,
        Err(code) => return Ok(with_cookie(fail(&ctx, &flow, code).await?, clear_flow)),
    };
    if flow.purpose != Purpose::SignIn {
        // Boxed: the round trip's hand-over (and its tracking) makes a large
        // future.
        let response = Box::pin(link::finish(
            &ctx, issuer, upstream, &headers, flow, signed_in, now,
        ))
        .await?;
        return Ok(with_cookie(response, clear_flow));
    }
    let Upstreamed { identity, tokens } = signed_in;
    let request = flow.request_id.as_deref();
    // The allow-list comes first, whether or not an account exists, so the
    // answer says nothing about accounts. Nothing is kept.
    if !ctx.permits_identity(&identity)? {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        tracing::info!(
            provider = provider.as_str(),
            "refused a sign-in the allow-list does not let in"
        );
        refused(
            &ctx,
            SignInMethod::Provider,
            Some(provider),
            "sign_in_not_allowed",
        )
        .await?;
        return Ok(with_cookie(
            sign_in_error("sign_in_not_allowed", request),
            clear_flow,
        ));
    }
    let response = match accounts::find(&ctx.state.db, &identity).await? {
        Some(Linked { disabled: true, .. }) => {
            refused(
                &ctx,
                SignInMethod::Provider,
                Some(provider),
                "account_suspended",
            )
            .await?;
            sign_in_error("account_suspended", request)
        }
        // A sign-in never changes a workspace's mailbox: a refresh token
        // the provider sent along is dropped, not stored.
        Some(linked) => {
            drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
            sign_in_known(&ctx, issuer, &headers, linked, &flow, provider, now).await?
        }
        None if !may_create_account(&ctx, &identity).await? => {
            // Nothing is kept: no pending sign-up, no tokens.
            drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
            refused(
                &ctx,
                SignInMethod::Provider,
                Some(provider),
                "signup_invite_only",
            )
            .await?;
            sign_in_error("signup_invite_only", request)
        }
        None => {
            let new = Unknown {
                identity,
                tokens,
                flow,
                now,
            };
            return Ok(match unknown_identity(&ctx, issuer, upstream, new).await? {
                // Its new round-trip cookie replaces this one.
                Next::AskAgain(response) => response,
                Next::Pending(response) => with_cookie(response, clear_flow),
            });
        }
    };
    Ok(with_cookie(response, clear_flow))
}

/// Where a refused round trip goes: back to the sign-in screen (the refusal
/// tracked) for a sign-in, back to `return_to` for a signed-in round trip.
async fn fail(ctx: &Ctx, flow: &Flow, code: &'static str) -> Result<Response> {
    if flow.purpose == Purpose::SignIn {
        refused(ctx, SignInMethod::Provider, Some(flow.provider), code).await?;
        Ok(sign_in_error(code, flow.request_id.as_deref()))
    } else {
        Ok(link::back(&flow.return_to, "link_error", code))
    }
}

/// The plan's kind of a refused sign-in's error `code`; `None` for what is
/// no sign-in attempt (a CSRF check, a rate limit, a malformed request).
pub(super) fn failure_of(code: &str) -> Option<SignInFailure> {
    Some(match code {
        "invalid_state" => SignInFailure::InvalidState,
        "cancelled_at_provider" => SignInFailure::CancelledAtProvider,
        "provider_unavailable" => SignInFailure::ProviderUnavailable,
        "login_failed" => SignInFailure::LoginFailed,
        "account_suspended" => SignInFailure::AccountSuspended,
        "signup_invite_only" => SignInFailure::SignupInviteOnly,
        "sign_in_not_allowed" => SignInFailure::SignInNotAllowed,
        "passkey_not_accepted" => SignInFailure::PasskeyNotAccepted,
        "passkey_expired" => SignInFailure::PasskeyExpired,
        _ => return None,
    })
}

/// Track a refused sign-in (error `code`, when it is one) in a transaction
/// of its own: the refusal changed nothing, and names no account.
pub(super) async fn refused(
    ctx: &Ctx,
    method: SignInMethod,
    provider: Option<Provider>,
    code: &str,
) -> Result<()> {
    let Some(failure) = failure_of(code) else {
        return Ok(());
    };
    if ctx.state.tracker.sink() == TrackingSink::None {
        return Ok(());
    }
    let mut tx = ctx.state.db.begin().await?;
    ctx.state
        .tracker
        .track(
            &mut tx,
            &tracking::SignInFailed {
                method,
                provider: provider.map(tracked::provider_of),
                failure,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(())
}

/// A round trip that ended with a provider identity Campbooks does not know.
struct Unknown {
    identity: ProviderIdentity,
    tokens: Tokens,
    flow: Flow,
    now: DateTime<Utc>,
}

/// Where a new account's round trip goes.
enum Next {
    /// Back to the provider (with a round-trip cookie of its own).
    AskAgain(Response),
    /// To the new-account screen.
    Pending(Response),
}

/// A new account's round trip: ask the provider again when the mailbox (or
/// its refresh token) is still to be asked for, else keep the identity for
/// the new-account screen.
async fn unknown_identity(
    ctx: &Ctx,
    issuer: &Issuer,
    upstream: &Upstream,
    new: Unknown,
) -> Result<Next> {
    let Unknown {
        identity,
        tokens,
        flow,
        now,
    } = new;
    let provider = flow.provider;
    let asked = asks_mailbox(&flow);
    let mail = MailGrant::of(ctx, provider, &tokens, asked);
    // A new Zoho account: now ask for its mailbox (see
    // `asks_mailbox`).
    let mailbox_offered = !ctx
        .state
        .grants
        .mailbox_scopes(provider.as_str())
        .is_empty();
    let ask_mailbox = !asked && mailbox_offered;
    if (ask_mailbox || (mail.granted && tokens.refresh_token.is_none())) && !flow.consent_retry {
        // Google sends a refresh token only on a fresh consent (this person
        // consented before), and a Zoho sign-in asked for no mailbox yet.
        // Ask once more, for this account.
        let retry = Flow {
            consent_retry: true,
            nonce: random_token(),
            code_verifier: random_token(),
            ..flow
        };
        let hint = match provider {
            Provider::Google => Some(identity.subject.as_str()),
            Provider::Microsoft | Provider::Zoho => identity.email.as_deref(),
        };
        return redirect_to_provider(
            ctx,
            issuer,
            upstream,
            retry,
            Prompt {
                consent: true,
                login_hint: hint,
            },
        )
        .await
        .map(Next::AskAgain);
    }
    pend_sign_up(ctx, issuer, upstream, identity, &tokens, mail, &flow, now)
        .await
        .map(Next::Pending)
}

/// Redeem the provider's code and validate its ID token; on failure, the
/// error code the browser is sent back with.
async fn identify(
    upstream: &Upstream,
    code: &str,
    query: &CallbackQuery,
    flow: &Flow,
) -> Result<Upstreamed, &'static str> {
    let provider = upstream.provider().as_str();
    upstream
        .sign_in(
            code,
            &flow.code_verifier,
            &flow.nonce,
            query.data_centre.as_deref(),
            query.iss.as_deref(),
        )
        .await
        .map_err(|error| match error {
            UpstreamError::Unavailable(reason) => {
                tracing::warn!(provider, %reason, "a provider could not finish a sign-in");
                "provider_unavailable"
            }
            error => {
                tracing::info!(provider, %error, "a provider refused to finish a sign-in");
                "login_failed"
            }
        })
}

/// Sign the account `linked` belongs to in, and move on.
async fn sign_in_known(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    linked: Linked,
    flow: &Flow,
    provider: Provider,
    now: DateTime<Utc>,
) -> Result<Response> {
    accounts::touch(&ctx.state.db, linked.identity_id, now).await?;
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let mut tx = ctx.state.db.begin().await?;
    let started = sessions::start(
        &mut tx,
        linked.account_id,
        linked.identity_id,
        user_agent,
        now,
    )
    .await?;
    signed_in(
        ctx,
        &mut tx,
        linked.account_id,
        provider,
        started.session.id,
        false,
        now,
    )
    .await?;
    tx.commit().await?;
    let location = continue_after_sign_in(
        ctx,
        issuer,
        headers,
        flow.request_id.as_deref(),
        &flow.return_to,
        &started.session,
        now,
    )
    .await?;
    Ok(with_cookie(
        see_other(&location),
        session_cookie(issuer, &started.token),
    ))
}

/// Whether the provider granted every mailbox scope this instance asked
/// for, and what it granted.
pub(super) struct MailGrant {
    pub granted: bool,
    pub scopes: Vec<String>,
}

impl MailGrant {
    /// What `tokens` granted of the mailbox scopes, when the round trip
    /// `asked` for them (nothing otherwise).
    pub(super) fn of(ctx: &Ctx, provider: Provider, tokens: &Tokens, asked: bool) -> Self {
        let wanted = if asked {
            ctx.state.grants.mailbox_scopes(provider.as_str())
        } else {
            Vec::new()
        };
        Self::from_answer(provider, tokens.scope.as_deref(), &wanted)
    }

    /// What a token answer's `scope` granted of the `wanted` mailbox scopes.
    /// Scopes are split on spaces and commas (Zoho documents commas). Zoho
    /// may leave `scope` out; its consent screen is all or nothing, so an
    /// answer without it grants what was asked.
    pub(super) fn from_answer(provider: Provider, scope: Option<&str>, wanted: &[String]) -> Self {
        let scopes: Vec<String> = match scope {
            None if provider == Provider::Zoho => wanted.to_vec(),
            scope => scope
                .unwrap_or_default()
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        };
        let granted = !wanted.is_empty() && wanted.iter().all(|s| scopes.contains(s));
        Self { granted, scopes }
    }
}

/// Keep `identity` (and its mailbox grant, when there is one with offline
/// access) for the new-account screen, bound to this browser.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
async fn pend_sign_up(
    ctx: &Ctx,
    issuer: &Issuer,
    upstream: &Upstream,
    identity: ProviderIdentity,
    tokens: &Tokens,
    mail: MailGrant,
    flow: &Flow,
    now: DateTime<Utc>,
) -> Result<Response> {
    if !mail.granted {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
    }
    let held = (mail.granted && tokens.refresh_token.is_some()).then(|| ProviderTokens {
        access_token: tokens.access_token.clone(),
        refresh_token: tokens.refresh_token.clone(),
        expires_in: tokens.expires_in,
        scopes: mail.scopes,
    });
    let browser = random_token();
    let pending = PendingSignUp {
        id: Uuid::now_v7(),
        identity,
        tokens: held,
        request_id: flow.request_id.clone(),
        return_to: flow.return_to.clone(),
    };
    upstream::store_sign_up(&ctx.state.db, &issuer.cipher, &browser, &pending, now).await?;
    let location = match &flow.request_id {
        Some(request) => format!("{NEW_ACCOUNT_PATH}?request={}", encode(request)),
        None => NEW_ACCOUNT_PATH.to_owned(),
    };
    Ok(with_cookie(
        see_other(&location),
        issuer.cookies.set(
            SIGN_UP_COOKIE,
            &browser,
            SIGN_UP_LIFETIME.num_seconds(),
            SameSite::Strict,
        ),
    ))
}

pub(super) fn session_cookie(issuer: &Issuer, token: &str) -> axum::http::HeaderValue {
    issuer.cookies.set(
        SESSION_COOKIE,
        token,
        sessions::ABSOLUTE.num_seconds(),
        SameSite::Lax,
    )
}

/// Record a sign-in in the audit trail, announce it, and track it with
/// the session it started (`session`).
async fn signed_in(
    ctx: &Ctx,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: Uuid,
    provider: Provider,
    session: Uuid,
    new_account: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    accounts::audit(
        tx,
        Some(account),
        "signed_in",
        serde_json::json!({ "method": "provider", "provider": provider.as_str() }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            tx,
            &SignedIn {
                account,
                method: "provider".to_owned(),
                provider: Some(provider.as_str().to_owned()),
            },
        )
        .await?;
    let session = tracked::session(tx, session).await?;
    ctx.state
        .tracker
        .track(
            tx,
            &tracking::SignInSucceeded {
                session,
                new_account,
            },
        )
        .await?;
    Ok(())
}

/// Where the browser goes once signed in: back to the client with a code
/// when an authorization request waits, else `return_to`.
pub(super) async fn continue_after_sign_in(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    request: Option<&str>,
    return_to: &str,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<String> {
    let browser = authorize::browser(issuer, headers);
    if let Some(request) = request
        && let Some(location) = authorize::complete_stored(
            &ctx.state.db,
            issuer,
            request,
            browser.as_deref(),
            session,
            now,
        )
        .await?
    {
        return Ok(location);
    }
    Ok(return_to.to_owned())
}

/// The pending new account, as the new-account screen shows it.
#[derive(Debug, Serialize, ToSchema)]
pub struct NewAccount {
    /// The provider signed in with.
    pub provider: String,
    /// The address it gave.
    pub email: Option<String>,
    /// The name it gave.
    pub display_name: Option<String>,
}

/// The new account this browser may create (after a round trip with an
/// identity no account uses).
#[utoipa::path(
    get,
    path = "/auth/new-account",
    tag = "identity",
    operation_id = "identity.get_new_account",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.pending_sign_up"]).access("identity.sign_in", campbooks_kernel::authz::Action::Read)))),
    security(()),
    responses(
        (status = 200, description = "The pending account.", body = NewAccount),
        (status = 404, description = "Nothing pending for this browser (or it expired)."),
    ),
)]
pub async fn new_account(State(ctx): State<Ctx>, headers: HeaderMap) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let token = issuer
        .cookies
        .read(&headers, SIGN_UP_COOKIE)
        .ok_or(Error::NotFound)?;
    let (provider, email, display_name) =
        upstream::peek_sign_up(&ctx.state.db, &token, issuer.now())
            .await?
            .ok_or(Error::NotFound)?;
    Ok(web::no_store(
        Json(NewAccount {
            provider,
            email,
            display_name,
        })
        .into_response(),
    ))
}

/// Where to go once the account exists.
#[derive(Debug, Serialize, ToSchema)]
pub struct Created {
    /// Navigate here: the client's redirect URI with a code, or `return_to`.
    pub redirect: String,
}

/// Create the pending account, sign in, and hold its mailbox grant.
#[utoipa::path(
    post,
    path = "/auth/new-account",
    tag = "identity",
    operation_id = "identity.create_account",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.account", "identity.session", "identity.held_grant"]).access("identity.sign_in", campbooks_kernel::authz::Action::Write)))),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    security(()),
    responses(
        (status = 201, description = "Created and signed in.", body = Created,
            headers(("set-cookie" = String))),
        (status = 403, description = "`csrf_check_failed`, `signup_invite_only` (this \
            instance takes invited people only; nothing was kept), or `sign_in_not_allowed` \
            (the address is not on the instance's allow-list; nothing was kept).", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 404, description = "Nothing pending for this browser."),
        (status = 409, description = "`sign_up_expired`, or `way_in_belongs_to_another_account` \
            (another account took this identity meanwhile).", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
#[allow(
    clippy::too_many_lines,
    reason = "one flow, read top to bottom; its tracked events sit next to the changes they record"
)]
pub async fn create_account(State(ctx): State<Ctx>, headers: HeaderMap) -> Result<Response> {
    let issuer = ctx.issuer()?;
    if !web::csrf_passes(&headers, &issuer.url) {
        return Err(Error::Denied(MESSAGES.reason("csrf_check_failed")));
    }
    let token = issuer
        .cookies
        .read(&headers, SIGN_UP_COOKIE)
        .ok_or(Error::NotFound)?;
    let now = issuer.now();
    let mut tx = ctx.state.db.begin().await?;
    let pending = upstream::take_sign_up(&mut tx, &issuer.cipher, &token, now)
        .await?
        .ok_or_else(|| Error::Conflict(MESSAGES.reason("sign_up_expired")))?;
    let pending_provider = tracked::provider(&pending.identity.provider);
    if !ctx.permits_identity(&pending.identity)? {
        // The allow-list may have been tightened since the round trip.
        track_refusal(
            &ctx,
            &mut tx,
            pending_provider,
            SignInFailure::SignInNotAllowed,
        )
        .await?;
        tx.commit().await?;
        return Err(Error::Denied(MESSAGES.reason("sign_in_not_allowed")));
    }
    if !may_create_account(&ctx, &pending.identity).await? {
        // The pending sign-up (and its tokens) is gone with this commit.
        track_refusal(
            &ctx,
            &mut tx,
            pending_provider,
            SignInFailure::SignupInviteOnly,
        )
        .await?;
        tx.commit().await?;
        return Err(Error::Denied(MESSAGES.reason("signup_invite_only")));
    }
    let Some(linked) = accounts::create(&mut tx, &pending.identity, now).await? else {
        return Err(Error::Conflict(
            MESSAGES.reason("way_in_belongs_to_another_account"),
        ));
    };
    let provider = Provider::parse(&pending.identity.provider)
        .ok_or_else(|| Error::internal("a pending sign-up names an unknown provider"))?;
    let mailbox_held = pending.tokens.is_some() && pending.identity.email.is_some();
    if let (Some(tokens), Some(address)) = (&pending.tokens, &pending.identity.email) {
        held_grants::hold(
            &mut tx,
            &issuer.cipher,
            &NewHeldGrant {
                account_id: linked.account_id,
                identity_id: linked.identity_id,
                provider: provider.as_str(),
                address,
                address_verified: pending.identity.email_verified,
                display_name: pending.identity.name.as_deref(),
                tokens,
            },
            now,
        )
        .await?;
    }
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let started = sessions::start(
        &mut tx,
        linked.account_id,
        linked.identity_id,
        user_agent,
        now,
    )
    .await?;
    accounts::audit(
        &mut tx,
        Some(linked.account_id),
        "account_created",
        serde_json::json!({ "provider": provider.as_str(), "mailbox_held": pending.tokens.is_some() }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            &mut tx,
            &AccountCreated {
                account: linked.account_id,
                provider: provider.as_str().to_owned(),
            },
        )
        .await?;
    let account = tracked::account(&mut tx, linked.account_id).await?;
    ctx.state
        .tracker
        .track(
            &mut tx,
            &tracking::AccountCreated {
                account,
                method: SignInMethod::Provider,
                provider: Some(tracked::provider_of(provider)),
                mailbox_held,
            },
        )
        .await?;
    signed_in(
        &ctx,
        &mut tx,
        linked.account_id,
        provider,
        started.session.id,
        true,
        now,
    )
    .await?;
    tx.commit().await?;
    let redirect = continue_after_sign_in(
        &ctx,
        issuer,
        &headers,
        pending.request_id.as_deref(),
        &pending.return_to,
        &started.session,
        now,
    )
    .await?;
    let response = (axum::http::StatusCode::CREATED, Json(Created { redirect })).into_response();
    let response = with_cookie(
        web::no_store(response),
        session_cookie(issuer, &started.token),
    );
    Ok(with_cookie(
        response,
        issuer.cookies.clear(SIGN_UP_COOKIE, SameSite::Strict),
    ))
}

/// Track a refused sign-up in `tx`, which commits what the refusal took
/// (the pending sign-up).
async fn track_refusal(
    ctx: &Ctx,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    provider: Option<tracking::Provider>,
    failure: SignInFailure,
) -> Result<()> {
    ctx.state
        .tracker
        .track(
            tx,
            &tracking::SignInFailed {
                method: SignInMethod::Provider,
                provider,
                failure,
            },
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MailGrant, Provider};

    fn wanted() -> Vec<String> {
        ["ZohoMail.accounts.READ", "ZohoMail.messages.ALL"]
            .map(str::to_owned)
            .to_vec()
    }

    #[test]
    fn granted_scopes_split_on_spaces_and_commas() {
        let commas = MailGrant::from_answer(
            Provider::Zoho,
            Some("openid,ZohoMail.accounts.READ,ZohoMail.messages.ALL"),
            &wanted(),
        );
        assert!(commas.granted);
        let spaces = MailGrant::from_answer(
            Provider::Google,
            Some("openid ZohoMail.accounts.READ, ZohoMail.messages.ALL"),
            &wanted(),
        );
        assert!(spaces.granted);
        let partial =
            MailGrant::from_answer(Provider::Zoho, Some("ZohoMail.accounts.READ"), &wanted());
        assert!(!partial.granted);
    }

    #[test]
    fn a_zoho_answer_without_scope_grants_what_was_asked() {
        let zoho = MailGrant::from_answer(Provider::Zoho, None, &wanted());
        assert!(zoho.granted);
        assert_eq!(zoho.scopes, wanted());
        // Google always says what it granted.
        assert!(!MailGrant::from_answer(Provider::Google, None, &wanted()).granted);
        // Nothing asked, nothing granted.
        assert!(!MailGrant::from_answer(Provider::Zoho, None, &[]).granted);
    }
}
