//! The `/bff/*` endpoints.

use axum::{
    Form, Json,
    body::Body,
    extract::{MatchedPath, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    Error, Problem,
    error::LocalizableProblem,
    i18n,
    operations::{Operation, Risk},
    rate_limit::{ClientIp, RateKey},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tower::ServiceExt as _;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    AccessError, Bff, CSRF_HEADER, Inner, crypto, expires_in, oidc::AuthorizationRequest,
    oidc::OidcError, sealed_for, seconds_until, store, user_agent,
};

/// Longest accepted `return_to`.
const MAX_RETURN_TO: usize = 2048;

/// Problem codes of the BFF (also listed in the OpenAPI document), each
/// explained by `errors.<code>` in the server's catalogue.
mod code {
    /// The BFF has no identity provider or client secret configured.
    pub const NOT_CONFIGURED: &str = "bff_not_configured";
    /// `/bff/api/*` without a live session: sign in at `/bff/login`.
    pub const SESSION_REQUIRED: &str = "session_required";
    /// `return_to` is not a path on this site.
    pub const INVALID_RETURN_TO: &str = "invalid_return_to";
    /// The callback's `state` names no sign-in started by this browser, or
    /// one already finished or expired.
    pub const INVALID_STATE: &str = "invalid_state";
    /// The IdP reported an error, or named another issuer.
    pub const LOGIN_FAILED: &str = "login_failed";
    /// The IdP refused the authorization code.
    pub const CODE_REJECTED: &str = "code_rejected";
    /// The ID token did not validate.
    pub const INVALID_ID_TOKEN: &str = "invalid_id_token";
    /// The back-channel logout token did not validate.
    pub const INVALID_LOGOUT_TOKEN: &str = "invalid_logout_token";
}

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        login, callback, session, logout, backchannel_logout,
        api_get, api_post, api_put, api_patch, api_delete,
        admin_get, admin_post, admin_put, admin_patch, admin_delete,
    ),
    components(schemas(SessionInfo, SessionUser, LogoutResponse)),
    tags((name = "bff", description = "The web app's backend-for-frontend: sign-in, the \
        session cookie and same-origin API access. Browsers only; other clients call `/api/*` \
        with their own OAuth token.")),
)]
pub struct BffDoc;

fn problem(status: StatusCode, code: &'static str) -> Response {
    LocalizableProblem::new(status, crate::MESSAGES.reason(code)).into_response()
}

fn not_configured() -> Response {
    problem(StatusCode::SERVICE_UNAVAILABLE, code::NOT_CONFIGURED)
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn with_cookie(mut response: Response, cookie: HeaderValue) -> Response {
    response.headers_mut().append(header::SET_COOKIE, cookie);
    response
}

fn oidc_failure(err: &OidcError) -> Response {
    tracing::warn!(error = %err, "web sign-in: identity provider call failed");
    match err {
        OidcError::Rejected(_) => problem(StatusCode::BAD_REQUEST, code::CODE_REJECTED),
        OidcError::Unavailable(_) => {
            Error::Unavailable(i18n::MESSAGES.reason("idp_unavailable")).into_response()
        }
    }
}

// --- login ---------------------------------------------------------------------

#[derive(Debug, Deserialize, IntoParams)]
pub struct LoginQuery {
    /// Where to land after signing in: a path on this site (default `/`).
    return_to: Option<String>,
    /// Sign in again unless the person did within this many seconds: the
    /// step-up an operation asks for with `step_up_required`, or `0` after
    /// the admin API's `admin_reauth_required`.
    max_age: Option<u32>,
}

/// Start signing in.
///
/// Creates a short-lived sign-in (state, nonce, PKCE verifier) bound to this
/// browser by a `SameSite=Lax` login cookie, and redirects to the identity
/// provider's authorization endpoint.
#[utoipa::path(
    get,
    path = "/bff/login",
    tag = "bff",
    operation_id = "bff.login",
    extensions(("x-campbooks" = json!(Operation::query().records(["bff.login_transaction"]).access("bff.login", campbooks_kernel::authz::Action::Write)))),
    params(LoginQuery),
    security(()),
    responses(
        (status = 303, description = "Redirect to the identity provider.",
            headers(("location" = String), ("set-cookie" = String))),
        (status = 400, description = "`invalid_return_to`", body = Problem,
            content_type = "application/problem+json"),
        (status = 429, description = "`rate_limited`: too many sign-ins started from this \
            address.", body = Problem, content_type = "application/problem+json"),
        (status = 503, description = "`bff_not_configured`, or the identity provider is unreachable.",
            body = Problem, content_type = "application/problem+json"),
    ),
)]
pub async fn login(
    State(bff): State<Bff>,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
) -> Response {
    let Some(inner) = &bff.inner else {
        return not_configured();
    };
    let ip = ClientIp::resolve(
        peer.map(|axum::Extension(axum::extract::ConnectInfo(addr))| addr.ip()),
        &headers,
        &inner.trusted_proxies,
    );
    if let Err(limited) = inner
        .rate_limits
        .check(&inner.db, &super::LOGIN_LIMIT, &RateKey::ip(ip))
        .await
    {
        return limited.into_response();
    }
    let Some(return_to) = return_to(query.return_to.as_deref()) else {
        return problem(StatusCode::BAD_REQUEST, code::INVALID_RETURN_TO);
    };
    no_store(inner.start_login(return_to, query.max_age).await)
}

