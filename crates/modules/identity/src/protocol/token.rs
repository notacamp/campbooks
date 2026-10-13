//! `/oauth2/token` (`authorization_code`, `refresh_token`,
//! `client_credentials`) and `/oauth2/revoke` (RFC 7009).
//!
//! Confidential clients authenticate with `client_secret_basic` or
//! `client_secret_post`; public clients send only `client_id` and rely on
//! PKCE. Only a workspace's own confidential client may use
//! `client_credentials`; its token is bound to its workspace. Every token is
//! for the API's resource (RFC 8707: another `resource` is
//! `invalid_target`). Responses are `no-store`; errors are RFC 6749 §5.2
//! JSON. Nothing a client sends is logged. Requests from the network count
//! against a per-address limit.

use std::time::Duration;

use axum::{
    Form, Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{
    Operation, Result, Risk,
    rate_limit::{ClientIp, Limit, RateKey},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Ctx;
use crate::{
    deps::Issuer,
    domain::{
        accounts,
        clients::{self, Client, Kind},
        sessions,
        tokens::{self, Grant, Mint, OFFLINE_ACCESS, Refusal, TOKEN_LIFETIME},
    },
    events::RefreshReuseDetected,
    web::{no_store, oauth_error},
};

/// A token request (`application/x-www-form-urlencoded`).
#[derive(Deserialize, ToSchema)]
pub struct TokenForm {
    /// `authorization_code`, `refresh_token` or `client_credentials`.
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
    /// Space-separated scopes (`client_credentials`: at most what the
    /// client was given; missing means all of them).
    pub scope: Option<String>,
    /// The resource the token is for (RFC 8707): the API's resource URI.
    pub resource: Option<String>,
}

/// Token requests one address may send.
const TOKEN_LIMIT: Limit = Limit::new("identity.token", 120, Duration::from_mins(1));

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

/// The client id and secret a request presents: HTTP Basic (RFC 6749
/// §2.3.1, form-urlencoded id and secret) or the form fields; never both.
fn credentials(
    headers: &HeaderMap,
    client_id: Option<&str>,
    client_secret: Option<&str>,
) -> Option<(String, Option<String>)> {
    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Basic "))
        .and_then(|encoded| STANDARD.decode(encoded.trim()).ok())
        .and_then(|decoded| String::from_utf8(decoded).ok());
    match (basic, client_secret) {
        (Some(_), Some(_)) => None,
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
            Some((id, Some(decode(secret)?)))
        }
        (None, secret) => Some((client_id?.to_owned(), secret.map(ToOwned::to_owned))),
    }
}

/// The client a request authenticates as.
async fn authenticate(
    db: &sqlx::PgPool,
    issuer: &Issuer,
    headers: &HeaderMap,
    client_id: Option<&str>,
    client_secret: Option<&str>,
) -> Result<Option<Client>> {
    let Some((id, secret)) = credentials(headers, client_id, client_secret) else {
        return Ok(None);
    };
    let Some(client) = issuer.clients.find(db, &id).await? else {
        return Ok(None);
    };
    Ok(client.authenticates(secret.as_deref()).then_some(client))
}

/// Exchange an authorization code, a refresh token or a workspace client's
/// credentials for tokens.
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
        (status = 400, description = "`invalid_request`, `invalid_grant`, `invalid_scope`, \
            `invalid_target`, `unauthorized_client` or `unsupported_grant_type`."),
        (status = 401, description = "`invalid_client`."),
        (status = 404, description = "Another identity provider serves this instance."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn token(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &TOKEN_LIMIT, &RateKey::ip(ip))
        .await?;
    let Some(client) = authenticate(
        &ctx.state.db,
        issuer,
        &headers,
        form.client_id.as_deref(),
        form.client_secret.as_deref(),
    )
    .await?
    else {
        return Ok(oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"));
    };
    if let Some(refused) = refuse_early(issuer, &client, &form) {
        return Ok(refused);
    }
    let now = issuer.now();
    if form.grant_type.as_deref() == Some("client_credentials") {
        return client_credentials(&ctx, issuer, &client, form.scope.as_deref(), now).await;
    }
    let mut tx = ctx.state.db.begin().await?;
    let (grant, refresh_token, nonce) = match form.grant_type.as_deref() {
        Some("authorization_code") => {
            let (Some(code), Some(redirect_uri), Some(verifier)) =
                (&form.code, &form.redirect_uri, &form.code_verifier)
            else {
                return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
            };
            match tokens::redeem_code(
                &mut tx,
                code,
                &client.id,
                redirect_uri,
                verifier,
                &issuer.resource,
                now,
            )
            .await?
            {
                Ok(redeemed) => {
                    let refresh = if has_scope(&redeemed.grant, OFFLINE_ACCESS)
                        && client.may_use("refresh_token")
                    {
                        let (family, token) =
                            tokens::create_family(&mut tx, &redeemed.grant, &client, now).await?;
                        tokens::attach_family(&mut tx, code, family).await?;
                        Some(token)
                    } else {
                        None
                    };
                    (redeemed.grant, refresh, redeemed.nonce)
                }
                Err(refusal) => return refused(&ctx, tx, &client, refusal).await,
            }
        }
        Some("refresh_token") => {
            let Some(token) = &form.refresh_token else {
                return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
            };
            match tokens::rotate(&mut tx, token, &client, &issuer.resource, now).await? {
                Ok((grant, next)) => (grant, Some(next), None),
                Err(refusal) => return refused(&ctx, tx, &client, refusal).await,
            }
        }
        Some(_) => {
            return Ok(oauth_error(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
            ));
        }
        None => return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request")),
    };
    let exchanged = Issued {
        grant,
        refresh_token,
        nonce,
    };
    respond(&ctx, issuer, &client, tx, exchanged, now).await
}

/// What a code or refresh token was exchanged for, before signing.
struct Issued {
    grant: Grant,
    refresh_token: Option<String>,
    nonce: Option<String>,
}

/// Commit `tx` and answer the tokens `exchanged` stands for, with an ID token
/// (carrying its nonce) when `openid` was granted.
async fn respond(
    ctx: &Ctx,
    issuer: &Issuer,
    client: &Client,
    tx: sqlx::Transaction<'static, sqlx::Postgres>,
    exchanged: Issued,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Response> {
    let Issued {
        grant,
        refresh_token,
        nonce,
    } = exchanged;
    let Some(profile) = accounts::profile(&ctx.state.db, grant.account_id).await? else {
        return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"));
    };
    tx.commit().await?;
    if !client.is_first_party() {
        clients::touch(&ctx.state.db, &client.id, now).await?;
    }
    let signer = issuer.keys.signer(&ctx.state.db).await?;
    let amr = sessions::amr(&ctx.state.db, grant.session_id).await?;
    let mint = Mint {
        amr: &amr,
        issuer: &issuer.url,
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

/// `unauthorized_client` for a grant type the client may not use, and
/// `invalid_target` for a resource other than the API's.
fn refuse_early(issuer: &Issuer, client: &Client, form: &TokenForm) -> Option<Response> {
    if let Some(grant_type) = form.grant_type.as_deref()
        && matches!(
            grant_type,
            "authorization_code" | "refresh_token" | "client_credentials"
        )
        && !client.may_use(grant_type)
    {
        return Some(oauth_error(StatusCode::BAD_REQUEST, "unauthorized_client"));
    }
    form.resource
        .as_deref()
        .is_some_and(|resource| resource != issuer.resource)
        .then(|| oauth_error(StatusCode::BAD_REQUEST, "invalid_target"))
}

/// RFC 6749 §4.4: a workspace's own confidential client, acting on its own
/// behalf in its workspace, for at most the scopes it was given. No refresh
/// token, no ID token.
async fn client_credentials(
    ctx: &Ctx,
    issuer: &Issuer,
    client: &Client,
    requested: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Response> {
    let (Kind::Workspace, Some(workspace), true) =
        (client.kind, client.workspace, client.is_confidential())
    else {
        return Ok(oauth_error(StatusCode::BAD_REQUEST, "unauthorized_client"));
    };
    let allowed = client.allowed_scopes.clone().unwrap_or_default();
    let scope: Vec<&str> = match requested {
        None => allowed.iter().map(String::as_str).collect(),
        Some(requested) => {
            let mut scope: Vec<&str> = Vec::new();
            for name in requested.split_whitespace() {
                if !allowed.iter().any(|a| a == name) {
                    return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_scope"));
                }
                if !scope.contains(&name) {
                    scope.push(name);
                }
            }
            scope
        }
    };
    let scope = scope.join(" ");
    clients::touch(&ctx.state.db, &client.id, now).await?;
    let signer = issuer.keys.signer(&ctx.state.db).await?;
    let access_token = tokens::client_access_token(
        &issuer.url,
        &signer,
        &client.id,
        &scope,
        workspace,
        &issuer.resource,
        now,
    )?;
    let body = TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: TOKEN_LIFETIME.num_seconds(),
        refresh_token: None,
        id_token: None,
        scope,
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
            client: client.id.clone(),
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
        tracing::warn!(
            client = %client.id,
            "a spent code or rotated refresh token was used again; its tokens are revoked"
        );
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
    ip: ClientIp,
    headers: HeaderMap,
    Form(form): Form<RevokeForm>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &TOKEN_LIMIT, &RateKey::ip(ip))
        .await?;
    let Some(client) = authenticate(
        &ctx.state.db,
        issuer,
        &headers,
        form.client_id.as_deref(),
        form.client_secret.as_deref(),
    )
    .await?
    else {
        return Ok(oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"));
    };
    let Some(token) = &form.token else {
        return Ok(oauth_error(StatusCode::BAD_REQUEST, "invalid_request"));
    };
    let mut conn = ctx.state.db.acquire().await?;
    tokens::revoke_token(&mut conn, token, &client.id, issuer.now()).await?;
    Ok(no_store(StatusCode::OK.into_response()))
}
