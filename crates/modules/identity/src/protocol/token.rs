//! `/oauth2/token` (authorization_code, refresh_token) and `/oauth2/revoke`
//! (RFC 7009).
//!
//! Confidential clients authenticate with `client_secret_basic` or
//! `client_secret_post`; public clients send only `client_id` and rely on
//! PKCE. Responses are `no-store`; errors are RFC 6749 §5.2 JSON. Nothing a
//! client sends is logged.

use axum::{
    Form, Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{Operation, Result, Risk};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Ctx;
use crate::{
    deps::Issuer,
    domain::{
        accounts,
        clients::Client,
        tokens::{self, Grant, Mint, OFFLINE_ACCESS, Refusal, TOKEN_LIFETIME},
    },
    events::RefreshReuseDetected,
    web::{no_store, oauth_error},
};

/// A token request (`application/x-www-form-urlencoded`).
#[derive(Deserialize, ToSchema)]
pub struct TokenForm {
    /// `authorization_code` or `refresh_token`.
    pub grant_type: Option<String>,
    /// The authorization code.
    pub code: Option<String>,
    /// The redirect URI the code was issued for.
    pub redirect_uri: Option<String>,
    /// The PKCE verifier.
    pub code_verifier: Option<String>,
    /// The refresh token.
    pub refresh_token: Option<String>,
    /// The client (`client_secret_post`, or a public client).
    pub client_id: Option<String>,
    /// The client's secret (`client_secret_post`).
    pub client_secret: Option<String>,
}

/// A revocation request (`application/x-www-form-urlencoded`).
#[derive(Deserialize, ToSchema)]
pub struct RevokeForm {
    /// The token to revoke.
    pub token: Option<String>,
    /// `refresh_token` or `access_token`; a hint only.
    pub token_type_hint: Option<String>,
    /// The client (`client_secret_post`, or a public client).
    pub client_id: Option<String>,
    /// The client's secret (`client_secret_post`).
    pub client_secret: Option<String>,
}

/// A successful token response (RFC 6749 §5.1).
#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    /// The access token, a JWT for the API.
    pub access_token: String,
    /// Always `Bearer`.
    pub token_type: &'static str,
    /// Seconds until the access token expires.
    pub expires_in: i64,
    /// A new refresh token (rotated on every use).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// The ID token (when `openid` was granted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    /// The granted scopes.
    pub scope: String,
}

/// The client a request authenticates as: HTTP Basic (RFC 6749 §2.3.1,
/// form-urlencoded id and secret) or the form fields; never both.
fn authenticate<'a>(
    issuer: &'a Issuer,
    headers: &HeaderMap,
    client_id: Option<&str>,
    client_secret: Option<&str>,
) -> Option<&'a Client> {
    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Basic "))
        .and_then(|encoded| STANDARD.decode(encoded.trim()).ok())
        .and_then(|decoded| String::from_utf8(decoded).ok());
    let (id, secret) = match (basic, client_secret) {
        (Some(_), Some(_)) => return None,
        (Some(pair), None) => {
            let (id, secret) = pair.split_once(':')?;
            let decode = |s: &str| {
                url::form_urlencoded::parse(format!("v={s}").as_bytes())
                    .next()
                    .map(|(_, v)| v.into_owned())
            };
            let id = decode(id)?;
            if client_id.is_some_and(|form_id| form_id != id) {
                return None;
            }
            (id, Some(decode(secret)?))
        }
        (None, secret) => (client_id?.to_owned(), secret.map(ToOwned::to_owned)),
    };
    let client = issuer.clients.get(&id)?;
    client.authenticates(secret.as_deref()).then_some(client)
}

