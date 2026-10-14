//! `/oauth2/authorize` (authorization code with PKCE S256 only).
//!
//! Validation order follows RFC 6749 §4.1.2.1: an unknown client or a
//! redirect URI it did not register is shown as an error and never
//! redirected to; every other problem goes back to the client's redirect
//! URI with `error`, `state` and `iss` (RFC 9207).
//!
//! Once the person is signed in, [`decide`] says what happens:
//!
//! - the web app gets a code at once;
//! - another client whose redirect is an exact `https` URI gets one at once
//!   when a remembered consent (for that same redirect, less than 90 days
//!   old) already covers the request;
//! - otherwise the person answers the consent screen (`/consent`), after a
//!   sign-in in the last ten minutes. A loopback or private-use-scheme
//!   redirect (native apps, many agents) is asked every time: any app on the
//!   device can claim one (RFC 8252 §8.6).
//!
//! A stored request belongs to the browser that made it (the
//! `__Host-campbooks_browser` cookie): another browser handed its id (a
//! link someone was sent) finds nothing, so nobody can be made to finish
//! someone else's sign-in.

use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::{
    Operation, Result,
    crypto::{hash, random_token},
    error::LocalizableProblem,
    rate_limit::{ClientIp, Limit, RateKey},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;
use uuid::Uuid;

use super::{Ctx, SIGN_IN_PATH};
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::{
        clients::{Client, is_https_redirect},
        consents,
        sessions::{self, Session},
        step_up::STEP_UP_WINDOW,
        tokens::{self, Grant},
    },
    metadata,
    web::{self, see_other},
};

/// How long an authorization request waits for the person to sign in.
pub const REQUEST_LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// How long a browser keeps the token its authorization requests are bound
/// to (renewed by each request): longer than a request waits, so the one a
/// slow sign-in stores again still finds it.
const BROWSER_LIFETIME: TimeDelta = TimeDelta::days(1);

/// Authorization requests one address may make (each may store a request).
const AUTHORIZE_LIMIT: Limit = Limit::new("identity.authorize", 60, Duration::from_mins(1));

/// The web client's consent screen.
pub const CONSENT_PATH: &str = "/consent";

/// `/oauth2/authorize` parameters.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuthorizeQuery {
    /// Must be `code`.
    pub response_type: Option<String>,
    /// The client: a registered id, or the `https` URL of its metadata
    /// document.
    pub client_id: Option<String>,
    /// One of the client's registered redirect URIs.
    pub redirect_uri: Option<String>,
    /// Space-separated scopes.
    pub scope: Option<String>,
    /// Returned unchanged to the client.
    pub state: Option<String>,
    /// Echoed in the ID token.
    pub nonce: Option<String>,
    /// The PKCE challenge.
    pub code_challenge: Option<String>,
    /// Must be `S256`.
    pub code_challenge_method: Option<String>,
    /// `none`, `login`, `consent` or `select_account`.
    pub prompt: Option<String>,
    /// Seconds since the person last authenticated, at most.
    pub max_age: Option<i64>,
    /// The resource the token is for (RFC 8707): the API's resource URI.
    pub resource: Option<String>,
}

/// A parsed, valid authorization request.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Request {
    /// The client.
    pub client_id: String,
    /// Where the code goes.
    pub redirect_uri: String,
    /// Granted scopes.
    pub scope: String,
    /// The client's `state`.
    pub state: Option<String>,
    /// The client's `nonce`.
    pub nonce: Option<String>,
    /// The PKCE challenge.
    pub code_challenge: String,
    /// The resource (`aud`).
    pub resource: Option<String>,
    /// The consent screen must be shown.
    pub consent_due: bool,
    /// The person a request waiting on the consent screen was put to.
    pub consent_account_id: Option<Uuid>,
}

