//! `/oauth2/register`: dynamic client registration (RFC 7591), for agents
//! that connect without a person setting them up first (MCP).
//!
//! Only public clients are registered (`token_endpoint_auth_method:
//! none`), using the code flow with PKCE, with `https` or loopback redirect
//! URIs. A registered client can do nothing on its own: every token needs a
//! person's consent, it is shown as unverified, and it may never hold an
//! `identity:*` scope. There is no registration access token (RFC 7592 is
//! not offered). Registrations are limited per address, and a client unused
//! for 30 days is deleted (`identity.prune`).

use std::time::Duration;

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse as _, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    Operation, Result, Risk,
    rate_limit::{ClientIp, Limit, RateKey},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Ctx;
use crate::{
    domain::{
        accounts,
        clients::{begin_as_server, redirect_host},
    },
    metadata::{Invalid, Registration},
    web::no_store,
};

/// Registrations one address may make.
const REGISTER_LIMIT: Limit = Limit::new("identity.register", 20, Duration::from_hours(1));
/// Registrations the whole instance takes per hour, from every address.
const REGISTER_ALL: Limit = Limit::new("identity.register_all", 300, Duration::from_hours(1));

/// What a client sends (RFC 7591 §2; the fields read).
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// Where codes may go: `https`, or loopback `http` (any port).
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    /// Must be `none` (or missing).
    pub token_endpoint_auth_method: Option<String>,
    /// `authorization_code` and optionally `refresh_token`.
    pub grant_types: Option<Vec<String>>,
    /// `code`.
    pub response_types: Option<Vec<String>>,
    /// A name to show on the consent screen (with "unverified").
    pub client_name: Option<String>,
    /// Space-separated scopes it may ask for; missing means every scope a
    /// third party may hold.
    pub scope: Option<String>,
}

/// The registered client (RFC 7591 §3.2.1).
#[derive(Debug, Serialize, ToSchema)]
pub struct Registered {
    /// The new client id.
    pub client_id: String,
    /// When it was issued (seconds since the epoch).
    pub client_id_issued_at: i64,
    /// The name it will be shown with.
    pub client_name: String,
    /// Its redirect URIs.
    pub redirect_uris: Vec<String>,
    /// Its grant types.
    pub grant_types: Vec<String>,
    /// `["code"]`.
    pub response_types: [&'static str; 1],
    /// `none`.
    pub token_endpoint_auth_method: &'static str,
    /// The scopes it may ask for.
    pub scope: String,
}

/// An RFC 7591 §3.2.2 error.
fn refuse(error: &'static str, description: &'static str) -> Response {
    no_store(
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error, "error_description": description })),
        )
            .into_response(),
    )
}

/// Register a public client.
#[utoipa::path(
    post,
    path = "/oauth2/register",
    tag = "identity",
    operation_id = "identity.register",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.client"])))),
    request_body = RegisterRequest,
    security(()),
    responses(
        (status = 201, description = "Registered.", body = Registered),
        (status = 400, description = "`invalid_redirect_uri` or `invalid_client_metadata`."),
        (status = 404, description = "Another identity provider serves this instance."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn register(
    State(ctx): State<Ctx>,
    ip: ClientIp,
    body: std::result::Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &REGISTER_LIMIT, &RateKey::ip(ip))
        .await?;
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &REGISTER_ALL, &RateKey::text("instance"))
        .await?;
    let Ok(Json(request)) = body else {
        return Ok(refuse(
            "invalid_client_metadata",
            "the body must be a JSON object of client metadata",
        ));
    };
    if request
        .token_endpoint_auth_method
        .as_deref()
        .is_some_and(|method| method != "none")
    {
        return Ok(refuse(
            "invalid_client_metadata",
            "only public clients (token_endpoint_auth_method none) are registered",
        ));
    }
    let fallback = request
        .redirect_uris
        .first()
        .and_then(|uri| redirect_host(uri))
        .unwrap_or_else(|| "App".to_owned());
    let described = match (Registration {
        redirect_uris: request.redirect_uris,
        grant_types: request.grant_types,
        response_types: request.response_types,
        scope: request.scope,
        client_name: request.client_name,
    })
    .check(&fallback, &issuer.api_scopes)
    {
        Ok(described) => described,
        Err(Invalid::BadRedirects) => {
            return Ok(refuse(
                "invalid_redirect_uri",
                "1 to 10 redirect URIs, each https or loopback http, without fragment or userinfo",
            ));
        }
        Err(_) => {
            return Ok(refuse(
                "invalid_client_metadata",
                "only the authorization code flow is supported",
            ));
        }
    };
    let mut id = [0_u8; 16];
    aws_lc_rs::rand::fill(&mut id).map_err(|_| campbooks_kernel::Error::internal("no entropy"))?;
    let client_id = format!("dcr_{}", URL_SAFE_NO_PAD.encode(id));
    let now = issuer.now();
    let mut tx = begin_as_server(&ctx.state.db).await?;
    sqlx::query(
        "INSERT INTO identity.clients
             (client_id, kind, name, redirect_uris, grant_types, auth_method, allowed_scopes,
              created_at)
         VALUES ($1, 'dynamic', $2, $3, $4, 'none', $5, $6)",
    )
    .bind(&client_id)
    .bind(&described.name)
    .bind(&described.redirect_uris)
    .bind(&described.grant_types)
    .bind(&described.scopes)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    accounts::audit(
        &mut tx,
        None,
        "client_registered",
        serde_json::json!({ "client": client_id, "kind": "dynamic" }),
        now,
    )
    .await?;
    tx.commit().await?;
    tracing::info!(client = %client_id, "a client registered itself");
    let body = Registered {
        client_id,
        client_id_issued_at: now.timestamp(),
        client_name: described.name,
        redirect_uris: described.redirect_uris,
        grant_types: described.grant_types,
        response_types: ["code"],
        token_endpoint_auth_method: "none",
        scope: described.scopes.join(" "),
    };
    Ok(no_store((StatusCode::CREATED, Json(body)).into_response()))
}