/// A same-origin path to land on: `/`-rooted, not protocol-relative, no
/// backslash, quotes, angle brackets, whitespace or control characters, not
/// under `/bff`. `None` when unacceptable; missing means `/`.
pub(super) fn return_to(value: Option<&str>) -> Option<String> {
    let Some(value) = value else {
        return Some("/".to_owned());
    };
    let acceptable = value.starts_with('/')
        && !value.starts_with("//")
        && value.len() <= MAX_RETURN_TO
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\\\"'<>`".contains(&b))
        && value != "/bff"
        && !value.starts_with("/bff/")
        && !value.starts_with("/bff?")
        && !value.starts_with("/bff#");
    acceptable.then(|| value.to_owned())
}

impl Inner {
    async fn start_login(&self, return_to: String, max_age: Option<u32>) -> Response {
        let state = crypto::random_token();
        let nonce = crypto::random_token();
        let verifier = crypto::random_token();
        let browser = crypto::random_token();
        let challenge = crypto::pkce_challenge(&verifier);
        let url = match self
            .oidc
            .authorization_url(&AuthorizationRequest {
                state: &state,
                nonce: &nonce,
                code_challenge: &challenge,
                max_age,
            })
            .await
        {
            Ok(url) => url,
            Err(err) => return oidc_failure(&err),
        };
        let state_hash = crypto::hash(&state);
        let now = Utc::now();
        let login = store::LoginTransaction {
            code_verifier: self.cipher.seal(&verifier, &verifier_context(&state_hash)),
            nonce: self.cipher.seal(&nonce, &nonce_context(&state_hash)),
            state_hash,
            browser_hash: crypto::hash(&browser),
            return_to,
            created_at: now,
            expires_at: now + self.timeouts.login,
            scopes: Some(self.oidc.scopes().to_owned()),
        };
        if let Err(err) = store::insert_login(&self.db, &login).await {
            return Error::from(err).into_response();
        }
        let Ok(location) = HeaderValue::from_str(&url) else {
            return Error::internal("the authorization URL is not a valid header").into_response();
        };
        let mut response = StatusCode::SEE_OTHER.into_response();
        response.headers_mut().insert(header::LOCATION, location);
        with_cookie(
            response,
            self.cookies
                .set_login(&browser, self.timeouts.login.num_seconds()),
        )
    }
}

/// Associated data of a sign-in's sealed PKCE verifier.
pub(super) fn verifier_context(state_hash: &[u8]) -> Vec<u8> {
    login_context(state_hash, "code_verifier")
}

/// Associated data of a sign-in's sealed nonce.
pub(super) fn nonce_context(state_hash: &[u8]) -> Vec<u8> {
    login_context(state_hash, "nonce")
}

/// Associated data binding a sealed sign-in value to its row and column.
fn login_context(state_hash: &[u8], column: &str) -> Vec<u8> {
    format!(
        "bff.login_transactions:{}:{column}",
        URL_SAFE_NO_PAD.encode(state_hash)
    )
    .into_bytes()
}

// --- callback ------------------------------------------------------------------