impl Request {
    /// The client's redirect URI with `params`, plus `state` and `iss`.
    pub fn redirect(&self, issuer: &str, params: &[(&str, &str)]) -> String {
        // Registered redirect URIs are valid URLs; the private-use scheme
        // one too.
        let Ok(mut url) = url::Url::parse(&self.redirect_uri) else {
            return self.redirect_uri.clone();
        };
        {
            let mut query = url.query_pairs_mut();
            for (name, value) in params {
                query.append_pair(name, value);
            }
            if let Some(state) = &self.state {
                query.append_pair("state", state);
            }
            query.append_pair("iss", issuer);
        }
        url.into()
    }
}

fn problem(status: StatusCode, code: &'static str) -> Response {
    web::no_store(LocalizableProblem::new(status, MESSAGES.reason(code)).into_response())
}

/// This browser's binding token (`__Host-campbooks_browser`), when it
/// sends a well-formed one.
pub fn browser(issuer: &Issuer, headers: &HeaderMap) -> Option<String> {
    issuer
        .cookies
        .read(headers, web::BROWSER_COOKIE)
        .filter(|cookie| {
            cookie.len() == 43
                && cookie
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// Metadata documents one address may make the server look up (each may
/// be a fetch from the internet).
const DOCUMENT_LIMIT: Limit = Limit::new("identity.metadata_document", 30, Duration::from_mins(1));

/// The client `id` names: Campbooks' own, a stored one, or the client a
/// metadata document at that URL describes (fetched now if not cached; the
/// asker's address is limited, so the server can't be made to fetch
/// without end).
pub async fn find_client(
    ctx: &Ctx,
    issuer: &Issuer,
    id: &str,
    ip: ClientIp,
) -> Result<Option<Client>> {
    if let Some(url) = metadata::document_url(id).filter(|_| issuer.clients.metadata_documents()) {
        ctx.state
            .rate_limits
            .check(&ctx.state.db, &DOCUMENT_LIMIT, &RateKey::ip(ip))
            .await?;
        return metadata::client(
            &ctx.state.db,
            &ctx.state.rate_limits,
            issuer.fetcher.as_ref(),
            &issuer.document_failures,
            &url,
            &issuer.api_scopes,
            issuer.now(),
        )
        .await;
    }
    issuer.clients.find(&ctx.state.db, id).await
}

/// Start (or, for a signed-in browser, complete at once) an authorization.
#[utoipa::path(
    get,
    path = "/oauth2/authorize",
    tag = "identity",
    operation_id = "identity.authorize",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.authorize_request", "identity.authorization_code"]).access("identity.sign_in", campbooks_kernel::authz::Action::Write)))),
    params(AuthorizeQuery),
    security(()),
    responses(
        (status = 303, description = "To the client's redirect URI with `code` (signed in, and \
            no consent to ask) or `error`; to the sign-in screen; or to the consent screen.",
            headers(("location" = String))),
        (status = 400, description = "`invalid_client` or `invalid_redirect_uri`: nothing is \
            redirected.", body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Another identity provider serves this instance."),
        (status = 429, description = "`rate_limited`: too many authorization requests, or \
            metadata-document clients looked up, from this address.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    ),
)]
pub async fn authorize(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &AUTHORIZE_LIMIT, &RateKey::ip(ip))
        .await?;
    let client = match query.client_id.as_deref() {
        Some(id) => find_client(&ctx, issuer, id, ip).await?,
        None => None,
    };
    let Some(client) = client else {
        return Ok(problem(StatusCode::BAD_REQUEST, "invalid_client"));
    };
    let Some(redirect_uri) = query
        .redirect_uri
        .as_deref()
        .filter(|uri| client.accepts_redirect(uri))
    else {
        return Ok(problem(StatusCode::BAD_REQUEST, "invalid_redirect_uri"));
    };
    let refuse = |request: &Request, error: &str| {
        see_other(&request.redirect(&issuer.url, &[("error", error)]))
    };
    let (request, error) = checked(issuer, &client, redirect_uri, &query);
    if let Some(error) = error {
        return Ok(refuse(&request, error));
    }
    let prompt = query.prompt.as_deref().unwrap_or_default();
    let force_login = prompt.split_whitespace().any(|p| p == "login");
    let silent = prompt.split_whitespace().any(|p| p == "none");
    let now = issuer.now();
    let browser = browser(issuer, &headers).unwrap_or_else(random_token);
    let bound = |response: Response| {
        web::with_cookie(
            response,
            issuer.cookies.set(
                web::BROWSER_COOKIE,
                &browser,
                BROWSER_LIFETIME.num_seconds(),
                web::SameSite::Lax,
            ),
        )
    };
    let session = match issuer.session_cookie(&headers) {
        Some(token) => sessions::current(&ctx.state.db, &token, now).await?,
        None => None,
    };
    let fresh_enough = |session: &Session| {
        query
            .max_age
            .is_none_or(|max_age| now - session.auth_time <= TimeDelta::seconds(max_age.max(0)))
    };
    if let Some(session) = session.filter(|s| !force_login && fresh_enough(s)) {
        if silent {
            // `prompt=none`: a code, or an error; never a screen.
            return Ok(
                match decision(&ctx.state.db, &client, &request, &session, now).await? {
                    Decision::Code(workspace) => see_other(
                        &issue(&ctx.state.db, issuer, &request, &session, workspace, now).await?,
                    ),
                    Decision::SignInAgain => refuse(&request, "login_required"),
                    Decision::Consent => refuse(&request, "consent_required"),
                },
            );
        }
        let location = decide(
            &ctx.state.db,
            issuer,
            &client,
            request,
            &session,
            &browser,
            now,
        )
        .await?;
        return Ok(bound(see_other(&location)));
    }
    if silent {
        return Ok(refuse(&request, "login_required"));
    }
    let id = store(&ctx.state.db, &request, &browser, now).await?;
    Ok(bound(see_other(&format!("{SIGN_IN_PATH}?request={id}"))))
}

/// The request `query` makes of `client`, redirecting to `redirect_uri`
/// (already checked), and what is wrong with it if anything (the error
/// sent back to the client).
fn checked(
    issuer: &Issuer,
    client: &Client,
    redirect_uri: &str,
    query: &AuthorizeQuery,
) -> (Request, Option<&'static str>) {
    let prompt = query.prompt.as_deref().unwrap_or_default();
    let mut request = Request {
        client_id: client.id.clone(),
        redirect_uri: redirect_uri.to_owned(),
        scope: String::new(),
        state: query.state.clone(),
        nonce: query.nonce.clone(),
        code_challenge: String::new(),
        resource: None,
        consent_due: prompt.split_whitespace().any(|p| p == "consent"),
        consent_account_id: None,
    };
    if query.response_type.as_deref() != Some("code") || !client.may_use("authorization_code") {
        return (request, Some("unsupported_response_type"));
    }
    match (
        &query.code_challenge,
        query.code_challenge_method.as_deref(),
    ) {
        (Some(challenge), Some("S256")) if is_s256_challenge(challenge) => {
            challenge.clone_into(&mut request.code_challenge);
        }
        _ => return (request, Some("invalid_request")),
    }
    let Some(scope) = allowed_scope(issuer, client, query.scope.as_deref()) else {
        return (request, Some("invalid_scope"));
    };
    request.scope = scope;
    match query.resource.as_deref() {
        None => {}
        Some(resource) if resource == issuer.resource => {
            request.resource = Some(resource.to_owned());
        }
        Some(_) => return (request, Some("invalid_target")),
    }
    if !prompt
        .split_whitespace()
        .all(|p| matches!(p, "none" | "login" | "consent" | "select_account"))
    {
        return (request, Some("invalid_request"));
    }
    (request, None)
}

/// The requested scopes when `client` may ask for every one (the OIDC
/// scopes and, for Campbooks' own clients, the API's catalogue; for anyone
/// else what they registered, never `identity:*`), deduplicated, without
/// `admin:access` unless `client` is first-party (ADR 0015); `None`
/// otherwise. Missing means `openid`.
fn allowed_scope(issuer: &Issuer, client: &Client, requested: Option<&str>) -> Option<String> {
    let requested = requested.unwrap_or("openid");
    let mut scopes: Vec<&str> = Vec::new();
    for scope in requested.split_whitespace() {
        // `admin:access` is dropped, not refused, for anyone but Campbooks'
        // own apps (ADR 0015): the app gets what it may have.
        if scope == campbooks_kernel::admin::ACCESS_SCOPE && !client.is_first_party() {
            continue;
        }
        if !client.may_ask_for(scope, &issuer.api_scopes) {
            return None;
        }
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }
    (!scopes.is_empty()).then(|| scopes.join(" "))
}

/// An S256 code challenge: 43 characters of base64url.
fn is_s256_challenge(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Store `request` for `browser` (its binding token); its id.
async fn store(
    db: &PgPool,
    request: &Request,
    browser: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    let id = random_token();
    sqlx::query(
        "INSERT INTO identity.authorize_requests
             (id_hash, client_id, redirect_uri, scope, state, nonce, code_challenge, created_at,
              expires_at, resource, consent_due, consent_account_id, browser_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(hash(&id))
    .bind(&request.client_id)
    .bind(&request.redirect_uri)
    .bind(&request.scope)
    .bind(&request.state)
    .bind(&request.nonce)
    .bind(&request.code_challenge)
    .bind(now)
    .bind(now + REQUEST_LIFETIME)
    .bind(&request.resource)
    .bind(request.consent_due)
    .bind(request.consent_account_id)
    .bind(hash(browser))
    .execute(db)
    .await?;
    Ok(id)
}

const REQUEST_COLUMNS: &str = "client_id, redirect_uri, scope, state, nonce, code_challenge, \
     resource, consent_due, consent_account_id";

/// The stored authorization request `id`, while it waits, when `browser`
/// (the asker's binding token) is the one that made it.
pub async fn load(
    db: &PgPool,
    id: &str,
    browser: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<Request>> {
    let Some(browser) = browser else {
        return Ok(None);
    };
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {REQUEST_COLUMNS}
           FROM identity.authorize_requests
          WHERE id_hash = $1 AND expires_at > $2 AND browser_hash = $3"
    )))
    .bind(hash(id))
    .bind(now)
    .bind(hash(browser))
    .fetch_optional(db)
    .await?)
}

/// Take the stored request `id` (single use), while it waits, when
/// `browser` is the one that made it.
pub async fn take(
    conn: &mut sqlx::PgConnection,
    id: &str,
    browser: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<Request>> {
    let Some(browser) = browser else {
        return Ok(None);
    };
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "DELETE FROM identity.authorize_requests
          WHERE id_hash = $1 AND expires_at > $2 AND browser_hash = $3
         RETURNING {REQUEST_COLUMNS}"
    )))
    .bind(hash(id))
    .bind(now)
    .bind(hash(browser))
    .fetch_optional(conn)
    .await?)
}

