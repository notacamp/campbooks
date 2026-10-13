//! The web backend-for-frontend (ADR 0004): the browser never holds a token.
//!
//! The server runs the OIDC authorization-code flow (PKCE S256) as
//! the confidential client `campbooks-web`, keeps the tokens it gets in
//! Postgres (schema `bff`, encrypted), and gives the browser one opaque,
//! `HttpOnly`, `SameSite=Strict` session cookie. Web API calls go to
//! `/bff/api/*`, which attaches the session's access token and runs the
//! request through the API router in-process.
//!
//! | Endpoint | Does |
//! | --- | --- |
//! | `GET /bff/login?return_to=` | starts a sign-in, redirects to the IdP |
//! | `GET /bff/callback` | finishes it: code exchange, ID token check, session |
//! | `GET /bff/session` | who is signed in (never a token) |
//! | `POST /bff/logout` | ends the session, revokes, returns the IdP logout URL |
//! | `POST /bff/backchannel-logout` | the IdP ends sessions (logout token) |
//! | `/bff/api/*` | the API, with the session's access token attached |
//!
//! Unsafe methods on cookie-authenticated endpoints pass a CSRF check
//! ([`csrf`]). The design and threat model are in
//! `openspec/changes/web-bff/design.md`.

pub mod config;
mod cookies;
mod crypto;
mod csrf;
mod handlers;
mod oidc;
mod store;

#[cfg(test)]
mod tests;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    http::HeaderMap,
    routing::{get, post},
};
use campbooks_kernel::{
    AppState, Error, Job, JobContext, JobError, auth::TokenVerifier, jobs::Background,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{
    OpenApi as _,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};
use uuid::Uuid;

pub use config::{Loaded, Settings};
pub use cookies::SESSION_COOKIE;
pub use csrf::CSRF_HEADER;

use self::{
    config::{ROTATION_GRACE, Timeouts},
    cookies::Cookies,
    crypto::{Cipher, hash, random_token},
    oidc::{Oidc, OidcError},
    store::Session,
};

/// Owner of the BFF's background work and its Postgres schema.
pub const OWNER: &str = "bff";

/// Name of the OpenAPI security scheme of cookie-authenticated BFF
/// operations: `security(("session" = []))`.
pub const SESSION_SCHEME: &str = "session";

/// How often expired sessions and sign-ins are deleted.
pub const PRUNE_EVERY: Duration = Duration::from_mins(15);

/// Refresh the access token this long before it expires. Rauthy refuses a
/// refresh token earlier than 60 s before the access token's expiry (and
/// revokes the session if one tries), so this stays well inside that window.
pub const REFRESH_BEFORE_EXPIRY: TimeDelta = TimeDelta::seconds(30);

/// Record use of a session at most this often.
const TOUCH_EVERY: TimeDelta = TimeDelta::minutes(1);

/// Number of in-process refresh locks; sessions hash onto them.
const REFRESH_LOCKS: usize = 64;

/// Longest stored `User-Agent`.
const MAX_USER_AGENT_CHARS: usize = 256;

/// The BFF's routes and state. Cheap to clone.
#[derive(Clone)]
pub struct Bff {
    inner: Option<Arc<Inner>>,
}

impl std::fmt::Debug for Bff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bff")
            .field("enabled", &self.inner.is_some())
            .finish_non_exhaustive()
    }
}

struct Inner {
    db: PgPool,
    verifier: TokenVerifier,
    oidc: Oidc,
    cipher: Arc<Cipher>,
    cookies: Cookies,
    public_origin: Arc<str>,
    timeouts: Timeouts,
    /// The API's routes (no middleware, no web client): `/bff/api/*` runs
    /// requests through it in-process.
    api: Router,
    /// Serialize refreshes within this process; a row lock serializes them
    /// across processes.
    refresh_locks: Box<[tokio::sync::Mutex<()>]>,
}

impl Bff {
    /// The BFF for `settings` (`None`: web sign-in is off and every endpoint
    /// but `/bff/session` answers 503). `api` is the API router requests to
    /// `/bff/api/*` run through.
    pub fn new(settings: Option<Settings>, state: &AppState, api: Router) -> Self {
        let inner = settings.map(|settings| {
            Arc::new(Inner {
                db: state.db.clone(),
                verifier: state.auth.clone(),
                oidc: Oidc::new(&settings),
                cipher: settings.cipher,
                cookies: Cookies::new(settings.secure_cookies),
                public_origin: settings.public_origin.into(),
                timeouts: settings.timeouts,
                api,
                refresh_locks: (0..REFRESH_LOCKS)
                    .map(|_| tokio::sync::Mutex::new(()))
                    .collect(),
            })
        });
        Self { inner }
    }