#[derive(Debug, Deserialize, IntoParams)]
pub struct CallbackQuery {
    /// The authorization code.
    code: Option<String>,
    /// The `state` sent with the authorization request.
    state: Option<String>,
    /// The issuer that answered (RFC 9207).
    iss: Option<String>,
    /// An error code instead of a code.
    error: Option<String>,
}

/// Finish signing in.
///
/// The identity provider redirects here. Checks that `state` names a sign-in
/// this browser started (single use), exchanges the code with the PKCE
/// verifier, validates the ID token (issuer, audience, nonce, expiry,
/// signature), starts a session and sets the `SameSite=Strict` session
/// cookie. Answers with a small same-origin page that moves on to
/// `return_to`: a browser treats the whole redirect chain from the IdP as
/// cross-site and would not send the new cookie on a plain redirect.
#[utoipa::path(
    get,
    path = "/bff/callback",
    tag = "bff",
    operation_id = "bff.callback",
    extensions(("x-campbooks" = json!(Operation::query().records(["bff.login_transaction", "bff.session"]).access("bff.login", campbooks_kernel::authz::Action::Write)))),
    params(CallbackQuery),
    security(()),
    responses(
        (status = 200, description = "Signed in; the page moves on to `return_to`.",
            content_type = "text/html", headers(("set-cookie" = String))),
        (status = 400, description = "`invalid_state`, `login_failed`, `code_rejected` or \
            `invalid_id_token`.", body = Problem, content_type = "application/problem+json"),
        (status = 503, description = "`bff_not_configured`, or the identity provider is unreachable.",
            body = Problem, content_type = "application/problem+json"),
    ),
)]
pub async fn callback(
    State(bff): State<Bff>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(inner) = &bff.inner else {
        return not_configured();
    };
    let response = inner.finish_login(query, &headers).await;
    // The login cookie is single-use, whatever happened.
    no_store(with_cookie(response, inner.cookies.clear_login()))
}

impl Inner {
    async fn finish_login(&self, query: CallbackQuery, headers: &HeaderMap) -> Response {
        let invalid_state = || problem(StatusCode::BAD_REQUEST, code::INVALID_STATE);
        let Some(state) = query.state.as_deref() else {
            return invalid_state();
        };
        let state_hash = crypto::hash(state);
        let login = match store::take_login(&self.db, &state_hash).await {
            Ok(Some(login)) => login,
            Ok(None) => return invalid_state(),
            Err(err) => return Error::from(err).into_response(),
        };
        let from_this_browser = self
            .cookies
            .login(headers)
            .is_some_and(|browser| crypto::same_hash(&crypto::hash(&browser), &login.browser_hash));
        if !from_this_browser || login.expires_at <= Utc::now() {
            return invalid_state();
        }
        if query.error.is_some()
            || query
                .iss
                .as_deref()
                .is_some_and(|iss| iss != self.oidc.issuer())
        {
            return problem(StatusCode::BAD_REQUEST, code::LOGIN_FAILED);
        }
        let Some(code) = query.code.as_deref() else {
            return problem(StatusCode::BAD_REQUEST, code::LOGIN_FAILED);
        };
        let Some((verifier, nonce)) = self.open_login(&login) else {
            return invalid_state();
        };
        let tokens = match self.oidc.exchange_code(code, &verifier).await {
            Ok(tokens) => tokens,
            Err(err) => return oidc_failure(&err),
        };
        let Some(id_token) = tokens.id_token.as_deref() else {
            return problem(StatusCode::BAD_REQUEST, code::INVALID_ID_TOKEN);
        };
        let identity = match self
            .verifier
            .verify_id_token(id_token, self.oidc.client_id(), &nonce)
            .await
        {
            Ok(identity) => identity,
            Err(Error::Unavailable(reason)) => return Error::Unavailable(reason).into_response(),
            Err(_) => return problem(StatusCode::BAD_REQUEST, code::INVALID_ID_TOKEN),
        };

        // A sign-in always starts a new session with a new id; one this
        // browser already had ends here, its refresh token revoked
        // (campbooks-cloud#214), the identity provider's session left as it
        // is (this sign-in may have gone straight through it).
        if let Some(previous) = self.cookies.session(headers)
            && let Err(err) = self.retire(&crypto::hash(&previous)).await
        {
            return Error::from(err).into_response();
        }
        let id = Uuid::now_v7();
        let token = crypto::random_token();
        let now = Utc::now();
        let expires_at = now + self.timeouts.absolute;
        let access = self
            .cipher
            .seal(&tokens.access_token, &sealed_for(id, "access_token"));
        let refresh = tokens
            .refresh_token
            .as_deref()
            .map(|t| self.cipher.seal(t, &sealed_for(id, "refresh_token")));
        let sealed_id = self.cipher.seal(id_token, &sealed_for(id, "id_token"));
        let agent = user_agent(headers);
        let session = store::NewSession {
            id,
            token_hash: &crypto::hash(&token),
            subject: &identity.subject,
            idp_session_id: identity.session_id.as_deref(),
            name: identity.name.as_deref(),
            email: identity.email.as_deref(),
            access_token: &access,
            access_expires_at: now + expires_in(tokens.expires_in),
            refresh_token: refresh.as_deref(),
            id_token: &sealed_id,
            user_agent: agent.as_deref(),
            now,
            idle_expires_at: (now + self.timeouts.idle).min(expires_at),
            expires_at,
            // What this sign-in asked for when it started (a deploy may have
            // come in between): its tokens carry those, not today's.
            scopes: login.scopes.as_deref(),
        };
        if let Err(err) = store::insert_session(&self.db, &session).await {
            return Error::from(err).into_response();
        }
        tracing::info!("web sign-in completed");
        with_cookie(
            landing_page(&login.return_to),
            self.cookies
                .set_session(&token, seconds_until(expires_at, now)),
        )
    }
}

