//! `/oauth2/authorize` (authorization code with PKCE S256 only) and
//! `/oauth2/end_session` (RP-initiated logout).
//!
//! Validation order follows RFC 6749 §4.1.2.1: an unknown client or a
//! redirect URI it did not register is shown as an error and never
//! redirected to; every other problem goes back to the client's redirect
//! URI with `error`, `state` and `iss` (RFC 9207).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::{
    Error, Operation, Result,
    crypto::{hash, random_token},
    error::LocalizableProblem,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;

use super::{Ctx, SIGN_IN_PATH, discovery::OIDC_SCOPES};
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::{
        sessions::{self, Session},
        tokens::{self, Grant},
    },
    web::{self, SESSION_COOKIE, SameSite, see_other, with_cookie},
};

/// How long an authorization request waits for the person to sign in.
pub const REQUEST_LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// `/oauth2/authorize` parameters.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuthorizeQuery {
    /// Must be `code`.
    pub response_type: Option<String>,
    /// The client.
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
}

impl Request {
    /// The client's redirect URI with `params`, plus `state` and `iss`.
    pub fn redirect(&self, issuer: &str, params: &[(&str, &str)]) -> String {
        let mut url = match url::Url::parse(&self.redirect_uri) {
            Ok(url) => url,
            // Registered redirect URIs are valid URLs; the private-use
            // scheme one too.
            Err(_) => return self.redirect_uri.clone(),
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

/// Start (or, for a signed-in browser, complete at once) an authorization.
#[utoipa::path(
    get,
    path = "/oauth2/authorize",
    tag = "identity",
    operation_id = "identity.authorize",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.authorize_request", "identity.authorization_code"])))),
    params(AuthorizeQuery),
    security(()),
    responses(
        (status = 303, description = "To the client's redirect URI with `code` (signed in) or \
            `error`, or to the sign-in screen.", headers(("location" = String))),
        (status = 400, description = "`invalid_client` or `invalid_redirect_uri`: nothing is \
            redirected.", body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn authorize(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let Some(client) = query.client_id.as_deref().and_then(|id| issuer.clients.get(id)) else {
        return Ok(problem(StatusCode::BAD_REQUEST, "invalid_client"));
    };
    let Some(redirect_uri) = query
        .redirect_uri
        .as_deref()
        .filter(|uri| client.accepts_redirect(uri))
    else {
        return Ok(problem(StatusCode::BAD_REQUEST, "invalid_redirect_uri"));
    };
    let mut request = Request {
        client_id: client.id.to_owned(),
        redirect_uri: redirect_uri.to_owned(),
        scope: String::new(),
        state: query.state.clone(),
        nonce: query.nonce.clone(),
        code_challenge: String::new(),
    };
    let refuse = |request: &Request, error: &str| see_other(&request.redirect(&issuer.url, &[("error", error)]));
    if query.response_type.as_deref() != Some("code") {
        return Ok(refuse(&request, "unsupported_response_type"));
    }
    match (&query.code_challenge, query.code_challenge_method.as_deref()) {
        (Some(challenge), Some("S256")) if is_s256_challenge(challenge) => {
            challenge.clone_into(&mut request.code_challenge);
        }
        _ => return Ok(refuse(&request, "invalid_request")),
    }
    let Some(scope) = allowed_scope(issuer, query.scope.as_deref()) else {
        return Ok(refuse(&request, "invalid_scope"));
    };
    request.scope = scope;
    let prompt = query.prompt.as_deref().unwrap_or_default();
    if !prompt
        .split_whitespace()
        .all(|p| matches!(p, "none" | "login" | "consent" | "select_account"))
    {
        return Ok(refuse(&request, "invalid_request"));
    }
    let force_login = prompt.split_whitespace().any(|p| p == "login");
    let now = issuer.now();
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
        let location = complete(&ctx.state.db, issuer, &request, &session, now).await?;
        return Ok(see_other(&location));
    }
    if prompt.split_whitespace().any(|p| p == "none") {
        return Ok(refuse(&request, "login_required"));
    }
    let id = store(&ctx.state.db, &request, now).await?;
    Ok(see_other(&format!("{SIGN_IN_PATH}?request={id}")))
}

/// The requested scopes when every one is known (the OpenID scopes and the
/// API's catalogue), deduplicated; `None` otherwise. Missing means `openid`.
fn allowed_scope(issuer: &Issuer, requested: Option<&str>) -> Option<String> {
    let requested = requested.unwrap_or("openid");
    let mut scopes: Vec<&str> = Vec::new();
    for scope in requested.split_whitespace() {
        let known = OIDC_SCOPES.contains(&scope) || issuer.api_scopes.iter().any(|s| s == scope);
        if !known {
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

async fn store(db: &PgPool, request: &Request, now: DateTime<Utc>) -> Result<String> {
    let id = random_token();
    sqlx::query(
        "INSERT INTO identity.authorize_requests
             (id_hash, client_id, redirect_uri, scope, state, nonce, code_challenge, created_at,
              expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
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
    .execute(db)
    .await?;
    Ok(id)
}

/// The stored authorization request `id`, while it waits.
pub async fn load(db: &PgPool, id: &str, now: DateTime<Utc>) -> Result<Option<Request>> {
    Ok(sqlx::query_as(
        "SELECT client_id, redirect_uri, scope, state, nonce, code_challenge
           FROM identity.authorize_requests WHERE id_hash = $1 AND expires_at > $2",
    )
    .bind(hash(id))
    .bind(now)
    .fetch_optional(db)
    .await?)
}

/// Complete the stored request `id` for `session`; where to send the
/// browser, or `None` when the request is gone or expired.
pub async fn complete_stored(
    db: &PgPool,
    issuer: &Issuer,
    id: &str,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let request: Option<Request> = sqlx::query_as(
        "DELETE FROM identity.authorize_requests WHERE id_hash = $1 AND expires_at > $2
         RETURNING client_id, redirect_uri, scope, state, nonce, code_challenge",
    )
    .bind(hash(id))
    .bind(now)
    .fetch_optional(db)
    .await?;
    match request {
        Some(request) => complete(db, issuer, &request, session, now).await.map(Some),
        None => Ok(None),
    }
}

/// Issue a code for `request` to the person signed in with `session` and
/// return the client's redirect URI carrying it.
async fn complete(
    db: &PgPool,
    issuer: &Issuer,
    request: &Request,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<String> {
    let grant = Grant {
        client_id: request.client_id.clone(),
        account_id: session.account_id,
        session_id: Some(session.id),
        scope: request.scope.clone(),
        auth_time: session.auth_time,
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

/// `/oauth2/end_session` parameters.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EndSessionQuery {
    /// The client asking.
    pub client_id: Option<String>,
    /// Where to go afterwards; one of the client's registered values.
    pub post_logout_redirect_uri: Option<String>,
    /// Returned unchanged.
    pub state: Option<String>,
}

/// End the browser's issuer session (and the refresh tokens issued in it),
/// then return to the client when it registered `post_logout_redirect_uri`.
/// No confirmation page: the client asked for it, and ending a session is
/// harmless.
#[utoipa::path(
    get,
    path = "/oauth2/end_session",
    tag = "identity",
    operation_id = "identity.end_session",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.session"])))),
    params(EndSessionQuery),
    security(()),
    responses(
        (status = 303, description = "Signed out; back to the client.", headers(("location" = String))),
        (status = 204, description = "Signed out; no registered place to return to."),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn end_session(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Query(query): Query<EndSessionQuery>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let now = issuer.now();
    if let Some(token) = issuer.session_cookie(&headers)
        && let Some(session) = sessions::current(&ctx.state.db, &token, now).await?
    {
        let mut tx = ctx.state.db.begin().await?;
        sessions::end(&mut tx, session.id, now).await?;
        tx.commit().await?;
    }
    let target = query
        .client_id
        .as_deref()
        .and_then(|id| issuer.clients.get(id))
        .and(query.post_logout_redirect_uri.as_deref())
        .filter(|uri| {
            query
                .client_id
                .as_deref()
                .and_then(|id| issuer.clients.get(id))
                .is_some_and(|client| client.post_logout_redirect_uris.iter().any(|u| u == uri))
        });
    let response = match target {
        Some(uri) => {
            let mut url = url::Url::parse(uri).map_err(Error::internal)?;
            if let Some(state) = &query.state {
                url.query_pairs_mut().append_pair("state", state);
            }
            see_other(url.as_str())
        }
        None => web::no_store(StatusCode::NO_CONTENT.into_response()),
    };
    Ok(with_cookie(
        response,
        issuer.cookies.clear(SESSION_COOKIE, SameSite::Lax),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_challenges_are_43_base64url_characters() {
        assert!(is_s256_challenge("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
        assert!(!is_s256_challenge("short"));
        assert!(!is_s256_challenge("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM"));
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
        };
        assert_eq!(
            request.redirect("https://app.example.com", &[("code", "abc")]),
            "https://app.example.com/bff/callback?x=1&code=abc&state=s+t&iss=https%3A%2F%2Fapp.example.com"
        );
    }
}
