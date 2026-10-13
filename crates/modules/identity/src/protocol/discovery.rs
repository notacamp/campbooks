//! Issuer metadata (OIDC Discovery 1.0, RFC 8414) and the
//! published signing keys.

use axum::{
    Json,
    extract::State,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use campbooks_kernel::{Operation, Result};

use super::Ctx;
use crate::{deps::Issuer, domain::keys::RECHECK_EVERY};

/// Scopes every first-party client may ask for besides the API's.
pub const OIDC_SCOPES: [&str; 4] = ["openid", "profile", "email", "offline_access"];

fn metadata(issuer: &Issuer) -> serde_json::Value {
    let url = &issuer.url;
    let scopes: Vec<&str> = OIDC_SCOPES
        .iter()
        .copied()
        .chain(issuer.api_scopes.iter().map(String::as_str))
        .collect();
    serde_json::json!({
        "issuer": url,
        "authorization_endpoint": format!("{url}/oauth2/authorize"),
        "token_endpoint": format!("{url}/oauth2/token"),
        "revocation_endpoint": format!("{url}/oauth2/revoke"),
        "end_session_endpoint": format!("{url}/oauth2/end_session"),
        "jwks_uri": format!("{url}/oauth2/jwks"),
        "scopes_supported": scopes,
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "revocation_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce", "sid", "amr",
            "email", "email_verified", "name"],
        "authorization_response_iss_parameter_supported": true,
        "prompt_values_supported": ["none", "login", "consent", "select_account"],
    })
}

/// OIDC Discovery metadata.
#[utoipa::path(
    get,
    path = "/.well-known/openid-configuration",
    tag = "identity",
    operation_id = "identity.openid_configuration",
    extensions(("x-campbooks" = json!(Operation::query()))),
    security(()),
    responses(
        (status = 200, description = "The issuer's metadata.", body = Object),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn openid_configuration(State(ctx): State<Ctx>) -> Result<Json<serde_json::Value>> {
    Ok(Json(metadata(ctx.issuer()?)))
}

/// OAuth 2.0 authorization server metadata (RFC 8414): the same document.
#[utoipa::path(
    get,
    path = "/.well-known/oauth-authorization-server",
    tag = "identity",
    operation_id = "identity.authorization_server_metadata",
    extensions(("x-campbooks" = json!(Operation::query()))),
    security(()),
    responses(
        (status = 200, description = "The issuer's metadata.", body = Object),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn authorization_server_metadata(
    State(ctx): State<Ctx>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(metadata(ctx.issuer()?)))
}

/// The issuer's published signing keys (JWKS): the key that signs now, its
/// successor once published, and recently retired ones.
#[utoipa::path(
    get,
    path = "/oauth2/jwks",
    tag = "identity",
    operation_id = "identity.jwks",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.signing_key"])))),
    security(()),
    responses(
        (status = 200, description = "The JWKS.", body = Object),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn jwks(State(ctx): State<Ctx>) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let document = issuer.keys.jwks(&ctx.state.db).await?;
    let mut response = Json(document).into_response();
    let max_age = format!("public, max-age={}", RECHECK_EVERY.num_seconds() * 60);
    if let Ok(value) = HeaderValue::from_str(&max_age) {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }
    Ok(response)
}