impl Inner {
    /// A sign-in's sealed PKCE verifier and nonce, opened; `None` when either
    /// does not open (another key, or another row's value).
    fn open_login(&self, login: &store::LoginTransaction) -> Option<(String, String)> {
        let verifier = self
            .cipher
            .open(&login.code_verifier, &verifier_context(&login.state_hash))
            .ok()?;
        let nonce = self
            .cipher
            .open(&login.nonce, &nonce_context(&login.state_hash))
            .ok()?;
        Some((verifier, nonce))
    }
}

/// `Content-Security-Policy` of the landing page: no scripts, nothing loaded.
const LANDING_CSP: &str =
    "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// A same-origin page sending the browser on to `return_to` (already
/// validated by [`return_to`], so it holds no quote or angle bracket).
fn landing_page(return_to: &str) -> Response {
    let href = return_to.replace('&', "&amp;");
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"refresh\" content=\"0;url={href}\"><title>Campbooks</title></head>\
         <body><a href=\"{href}\">Campbooks</a></body></html>"
    );
    let mut response = (StatusCode::OK, html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(LANDING_CSP),
    );
    // The URL that led here carried the code and state.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

// --- session -------------------------------------------------------------------

/// Who is signed in. Never contains a token.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    /// Whether the request carries a live session.
    pub authenticated: bool,
    /// The signed-in user, when authenticated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<SessionUser>,
    /// When the session ends at the latest (it ends sooner if left unused).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// The signed-in user, as the identity provider describes them.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionUser {
    /// Stable user id at the identity provider.
    pub sub: String,
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// The current session.
///
/// For the web app to bootstrap: whether someone is signed in, and who.
/// Never returns a token.
#[utoipa::path(
    get,
    path = "/bff/session",
    tag = "bff",
    operation_id = "bff.session",
    extensions(("x-campbooks" = json!(Operation::query().records(["bff.session"]).access("bff.session", campbooks_kernel::authz::Action::Read)))),
    security((), ("session" = [])),
    responses(
        (status = 200, description = "The session, or `authenticated: false`.", body = SessionInfo),
    ),
)]
pub async fn session(State(bff): State<Bff>, headers: HeaderMap) -> Result<Response, Error> {
    let signed_out = Json(SessionInfo {
        authenticated: false,
        user: None,
        expires_at: None,
    });
    let Some(inner) = &bff.inner else {
        return Ok(no_store(signed_out.into_response()));
    };
    let Some(current) = inner.current(&headers).await? else {
        let response = signed_out.into_response();
        let response = if inner.cookies.session(&headers).is_some() {
            with_cookie(response, inner.cookies.clear_session())
        } else {
            response
        };
        return Ok(no_store(response));
    };
    let session = current.session;
    let mut response = Json(SessionInfo {
        authenticated: true,
        user: Some(SessionUser {
            sub: session.subject,
            name: session.name,
            email: session.email,
        }),
        expires_at: Some(session.expires_at),
    })
    .into_response();
    if let Some(rotated) = current.rotated {
        response = with_cookie(response, rotated);
    }
    Ok(no_store(response))
}