    /// The `/bff/*` routes.
    pub fn router(&self) -> Router {
        let public = Router::new()
            .route("/bff/login", get(handlers::login))
            .route("/bff/callback", get(handlers::callback))
            .route("/bff/session", get(handlers::session))
            .route(
                "/bff/backchannel-logout",
                post(handlers::backchannel_logout),
            );
        let mut protected = Router::new()
            .route("/bff/logout", post(handlers::logout))
            .route(
                "/bff/api/{*path}",
                get(handlers::api_get)
                    .post(handlers::api_post)
                    .put(handlers::api_put)
                    .patch(handlers::api_patch)
                    .delete(handlers::api_delete),
            );
        if let Some(inner) = &self.inner {
            protected = protected.layer(axum::middleware::from_fn_with_state(
                Arc::clone(&inner.public_origin),
                csrf::check,
            ));
        }
        public.merge(protected).with_state(self.clone())
    }
}

/// The BFF's background work: pruning ended sessions.
pub fn background() -> Background {
    Background::new().schedule::<PruneSessions>(PRUNE_EVERY)
}

/// Delete ended sessions and expired sign-ins. Scheduled every
/// [`PRUNE_EVERY`]; sessions are checked on every use anyway, so this only
/// keeps the tables small.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneSessions;

impl Job for PruneSessions {
    const NAME: &'static str = "bff.prune_sessions";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let (sessions, logins) = store::prune(ctx.pool(), Utc::now()).await?;
        tracing::info!(sessions, logins, "pruned ended web sessions and sign-ins");
        Ok(())
    }
}

/// The OpenAPI document of the BFF's endpoints.
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = handlers::BffDoc::openapi();
    doc.components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            SESSION_SCHEME,
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                SESSION_COOKIE,
                "The web app's session cookie, set by `/bff/callback` (`HttpOnly`, `Secure`, \
                 `SameSite=Strict`; named `campbooks_session` when the instance's public URL is \
                 plain http). Unsafe methods also need the `X-Campbooks-CSRF: 1` header and a \
                 same-origin request.",
            ))),
        );
    doc
}

/// A session found by its cookie.
struct Current {
    session: Session,
    /// `Set-Cookie` for the session's new token, when this request rotated it.
    rotated: Option<axum::http::HeaderValue>,
}

/// Why no access token could be produced.
enum AccessError {
    /// The session ended (refresh refused, tokens unreadable, row gone):
    /// sign in again.
    SessionEnded,
    /// Something else failed; the request may be retried.
    Failed(Error),
}

impl From<sqlx::Error> for AccessError {
    fn from(err: sqlx::Error) -> Self {
        Self::Failed(err.into())
    }
}

/// Associated data binding a sealed column value to its row.
fn sealed_for(id: Uuid, column: &str) -> Vec<u8> {
    format!("bff.sessions:{id}:{column}").into_bytes()
}

/// Seconds from `now` until `until`, at least 0.
fn seconds_until(until: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    (until - now).num_seconds().max(0)
}

impl Inner {
    /// The live session the request's cookie names, recording its use and
    /// rotating its token when due.
    async fn current(&self, headers: &HeaderMap) -> Result<Option<Current>, Error> {
        let Some(token) = self.cookies.session(headers) else {
            return Ok(None);
        };
        let now = Utc::now();
        let token_hash = hash(&token);
        let Some(session) =
            store::find_session(&self.db, &token_hash, now, now - ROTATION_GRACE).await?
        else {
            return Ok(None);
        };
        if now - session.last_seen_at >= TOUCH_EVERY {
            let idle_until = (now + self.timeouts.idle).min(session.expires_at);
            store::touch(&self.db, session.id, now, idle_until).await?;
        }
        let mut rotated = None;
        if session.current && now - session.rotated_at >= self.timeouts.rotate {
            let fresh = random_token();
            if store::rotate(&self.db, session.id, &token_hash, &hash(&fresh), now).await? {
                rotated = Some(
                    self.cookies
                        .set_session(&fresh, seconds_until(session.expires_at, now)),
                );
            }
        }
        Ok(Some(Current { session, rotated }))
    }