/// Complete the stored request `id` for `session` in `browser`; where to
/// send the browser (the client with a code, or the consent screen), or
/// `None` when the request is gone, expired, or another browser's.
pub async fn complete_stored(
    db: &PgPool,
    issuer: &Issuer,
    id: &str,
    browser: Option<&str>,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let mut conn = db.acquire().await?;
    let Some(request) = take(&mut conn, id, browser, now).await? else {
        return Ok(None);
    };
    let Some(browser) = browser else {
        return Ok(None);
    };
    drop(conn);
    let Some(client) = issuer.clients.find(db, &request.client_id).await? else {
        // Removed while the person signed in.
        return Ok(Some(
            request.redirect(&issuer.url, &[("error", "unauthorized_client")]),
        ));
    };
    decide(db, issuer, &client, request, session, browser, now)
        .await
        .map(Some)
}

/// What becomes of a signed-in authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    /// A code, bound to this workspace (if any).
    Code(Option<Uuid>),
    /// The consent screen, after a fresh sign-in.
    SignInAgain,
    /// The consent screen.
    Consent,
}

/// See the module docs.
async fn decision(
    db: &PgPool,
    client: &Client,
    request: &Request,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<Decision> {
    if !client.needs_consent() {
        return Ok(Decision::Code(None));
    }
    if !request.consent_due && is_https_redirect(&request.redirect_uri) {
        let remembered = consents::live(db, session.account_id, &client.id)
            .await?
            .filter(|consent| {
                consent.remembered
                    && consent.remember_until.is_some_and(|until| until > now)
                    && consent.redirect_uri == request.redirect_uri
                    && (consent.bound_workspace_id.is_some() || !client.binds_workspace())
                    && request
                        .scope
                        .split_whitespace()
                        .all(|s| consent.scopes.iter().any(|c| c == s))
            });
        if let Some(consent) = remembered {
            consents::touch(db, consent.id, now).await?;
            return Ok(Decision::Code(consent.bound_workspace_id));
        }
    }
    if now - session.auth_time > STEP_UP_WINDOW {
        return Ok(Decision::SignInAgain);
    }
    Ok(Decision::Consent)
}

/// Where the browser goes for `request`, signed in with `session`: the
/// client with a code, the sign-in screen (to sign in again before the
/// consent screen), or the consent screen.
async fn decide(
    db: &PgPool,
    issuer: &Issuer,
    client: &Client,
    mut request: Request,
    session: &Session,
    browser: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    match decision(db, client, &request, session, now).await? {
        Decision::Code(workspace) => issue(db, issuer, &request, session, workspace, now).await,
        Decision::SignInAgain => {
            // The screen will follow the new sign-in.
            request.consent_due = true;
            let id = store(db, &request, browser, now).await?;
            Ok(format!("{SIGN_IN_PATH}?request={id}&reauth=1"))
        }
        Decision::Consent => {
            request.consent_due = true;
            request.consent_account_id = Some(session.account_id);
            let id = store(db, &request, browser, now).await?;
            Ok(format!("{CONSENT_PATH}?request={id}"))
        }
    }
}

/// Issue a code for `request` to the person signed in with `session`,
/// bound to `workspace`, and return the client's redirect URI carrying it.
pub async fn issue(
    db: &PgPool,
    issuer: &Issuer,
    request: &Request,
    session: &Session,
    workspace: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<String> {
    let grant = Grant {
        client_id: request.client_id.clone(),
        account_id: session.account_id,
        session_id: Some(session.id),
        scope: request.scope.clone(),
        auth_time: session.auth_time,
        resource: request
            .resource
            .clone()
            .unwrap_or_else(|| issuer.resource.clone()),
        workspace,
    };
    let mut conn = db.acquire().await?;
    let code = tokens::create_code(
        &mut conn,
        &grant,
        &request.redirect_uri,
        &request.code_challenge,
        request.nonce.as_deref(),
        now,
    )
    .await?;
    Ok(request.redirect(&issuer.url, &[("code", &code)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_challenges_are_43_base64url_characters() {
        assert!(is_s256_challenge(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        ));
        assert!(!is_s256_challenge("short"));
        assert!(!is_s256_challenge(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM"
        ));
    }

    #[test]
    fn redirects_carry_state_and_issuer() {
        let request = Request {
            client_id: "c".into(),
            redirect_uri: "https://app.example.com/bff/callback?x=1".into(),
            scope: "openid".into(),
            state: Some("s t".into()),
            nonce: None,
            code_challenge: String::new(),
            resource: None,
            consent_due: false,
            consent_account_id: None,
        };
        assert_eq!(
            request.redirect("https://app.example.com", &[("code", "abc")]),
            "https://app.example.com/bff/callback?x=1&code=abc&state=s+t&iss=https%3A%2F%2Fapp.example.com"
        );
    }
}