// --- logout --------------------------------------------------------------------

/// Where to send the browser to sign out of the identity provider too.
#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {
    /// The identity provider's end-session URL (RP-initiated logout), which
    /// comes back to the app's root; `null` when it has none.
    pub logout_url: Option<String>,
}

/// Sign out.
///
/// Ends the session, revokes its refresh token at the identity provider and
/// clears the cookie. Navigate to `logout_url` to end the identity
/// provider's session too.
#[utoipa::path(
    post,
    path = "/bff/logout",
    tag = "bff",
    operation_id = "bff.logout",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["bff.session"]).access("bff.session", campbooks_kernel::authz::Action::Delete)))),
    security(("session" = [])),
    params(("x-campbooks-csrf" = String, Header, description = "Must be `1`.")),
    responses(
        (status = 200, description = "Signed out (also when there was no session).",
            body = LogoutResponse, headers(("set-cookie" = String))),
        (status = 403, description = "`csrf_check_failed`", body = Problem,
            content_type = "application/problem+json"),
        (status = 503, description = "`bff_not_configured`", body = Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn logout(State(bff): State<Bff>, headers: HeaderMap) -> Result<Response, Error> {
    let Some(inner) = &bff.inner else {
        return Ok(not_configured());
    };
    let mut ended_here = false;
    // A session that predates the scopes asked for now still signs out in
    // full here (the identity provider's session too), rather than being
    // retired by the lookup and leaving that session behind.
    if let Some(current) = inner.current_any_scopes(&headers).await? {
        ended_here = inner.terminate(&current.session.presented).await?;
        tracing::info!("web sign-out");
    }
    let logout_url = if ended_here || inner.oidc.in_process() {
        None
    } else {
        inner.oidc.end_session_url().await.unwrap_or_else(|err| {
            tracing::warn!(error = %err, "no end-session URL for a sign-out");
            None
        })
    };
    Ok(no_store(with_cookie(
        Json(LogoutResponse { logout_url }).into_response(),
        inner.cookies.clear_session(),
    )))
}

// --- back-channel logout -------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct BackchannelLogout {
    /// The identity provider's signed logout token.
    logout_token: String,
}

/// Back-channel logout (OIDC Back-Channel Logout 1.0).
///
/// Called by the identity provider, not browsers, when a user's IdP session
/// ends: ends the web sessions the signed logout token names (by IdP session
/// id, or every session of the user).
#[utoipa::path(
    post,
    path = "/bff/backchannel-logout",
    tag = "bff",
    operation_id = "bff.backchannel_logout",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["bff.session"]).access("bff.login", campbooks_kernel::authz::Action::Delete)))),
    security(()),
    request_body(content = BackchannelLogout, content_type = "application/x-www-form-urlencoded"),
    responses(
        (status = 200, description = "The sessions named by the token are ended."),
        (status = 400, description = "`invalid_logout_token`", body = Problem,
            content_type = "application/problem+json"),
        (status = 503, description = "`bff_not_configured`, or the identity provider's keys are unavailable.",
            body = Problem, content_type = "application/problem+json"),
    ),
)]
pub async fn backchannel_logout(
    State(bff): State<Bff>,
    Form(form): Form<BackchannelLogout>,
) -> Result<Response, Error> {
    let Some(inner) = &bff.inner else {
        return Ok(not_configured());
    };
    let token = match inner
        .verifier
        .verify_logout_token(&form.logout_token, inner.oidc.client_id())
        .await
    {
        Ok(token) => token,
        Err(Error::Unavailable(reason)) => return Err(Error::Unavailable(reason)),
        Err(_) => {
            return Ok(no_store(problem(
                StatusCode::BAD_REQUEST,
                code::INVALID_LOGOUT_TOKEN,
            )));
        }
    };
    let ended = store::delete_for_logout(
        &inner.db,
        token.subject.as_deref(),
        token.session_id.as_deref(),
    )
    .await?;
    tracing::info!(sessions = ended, "back-channel logout");
    Ok(no_store(StatusCode::OK.into_response()))
}

// --- the API -------------------------------------------------------------------