    /// The session's access token, refreshed first when it is about to
    /// expire. Refreshes are single-flight per session: concurrent requests
    /// wait for the one refreshing and use its result.
    async fn access_token(&self, session: &Session) -> Result<String, AccessError> {
        if Utc::now() + REFRESH_BEFORE_EXPIRY < session.access_expires_at {
            return self.open_access(session.id, &session.access_token).await;
        }
        let lock = &self.refresh_locks[lock_index(session.id)];
        let _guard = lock.lock().await;
        let mut tx = self.db.begin().await?;
        let Some(tokens) = store::lock_tokens(&mut tx, session.id).await? else {
            return Err(AccessError::SessionEnded);
        };
        let now = Utc::now();
        if now + REFRESH_BEFORE_EXPIRY < tokens.access_expires_at {
            // Another request refreshed while this one waited.
            tx.commit().await?;
            return self.open_access(session.id, &tokens.access_token).await;
        }
        let still_valid = now < tokens.access_expires_at;
        let Some(sealed_refresh) = &tokens.refresh_token else {
            tx.commit().await?;
            return if still_valid {
                self.open_access(session.id, &tokens.access_token).await
            } else {
                self.end(session.id).await
            };
        };
        let Ok(refresh_token) = self
            .cipher
            .open(sealed_refresh, &sealed_for(session.id, "refresh_token"))
        else {
            drop(tx);
            return self.unreadable(session.id).await;
        };
        match self.oidc.refresh(&refresh_token).await {
            Ok(fresh) => {
                let expires_at = now + expires_in(fresh.expires_in);
                let access = self
                    .cipher
                    .seal(&fresh.access_token, &sealed_for(session.id, "access_token"));
                let refresh = fresh.refresh_token.as_deref().map(|token| {
                    self.cipher
                        .seal(token, &sealed_for(session.id, "refresh_token"))
                });
                store::update_tokens(
                    &mut tx,
                    session.id,
                    &access,
                    expires_at,
                    refresh.as_deref().or(Some(sealed_refresh.as_slice())),
                )
                .await?;
                tx.commit().await?;
                Ok(fresh.access_token)
            }
            Err(OidcError::Rejected(reason)) => {
                tracing::info!(%reason, "the identity provider refused to refresh a web session; it ends");
                store::delete_session(&mut tx, session.id).await?;
                tx.commit().await?;
                Err(AccessError::SessionEnded)
            }
            Err(OidcError::Unavailable(reason)) => {
                drop(tx);
                if still_valid {
                    tracing::warn!(%reason, "could not refresh a web session yet; using its current access token");
                    self.open_access(session.id, &tokens.access_token).await
                } else {
                    Err(AccessError::Failed(Error::Unavailable(
                        campbooks_kernel::i18n::MESSAGES.reason("idp_unavailable"),
                    )))
                }
            }
        }
    }

    async fn open_access(&self, id: Uuid, sealed: &[u8]) -> Result<String, AccessError> {
        match self.cipher.open(sealed, &sealed_for(id, "access_token")) {
            Ok(token) => Ok(token),
            Err(_) => self.unreadable(id).await,
        }
    }

    /// The session's tokens cannot be decrypted (e.g. `CAMPBOOKS_SECRET_KEY`
    /// changed): it ends, and the user signs in again.
    async fn unreadable(&self, id: Uuid) -> Result<String, AccessError> {
        tracing::warn!(
            "a web session's stored tokens could not be decrypted (was CAMPBOOKS_SECRET_KEY \
             changed?); the session ends"
        );
        self.end(id).await
    }

    async fn end(&self, id: Uuid) -> Result<String, AccessError> {
        store::delete_session(&mut *self.db.acquire().await?, id).await?;
        Err(AccessError::SessionEnded)
    }
}

/// Lifetime of a new access token: the IdP's `expires_in`, or a cautious
/// default when it gives none.
fn expires_in(seconds: Option<u64>) -> TimeDelta {
    TimeDelta::seconds(seconds.and_then(|s| i64::try_from(s).ok()).unwrap_or(300))
}

fn lock_index(id: Uuid) -> usize {
    usize::from(id.as_bytes()[15]) % REFRESH_LOCKS
}

/// The `User-Agent` worth keeping: at most [`MAX_USER_AGENT_CHARS`].
fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.chars().take(MAX_USER_AGENT_CHARS).collect())
}
