//! The issuer's protocol endpoints: OAuth 2.1 / OIDC
//! (`/.well-known/*`, `/oauth2/*`) and the provider round trips and actions
//! behind the sign-in screens (`/auth/*`).
//!
//! These serve browsers and OAuth clients, not the operation catalogue: like
//! the web BFF's, they are documented in the OpenAPI document but no
//! automation or assistant may call them. When the issuer does not run in
//! this process (another identity provider is configured), every one of them
//! answers 404.
//!
//! The sign-in screens themselves are the web client's (`/sign-in`,
//! `/sign-in/new`); these endpoints redirect there and answer their calls.

pub mod authorize;
pub mod discovery;
pub mod end_session;
pub mod sign_in;
pub mod token;

use std::sync::Arc;

use axum::{Router, http::HeaderMap};
use campbooks_kernel::{AppState, Error, Result};
use chrono::{DateTime, Utc};
use utoipa::openapi::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::deps::{Deps, Issuer};

/// The web client's sign-in screen.
pub const SIGN_IN_PATH: &str = "/sign-in";
/// The web client's new-account screen.
pub const NEW_ACCOUNT_PATH: &str = "/sign-in/new";

/// What the protocol handlers reach. Cheap to clone.
#[derive(Clone)]
pub struct Ctx {
    /// The application state.
    pub state: AppState,
    /// The module's dependencies.
    pub deps: Deps,
}

impl Ctx {
    /// The in-process issuer; 404 when another identity provider is used.
    pub fn issuer(&self) -> Result<&Arc<Issuer>> {
        self.deps.issuer.as_ref().ok_or(Error::NotFound)
    }
}

impl Issuer {
    /// The time.
    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// The issuer session cookie the request carries.
    pub fn session_cookie(&self, headers: &HeaderMap) -> Option<String> {
        self.cookies.read(headers, crate::web::SESSION_COOKIE)
    }
}

fn openapi_router() -> OpenApiRouter<Ctx> {
    OpenApiRouter::new()
        .routes(routes!(discovery::openid_configuration))
        .routes(routes!(discovery::authorization_server_metadata))
        .routes(routes!(discovery::jwks))
        .routes(routes!(authorize::authorize))
        .routes(routes!(token::token))
        .routes(routes!(token::revoke))
        .routes(routes!(
            end_session::end_session,
            end_session::end_session_post
        ))
        .routes(routes!(sign_in::request))
        .routes(routes!(sign_in::start))
        .routes(routes!(sign_in::callback))
        .routes(routes!(sign_in::new_account, sign_in::create_account))
}

/// The protocol endpoints over `state` and `deps`, ready to merge into the
/// server's application.
pub fn router(state: AppState, deps: Deps) -> Router {
    let (router, _) = openapi_router().split_for_parts();
    router.with_state(Ctx { state, deps })
}

/// Their OpenAPI document.
pub fn openapi() -> OpenApi {
    openapi_router().into_openapi()
}