/// Exchange an authorization code or a refresh token for tokens.
#[utoipa::path(
    post,
    path = "/oauth2/token",
    tag = "identity",
    operation_id = "identity.token",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.authorization_code", "identity.refresh_token"])))),
    request_body(content = TokenForm, content_type = "application/x-www-form-urlencoded"),
    security(()),
    responses(
        (status = 200, description = "Tokens.", body = TokenResponse),
        (status = 400, description = "`invalid_request`, `invalid_grant` or `unsupported_grant_type`."),
        (status = 401, description = "`invalid_client`."),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn token(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let Some(client) = authenticate(
        issuer,
        &headers,
        form.client_id.as_deref(),
        form.client_secret.as_deref(),
    ) else {
        return Ok(oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"));
    };
    let now = issuer.now();
    let mut tx = ctx.state.db.begin().await?;
    let (grant, refresh_token, nonce) = match form.grant_type.as_deref() {
        Some("authorization_code") => {
            let (Some(code), Some(redirect_uri), Some(verifier)) =
                (&form.code, &form.redirect_uri, &form.code_verifier)
            else {
                return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
            };
            match tokens::redeem_code(&mut tx, code, client.id, redirect_uri, verifier, now).await? {
                Ok(redeemed) => {
                    let refresh = if has_scope(&redeemed.grant, OFFLINE_ACCESS) {
                        let (family, token) =
                            tokens::create_family(&mut tx, &redeemed.grant, client, now).await?;
                        tokens::attach_family(&mut tx, code, family).await?;
                        Some(token)
                    } else {
                        None
                    };
                    (redeemed.grant, refresh, redeemed.nonce)
                }
                Err(refusal) => return refused(&ctx, tx, client, refusal).await,
            }
        }
        Some("refresh_token") => {
            let Some(token) = &form.refresh_token else {
                return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
            };
            match tokens::rotate(&mut tx, token, client, now).await? {
                Ok((grant, next)) => (grant, Some(next), None),
                Err(refusal) => return refused(&ctx, tx, client, refusal).await,
            }
        }
        Some(_) => return Ok(oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type")),
        None => return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request")),
    };
    let Some(profile) = accounts::profile(&ctx.state.db, grant.account_id).await? else {
        return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"));
    };
    tx.commit().await?;
    let signer = issuer.keys.signer(&ctx.state.db).await?;
    let mint = Mint {
        issuer: &issuer.url,
        audience: &issuer.audience,
        signer: &signer,
        grant: &grant,
        profile: &profile,
        now,
    };
    let id_token = if has_scope(&grant, "openid") {
        Some(mint.id_token(nonce.as_deref())?)
    } else {
        None
    };
    let body = TokenResponse {
        access_token: mint.access_token()?,
        token_type: "Bearer",
        expires_in: TOKEN_LIFETIME.num_seconds(),
        refresh_token,
        id_token,
        scope: grant.scope.clone(),
    };
    Ok(no_store(Json(body).into_response()))
}

fn has_scope(grant: &Grant, scope: &str) -> bool {
    grant.scope.split_whitespace().any(|s| s == scope)
}

/// Commit what a refusal changed (a revoked family on reuse), record reuse,
/// and answer `invalid_grant`.
async fn refused(
    ctx: &Ctx,
    mut tx: sqlx::Transaction<'static, sqlx::Postgres>,
    client: &Client,
    refusal: Refusal,
) -> Result<Response> {
    if let Refusal::Reused { account_id } = refusal {
        let now = ctx.issuer()?.now();
        let event = RefreshReuseDetected {
            account: account_id,
            client: client.id.to_owned(),
        };
        accounts::audit(
            &mut tx,
            Some(account_id),
            "refresh_reuse_detected",
            serde_json::json!({ "client": client.id }),
            now,
        )
        .await?;
        ctx.state.events.publish(&mut tx, &event).await?;
        tracing::warn!(client = client.id, "a spent code or rotated refresh token was used again; its tokens are revoked");
    }
    tx.commit().await?;
    Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"))
}

/// Revoke a refresh token and every token rotated from the same sign-in.
/// Access tokens are not tracked: they live ten minutes. Unknown tokens
/// answer 200 too (RFC 7009 §2.2).
#[utoipa::path(
    post,
    path = "/oauth2/revoke",
    tag = "identity",
    operation_id = "identity.revoke",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.refresh_token"])))),
    request_body(content = RevokeForm, content_type = "application/x-www-form-urlencoded"),
    security(()),
    responses(
        (status = 200, description = "Revoked, or nothing to revoke."),
        (status = 401, description = "`invalid_client`."),
        (status = 404, description = "Another identity provider serves this instance."),
    ),
)]
pub async fn revoke(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Form(form): Form<RevokeForm>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let Some(client) = authenticate(
        issuer,
        &headers,
        form.client_id.as_deref(),
        form.client_secret.as_deref(),
    ) else {
        return Ok(oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"));
    };
    let Some(token) = &form.token else {
        return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
    };
    let mut conn = ctx.state.db.acquire().await?;
    tokens::revoke_token(&mut conn, token, client.id, issuer.now()).await?;
    Ok(no_store(StatusCode::OK.into_response()))
}