/// Documents `/bff/api/{path}` for one HTTP method; every method is served
/// by [`api`]. One operation per method keeps operation ids unique.
macro_rules! api_operation {
    ($name:ident, $method:ident, $id:literal, $meta:tt, $summary:literal) => {
        #[doc = $summary]
        ///
        /// `/bff/api/<path>` is `/api/<path>` (same method, query, headers and
        /// body) with the session's access token attached, refreshed first when
        /// it is about to expire. The token never leaves the server. Answers are
        /// the API's own; without a live session the answer is 401
        /// `session_required` (sign in at `/bff/login`). POST, PUT, PATCH and
        /// DELETE need the `X-Campbooks-CSRF: 1` header from the app's origin.
        #[utoipa::path(
            $method,
            path = "/bff/api/{path}",
            tag = "bff",
            operation_id = $id,
            extensions(("x-campbooks" = json!($meta))),
            security(("session" = [])),
            params(
                ("path" = String, Path, description = "The API path after `/api/`, e.g. \
                    `v1/system/version`; may contain slashes."),
            ),
            responses(
                (status = 200, description = "The API's response, whatever its status."),
                (status = 401, description = "`session_required`: no live session.",
                    body = Problem, content_type = "application/problem+json"),
                (status = 403, description = "`csrf_check_failed`", body = Problem,
                    content_type = "application/problem+json"),
                (status = 503, description = "`bff_not_configured`, or the identity provider \
                    is unavailable while the access token needs refreshing.", body = Problem,
                    content_type = "application/problem+json"),
            ),
        )]
        pub async fn $name(state: State<Bff>, request: Request) -> Result<Response, Error> {
            api(state, request).await
        }
    };
}

