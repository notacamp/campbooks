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
    Operation, Result,
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
        clients::Client,
        sessions::{self, Session},
        tokens::{self, Grant},
    },
    web::{self, see_other},
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
    let Some(client) = query
        .client_id
        .as_deref()
        .and_then(|id| issuer.clients.get(id))
    else {
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
    let refuse = |request: &Request, error: &str| {
        see_other(&request.redirect(&issuer.url, &[("error", error)]))
    };
    if query.response_type.as_deref() != Some("code") {
        return Ok(refuse(&request, "unsupported_response_type"));
    }
    match (
        &query.code_challenge,
        query.code_challenge_method.as_deref(),
    ) {
        (Some(challenge), Some("S256")) if is_s256_challenge(challenge) => {
            challenge.clone_into(&mut request.code_challenge);
        }
        _ => return Ok(refuse(&request, "invalid_request")),
    }
    let Some(scope) = allowed_scope(&issuer.api_scopes, client, query.scope.as_deref()) else {
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

/// The requested scopes when every one is known (the OIDC scopes and the
/// API's catalogue), deduplicated, without those `client` may not get
/// (`admin:access` unless it is first-party, ADR 0015); `None` when one is
/// unknown. Missing means `openid`.
fn allowed_scope(
    api_scopes: &[String],
    client: &Client,
    requested: Option<&str>,
) -> Option<String> {
    let requested = requested.unwrap_or("openid");
    let mut scopes: Vec<&str> = Vec::new();
    for scope in requested.split_whitespace() {
        let known = OIDC_SCOPES.contains(&scope) || api_scopes.iter().any(|s| s == scope);
        if !known {
            return None;
        }
        if client.may_request(scope) && !scopes.contains(&scope) {
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
    fn only_first_party_clients_are_granted_admin_access() {
        use secrecy::SecretString;

        use crate::domain::clients::{Authentication, Clients, WEB_CLIENT_ID};

        let api = ["admin:access".to_owned(), "conversations:read".to_owned()];
        let clients = Clients::first_party("https://app.example.com", SecretString::from("s"));
        let web = clients.get(WEB_CLIENT_ID).unwrap();
        let requested = Some("openid admin:access conversations:read admin:access");
        assert_eq!(
            allowed_scope(&api, web, requested).as_deref(),
            Some("openid admin:access conversations:read")
        );
        let third_party = Client {
            id: "acme-app",
            first_party: false,
            active: true,
            authentication: Authentication::None,
            redirect_uris: Vec::new(),
            loopback_any_port: false,
            post_logout_redirect_uris: Vec::new(),
            refresh_idle: TimeDelta::days(1),
            refresh_absolute: TimeDelta::days(1),
        };
        assert_eq!(
            allowed_scope(&api, &third_party, requested).as_deref(),
            Some("openid conversations:read"),
            "dropped, not refused: the client still signs the person in"
        );
        assert_eq!(
            allowed_scope(&api, &third_party, Some("admin:access")),
            None,
            "nothing left"
        );
        assert_eq!(
            allowed_scope(&api, web, Some("openid mail:everything")),
            None
        );
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
