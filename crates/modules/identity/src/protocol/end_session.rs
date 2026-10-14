//! `/oauth2/end_session` (OIDC RP-initiated logout): end the issuer
//! session an ID token was issued in, and the refresh tokens of that sign-in.
//!
//! A valid `id_token_hint` is required: only the client the person signed in
//! to holds it, so another site cannot sign anyone out. An expired hint is
//! fine (OIDC RP-Initiated Logout §2), but its signature, issuer and client
//! are checked. The web BFF calls it server-side, in-process, with the ID
//! token it keeps; browsers never see one.

use axum::{
    Form,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse as _, Response},
};
use std::time::Duration;

use campbooks_kernel::{
    Error, Operation, Result, Risk,
    error::LocalizableProblem,
    rate_limit::{ClientIp, Limit, RateKey},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::Jwk};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::Ctx;
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::sessions,
    web::{self, SESSION_COOKIE, SameSite, see_other, with_cookie},
};

/// Sign-outs one address may ask for (each verifies a signature). The web
/// BFF's, in-process, have no address and are not counted.
const END_SESSION_LIMIT: Limit = Limit::new("identity.end_session", 30, Duration::from_mins(1));

/// `/oauth2/end_session` parameters (query for GET, form for POST).
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct EndSession {
    /// An ID token the issuer gave the client for this sign-in.
    pub id_token_hint: Option<String>,
    /// The client asking; must be the hint's audience when given.
    pub client_id: Option<String>,
    /// Where to go afterwards; one of the client's registered values.
    pub post_logout_redirect_uri: Option<String>,
    /// Returned unchanged.
    pub state: Option<String>,
}

/// End the session `id_token_hint` names, then return to its client when it
/// registered `post_logout_redirect_uri`. No confirmation page.
#[utoipa::path(
    get,
    path = "/oauth2/end_session",
    tag = "identity",
    operation_id = "identity.end_session",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.session"]).access("identity.session", campbooks_kernel::authz::Action::Delete)))),
    params(EndSession),
    security(()),
    responses(
        (status = 303, description = "Signed out; back to the client.", headers(("location" = String))),
        (status = 204, description = "Signed out; no registered place to return to."),
        (status = 400, description = "`invalid_id_token_hint`: nothing was ended.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Another identity provider serves this instance."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn end_session(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    Query(query): Query<EndSession>,
) -> Result<Response> {
    end(&ctx, ip, query).await
}

/// The same, as a form POST (what the web BFF sends, in-process).
#[utoipa::path(
    post,
    path = "/oauth2/end_session",
    tag = "identity",
    operation_id = "identity.end_session_post",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.session"]).access("identity.session", campbooks_kernel::authz::Action::Delete)))),
    request_body(content = EndSession, content_type = "application/x-www-form-urlencoded"),
    security(()),
    responses(
        (status = 303, description = "Signed out; back to the client.", headers(("location" = String))),
        (status = 204, description = "Signed out; no registered place to return to."),
        (status = 400, description = "`invalid_id_token_hint`: nothing was ended.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Another identity provider serves this instance."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn end_session_post(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    Form(form): Form<EndSession>,
) -> Result<Response> {
    end(&ctx, ip, form).await
}

async fn end(ctx: &Ctx, ip: ClientIp, request: EndSession) -> Result<Response> {
    let issuer = ctx.issuer()?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &END_SESSION_LIMIT, &RateKey::ip(ip))
        .await?;
    let hint = match &request.id_token_hint {
        Some(token) => verify_hint(ctx, issuer, token).await?,
        None => None,
    };
    let Some(hint) = hint.filter(|hint| {
        request
            .client_id
            .as_deref()
            .is_none_or(|client| client == hint.client)
    }) else {
        return Ok(web::no_store(
            LocalizableProblem::new(
                StatusCode::BAD_REQUEST,
                MESSAGES.reason("invalid_id_token_hint"),
            )
            .into_response(),
        ));
    };
    let now = issuer.now();
    if let Some(session) = hint.session {
        let mut tx = ctx.state.db.begin().await?;
        sessions::end_for(&mut tx, session, hint.account, now).await?;
        tx.commit().await?;
    }
    let client = issuer.clients.first_party_client(&hint.client);
    let target = request
        .post_logout_redirect_uri
        .as_deref()
        .filter(|uri| client.is_some_and(|c| c.post_logout_redirect_uris.iter().any(|u| u == uri)));
    let response = match target {
        Some(uri) => {
            let mut url = url::Url::parse(uri).map_err(Error::internal)?;
            if let Some(state) = &request.state {
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

/// What a valid hint says.
struct Hint {
    account: Uuid,
    session: Option<Uuid>,
    client: String,
}

#[derive(Deserialize)]
struct HintClaims {
    sub: String,
    aud: String,
    sid: Option<String>,
}

/// The hint's account, session and client when it is an ID token this
/// issuer signed for one of its clients; `None` otherwise.
async fn verify_hint(ctx: &Ctx, issuer: &Issuer, token: &str) -> Result<Option<Hint>> {
    let Ok(header) = jsonwebtoken::decode_header(token) else {
        return Ok(None);
    };
    if header.alg != Algorithm::EdDSA || header.typ.as_deref() != Some("JWT") {
        return Ok(None);
    }
    let jwks = issuer.keys.jwks(&ctx.state.db).await?;
    let key = jwks["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|jwk| serde_json::from_value::<Jwk>(jwk.clone()).ok())
        .find(|jwk| jwk.common.key_id == header.kid)
        .and_then(|jwk| DecodingKey::from_jwk(&jwk).ok());
    let Some(key) = key else {
        return Ok(None);
    };
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[&issuer.url]);
    validation.validate_aud = false;
    // An expired hint still names the sign-in to end.
    validation.validate_exp = false;
    validation.set_required_spec_claims(&["iss", "sub", "aud"]);
    let Ok(data) = jsonwebtoken::decode::<HintClaims>(token, &key, &validation) else {
        return Ok(None);
    };
    let claims = data.claims;
    let Ok(account) = claims.sub.parse() else {
        return Ok(None);
    };
    if issuer.clients.first_party_client(&claims.aud).is_none() {
        return Ok(None);
    }
    Ok(Some(Hint {
        account,
        session: claims.sid.and_then(|sid| sid.parse().ok()),
        client: claims.aud,
    }))
}