api_operation!(
    api_get,
    get,
    "bff.api_get",
    (Operation::query().access("bff.proxy", campbooks_kernel::authz::Action::Read)),
    "Read from the API as the signed-in user."
);
api_operation!(
    api_post,
    post,
    "bff.api_post",
    (Operation::command(Risk::Low).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "POST to the API as the signed-in user."
);
api_operation!(
    api_put,
    put,
    "bff.api_put",
    (Operation::command(Risk::Low).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "PUT to the API as the signed-in user."
);
api_operation!(
    api_patch,
    patch,
    "bff.api_patch",
    (Operation::command(Risk::Low).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "PATCH the API as the signed-in user."
);
api_operation!(
    api_delete,
    delete,
    "bff.api_delete",
    (Operation::command(Risk::Low).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "DELETE in the API as the signed-in user."
);

/// Documents `/bff/admin/{path}` for one HTTP method; every method is served
/// by [`admin`].
macro_rules! admin_operation {
    ($name:ident, $method:ident, $id:literal, $meta:tt, $summary:literal) => {
        #[doc = $summary]
        ///
        /// `/bff/admin/<path>` is `/admin/<path>` (the admin API, ADR 0015) with
        /// the session's access token attached, exactly like `/bff/api/*`. Its
        /// answers are the admin API's own: 404 for anyone who is not an
        /// administrator with the operation's permission, 401
        /// `admin_reauth_required` when the last sign-in is more than 1 hour
        /// old (sign in again at `/bff/login?max_age=0`). Without a live session
        /// the answer is 401 `session_required`. POST, PUT, PATCH and DELETE
        /// need the `X-Campbooks-CSRF: 1` header from the app's origin.
        #[utoipa::path(
            $method,
            path = "/bff/admin/{path}",
            tag = "bff",
            operation_id = $id,
            extensions(("x-campbooks" = json!($meta))),
            security(("session" = [])),
            params(
                ("path" = String, Path, description = "The admin API path after `/admin/`, \
                    e.g. `v1/system/overview`; may contain slashes."),
            ),
            responses(
                (status = 200, description = "The admin API's response, whatever its status."),
                (status = 401, description = "`session_required`: no live session.",
                    body = Problem, content_type = "application/problem+json"),
                (status = 403, description = "`csrf_check_failed`", body = Problem,
                    content_type = "application/problem+json"),
                (status = 503, description = "`bff_not_configured`, or the identity provider \
                    is unavailable while the access token needs refreshing.", body = Problem,
                    content_type = "application/problem+json"),
            ),
        )]
        pub async fn $name(state: State<Bff>, request: Request) -> Result<Response, Error> {
            admin(state, request).await
        }
    };
}

admin_operation!(
    admin_get,
    get,
    "bff.admin_get",
    (Operation::query().access("bff.proxy", campbooks_kernel::authz::Action::Read)),
    "Read from the admin API as the signed-in administrator."
);
admin_operation!(
    admin_post,
    post,
    "bff.admin_post",
    (Operation::command(Risk::High).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "POST to the admin API as the signed-in administrator."
);
admin_operation!(
    admin_put,
    put,
    "bff.admin_put",
    (Operation::command(Risk::High).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "PUT to the admin API as the signed-in administrator."
);
admin_operation!(
    admin_patch,
    patch,
    "bff.admin_patch",
    (Operation::command(Risk::High).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "PATCH the admin API as the signed-in administrator."
);
admin_operation!(
    admin_delete,
    delete,
    "bff.admin_delete",
    (Operation::command(Risk::High).access("bff.proxy", campbooks_kernel::authz::Action::Write)),
    "DELETE in the admin API as the signed-in administrator."
);

/// `/bff/api/*`: the API, with the session's access token attached.
async fn api(State(bff): State<Bff>, request: Request) -> Result<Response, Error> {
    forward(&bff, request, Target::Api).await
}

/// `/bff/admin/*`: the admin API, with the session's access token attached.
async fn admin(State(bff): State<Bff>, request: Request) -> Result<Response, Error> {
    forward(&bff, request, Target::Admin).await
}

/// Which in-process router a BFF request runs through.
#[derive(Clone, Copy)]
enum Target {
    Api,
    Admin,
}

/// Run `request` through `target` as the session's user.
async fn forward(bff: &Bff, request: Request, target: Target) -> Result<Response, Error> {
    let Some(inner) = &bff.inner else {
        return Ok(not_configured());
    };
    let session_required = || {
        with_cookie(
            problem(StatusCode::UNAUTHORIZED, code::SESSION_REQUIRED),
            inner.cookies.clear_session(),
        )
    };
    let Some(current) = inner.current(request.headers()).await? else {
        return Ok(session_required());
    };
    let access_token = match inner.access_token(&current.session).await {
        Ok(token) => token,
        Err(AccessError::SessionEnded) => return Ok(session_required()),
        Err(AccessError::Failed(err)) => return Err(err),
    };
    // A request with no body, or a small one held here, can be sent again,
    // once, with a fresh token.
    let (request, again) = replayable(request).await?;
    let router = match target {
        Target::Api => inner.api.clone(),
        Target::Admin => inner.admin.clone(),
    };
    let request = to_api_request(request, &access_token)?;
    let mut response = match router.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    // The API refused the token as invalid: its sign-in ended (a way in
    // removed, signed out elsewhere, suspended; campbooks-cloud#143), or
    // it expired. One forced refresh: refused too, the session ends and the
    // app signs in again, instead of failing until the token would have
    // expired. The admin API answers 404, never this, to a token it cannot
    // accept, so `/bff/admin` goes through here without ever refreshing.
    if refused_as_invalid(&response) {
        match inner.refresh(&current.session, Some(&access_token)).await {
            Ok(fresh) => {
                if let Some(again) = again {
                    let request = to_api_request(again, &fresh)?;
                    response = match router.oneshot(request).await {
                        Ok(response) => response,
                        Err(never) => match never {},
                    };
                }
            }
            Err(AccessError::SessionEnded) => return Ok(session_required()),
            Err(AccessError::Failed(err)) => return Err(err),
        }
    }
    Ok(match current.rotated {
        Some(rotated) => with_cookie(response, rotated),
        None => response,
    })
}

/// The largest request body held to send again after a forced refresh.
pub(super) const REPLAY_BODY_LIMIT: usize = 1024 * 1024;

/// `request`, and a copy of it to send again after a forced refresh when
/// that is safe (campbooks-cloud#259): its body is empty (a `GET`, a
/// `HEAD`, most `DELETE`s) or of a known length of at most
/// [`REPLAY_BODY_LIMIT`] bytes, read here into memory. A write is safe to
/// send again because the API refuses a token as invalid (401
/// `invalid_token`) while authenticating, before any handler runs, so the
/// refused attempt changed nothing; and the copy carries the same
/// `Idempotency-Key`, so the idempotency layer still runs it at most once.
/// A larger body, or one of unknown length (streamed), is passed through
/// untouched and not sent again: the API's refusal comes back, with the
/// fresh token held for the app's next try.
async fn replayable(request: Request) -> Result<(Request, Option<Request>), Error> {
    use axum::body::HttpBody as _;

    let Some(length) = request
        .body()
        .size_hint()
        .exact()
        .and_then(|length| usize::try_from(length).ok())
        .filter(|length| *length <= REPLAY_BODY_LIMIT)
    else {
        return Ok((request, None));
    };
    let copy = |request: &Request, body: axum::body::Bytes| {
        let mut again = Request::new(Body::from(body));
        *again.method_mut() = request.method().clone();
        *again.uri_mut() = request.uri().clone();
        *again.version_mut() = request.version();
        *again.headers_mut() = request.headers().clone();
        // The peer address and the like, as the first attempt had them.
        *again.extensions_mut() = request.extensions().clone();
        again
    };
    if length == 0 {
        let again = copy(&request, axum::body::Bytes::new());
        return Ok((request, Some(again)));
    }
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, REPLAY_BODY_LIMIT)
        .await
        .map_err(|_| {
            Error::BadRequest(
                campbooks_kernel::i18n::MESSAGES
                    .reason("invalid_body")
                    .arg("reason", "it could not be read"),
            )
        })?;
    let first = Request::from_parts(parts, Body::from(bytes.clone()));
    let again = copy(&first, bytes);
    Ok((first, Some(again)))
}

/// Whether the API refused the request's token as invalid (401 with
/// `error="invalid_token"`), not for a missing scope or anything else.
fn refused_as_invalid(response: &Response) -> bool {
    response.status() == StatusCode::UNAUTHORIZED
        && response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("error=\"invalid_token\""))
}

/// `/bff/<api|admin>/<path>?<query>` → `/<api|admin>/<path>?<query>`, with
/// the browser's cookies and CSRF header removed and the access token
/// attached.
fn to_api_request(request: Request, access_token: &str) -> Result<Request, Error> {
    let (mut parts, body): (_, Body) = request.into_parts();
    // The API router is an app of its own, not nested under `/bff/api`:
    // left in place, the outer route's template would prefix the API's
    // (`/bff/api/{*path}/api/v1/…`), and the idempotency layer, which finds
    // the operation by its template, would not recognise it.
    parts.extensions.remove::<MatchedPath>();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("", |pq| pq.as_str())
        .strip_prefix("/bff")
        .ok_or(Error::NotFound)?
        .to_owned();
    parts.uri = Uri::try_from(path_and_query).map_err(|_| Error::NotFound)?;
    let headers = &mut parts.headers;
    headers.remove(header::COOKIE);
    headers.remove(CSRF_HEADER);
    headers.remove(header::AUTHORIZATION);
    let bearer = HeaderValue::from_str(&format!("Bearer {access_token}"))
        .map_err(|_| Error::internal("the access token is not a valid header value"))?;
    headers.insert(header::AUTHORIZATION, bearer);
    Ok(Request::from_parts(parts, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_to_accepts_same_origin_paths_only() {
        assert_eq!(return_to(None).as_deref(), Some("/"));
        for ok in [
            "/",
            "/inbox",
            "/people/42?thread=7#m-3",
            "/a/b%20c",
            "/bffs",
        ] {
            assert_eq!(return_to(Some(ok)).as_deref(), Some(ok), "{ok}");
        }
        let long = format!("/{}", "a".repeat(MAX_RETURN_TO));
        for bad in [
            "",
            "inbox",
            "//evil.example.com",
            "/\\evil.example.com",
            "https://evil.example.com/",
            "javascript:alert(1)",
            "/a b",
            "/a\"><script>",
            "/a'b",
            "/a\r\nSet-Cookie: x=1",
            "/caf\u{e9}",
            "/bff",
            "/bff/login",
            "/bff?x",
            long.as_str(),
        ] {
            assert_eq!(return_to(Some(bad)), None, "{bad:?}");
        }
    }

    #[test]
    fn landing_page_escapes_and_moves_on() {
        let response = landing_page("/a?b=1&c=2");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            LANDING_CSP
        );
        assert!(response.headers().get(header::LOCATION).is_none());
    }
}
