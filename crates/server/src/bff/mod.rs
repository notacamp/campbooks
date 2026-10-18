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
//! | `GET /bff/login?return_to=&max_age=` | starts a sign-in, redirects to the IdP |
//! | `GET /bff/callback` | finishes it: code exchange, ID token check, session |
//! | `GET /bff/session` | who is signed in (never a token) |
//! | `POST /bff/logout` | ends the session, revokes, returns the IdP logout URL |
//! | `POST /bff/backchannel-logout` | the IdP ends sessions (logout token) |
//! | `/bff/api/*` | the API, with the session's access token attached |
//! | `/bff/admin/*` | the admin API (ADR 0015), the same way |
//!
//! Unsafe methods on cookie-authenticated endpoints pass a CSRF check
//! ([`csrf`]). The design and threat model are in
//! `openspec/changes/web-bff/design.md`.

pub mod config;
mod cookies;
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
use campbooks_identity::events::AccountSuspended;
use campbooks_kernel::{
    AppState, Error, Event, EventHandler, Job, JobContext, JobError,
    auth::TokenVerifier,
    jobs::Background,
    rate_limit::{Limit, RateLimits, TrustedProxies},
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

use campbooks_kernel::crypto::{self, Cipher, hash, random_token};

use self::{
    config::{ROTATION_GRACE, Timeouts},
    cookies::Cookies,
    oidc::{Oidc, OidcError},
    store::{Reissue, Session},
};

/// Owner of the BFF's background work and its Postgres schema.
pub const OWNER: &str = "bff";

/// Name of the OpenAPI security scheme of cookie-authenticated BFF
/// operations: `security(("session" = []))`.
pub const SESSION_SCHEME: &str = "session";

/// How often expired sessions and sign-ins are deleted.
pub const PRUNE_EVERY: Duration = Duration::from_mins(15);

/// Refresh the access token this long before it expires. Some issuers
/// refuse a refresh token earlier than 60 s before the access token's expiry
/// (and revoke the session if one tries), so this stays well inside that window.
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
    /// The admin API's routes: `/bff/admin/*` runs requests through them.
    admin: Router,
    /// Serialize refreshes within this process; a row lock serializes them
    /// across processes.
    refresh_locks: Box<[tokio::sync::Mutex<()>]>,
    /// Counts sign-ins started per address ([`LOGIN_LIMIT`]).
    rate_limits: RateLimits,
    /// The proxies whose `X-Forwarded-For` names the client.
    trusted_proxies: TrustedProxies,
}

/// Sign-ins one address may start (each stores a sign-in transaction and
/// may make the server fetch the provider's metadata).
const LOGIN_LIMIT: Limit = Limit::new("bff.login", 30, Duration::from_mins(1));

impl Bff {
    /// The BFF for `settings` (`None`: web sign-in is off and every endpoint
    /// but `/bff/session` answers 503). `api` is the API router requests to
    /// `/bff/api/*` run through, `admin` the admin API's for `/bff/admin/*`.
    pub fn new(
        settings: Option<Settings>,
        state: &AppState,
        api: Router,
        admin: Router,
        issuer: Router,
    ) -> Self {
        let inner = settings.map(|settings| {
            Arc::new(Inner {
                db: state.db.clone(),
                verifier: state.auth.clone(),
                oidc: Oidc::new(&settings, issuer),
                cipher: settings.cipher,
                cookies: Cookies::new(settings.secure_cookies),
                public_origin: settings.public_origin.into(),
                timeouts: settings.timeouts,
                api: api.route_layer(axum::middleware::from_fn(crate::telemetry::served_route)),
                admin: admin.route_layer(axum::middleware::from_fn(crate::telemetry::served_route)),
                refresh_locks: (0..REFRESH_LOCKS)
                    .map(|_| tokio::sync::Mutex::new(()))
                    .collect(),
                rate_limits: state.rate_limits.clone(),
                trusted_proxies: state.config.trusted_proxies.clone(),
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
            )
            .route(
                "/bff/admin/{*path}",
                get(handlers::admin_get)
                    .post(handlers::admin_post)
                    .put(handlers::admin_put)
                    .patch(handlers::admin_patch)
                    .delete(handlers::admin_delete),
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

/// The BFF's part of the access catalogue (ADR 0017). Signing in is public;
/// a session belongs to the person signed in, who alone reads and ends it
/// (the cookie proves it); the proxies forward to the API, whose own
/// operations authorize each request.
static RESOURCES: [campbooks_kernel::authz::ResourceType; 3] = {
    use campbooks_kernel::authz::{
        Action::{Delete, Read, Write},
        Grant, Grantee, Owner, ResourceType, Tenancy,
    };
    [
        ResourceType {
            grants: &[Grant::new(Grantee::Anyone, &[Read, Write, Delete])],
            ..ResourceType::new(
                "bff.login",
                "A web sign-in in progress (the OIDC round trip) and the identity provider's \
                 back-channel logout: public, each step proving the one before.",
                Tenancy::Public,
            )
        },
        ResourceType {
            owner: Owner::Account,
            owner_actions: &[Read, Delete],
            ..ResourceType::new(
                "bff.session",
                "A browser's session: its cookie's holder reads and ends it.",
                Tenancy::Account,
            )
        },
        ResourceType {
            owner: Owner::Account,
            owner_actions: &[Read, Write],
            ..ResourceType::new(
                "bff.proxy",
                "The API and admin API as the signed-in person: forwarded with their own \
                 token, so every operation there authorizes the request itself.",
                Tenancy::Account,
            )
        },
    ]
};

/// The BFF's resource types.
pub fn resources() -> campbooks_kernel::authz::Resources {
    campbooks_kernel::authz::Resources::declared(&RESOURCES)
}

/// The BFF's background work: pruning ended sessions, and ending a
/// suspended account's sessions, or those of a person who signed out
/// everywhere.
pub fn background() -> Background {
    Background::new()
        .schedule::<PruneSessions>(PRUNE_EVERY)
        .on_event::<EndSuspendedSessions>()
        .on_event::<EndSignedOutSessions>()
}

/// `bff.end_signed_out_sessions` ← `identity.signed_out_everywhere`: every
/// web session of the account ends (their refresh tokens were already
/// revoked at the issuer; this ends them now rather than at the next
/// refresh).
#[derive(Debug, Clone, Copy, Default)]
pub struct EndSignedOutSessions;

impl EventHandler for EndSignedOutSessions {
    type Event = campbooks_identity::events::SignedOutEverywhere;
    const NAME: &'static str = "bff.end_signed_out_sessions";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<Self::Event>) -> Result<(), JobError> {
        let subject = event.payload.account.to_string();
        let ended = store::delete_for_logout(ctx.pool(), Some(&subject), None).await?;
        tracing::info!(
            sessions = ended,
            "ended the web sessions of a sign-out everywhere"
        );
        Ok(())
    }
}

/// An administrator suspended an account (ADR 0015): its web sessions end
/// at once, as an identity provider's back-channel logout would end them
/// (the in-process issuer sends none; it publishes
/// `identity.account_suspended` instead). Its next web request is asked to
/// sign in, and signing in tells the person the account is suspended.
#[derive(Debug, Clone, Copy, Default)]
pub struct EndSuspendedSessions;

impl EventHandler for EndSuspendedSessions {
    type Event = AccountSuspended;
    const NAME: &'static str = "bff.end_suspended_sessions";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<AccountSuspended>) -> Result<(), JobError> {
        let subject = event.payload.account.to_string();
        let ended = store::delete_for_logout(ctx.pool(), Some(&subject), None).await?;
        tracing::info!(sessions = ended, "ended a suspended account's web sessions");
        Ok(())
    }
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
        let (sessions, logins) = store::prune(ctx.pool()).await?;
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
    /// rotating its token when due. A session that predates the scopes the
    /// BFF asks for now is retired and counts as none.
    async fn current(&self, headers: &HeaderMap) -> Result<Option<Current>, Error> {
        self.lookup(headers, true).await
    }

    /// [`Self::current`], keeping a session whatever scopes it asked for:
    /// for signing out, which ends it in full.
    async fn current_any_scopes(&self, headers: &HeaderMap) -> Result<Option<Current>, Error> {
        self.lookup(headers, false).await
    }

    async fn lookup(
        &self,
        headers: &HeaderMap,
        check_scopes: bool,
    ) -> Result<Option<Current>, Error> {
        let Some(token) = self.cookies.session(headers) else {
            return Ok(None);
        };
        let now = Utc::now();
        let token_hash = hash(&token);
        let Some(session) = store::find_session(&self.db, &token_hash).await? else {
            if store::is_retired(&self.db, &token_hash).await? {
                // A token two or more rotations old: a copied cookie.
                tracing::warn!("a retired web session token was presented again; the session ends");
                self.terminate(&token_hash).await?;
            }
            return Ok(None);
        };
        let mut rotated = None;
        if !session.current && session.rotated_at <= now - ROTATION_GRACE {
            if session.token_delivered {
                // A token rotated away minutes ago, presented again after its
                // successor was used: the cookie was copied, and either copy
                // may be the thief's. The session ends everywhere it reaches.
                tracing::warn!(
                    "a rotated-away web session token was presented again; the session ends"
                );
                self.terminate(&token_hash).await?;
                return Ok(None);
            }
            // The successor never reached this browser (the response that
            // carried it was lost): it gets another one. When a concurrent
            // request (a page waking up fires several) reissued first, this
            // one is within the new grace and goes through as it is; when
            // the successor was presented meanwhile, this is a copy.
            let fresh = random_token();
            match store::reissue(
                &self.db,
                &token_hash,
                &hash(&fresh),
                now,
                now - ROTATION_GRACE,
            )
            .await?
            {
                Reissue::Reissued => {
                    rotated = Some(
                        self.cookies
                            .set_session(&fresh, seconds_until(session.expires_at, now)),
                    );
                }
                Reissue::WithinGrace => {}
                Reissue::Delivered => {
                    tracing::warn!(
                        "a rotated-away web session token was presented again; the session ends"
                    );
                    self.terminate(&token_hash).await?;
                    return Ok(None);
                }
                Reissue::Gone => return Ok(None),
            }
        } else if session.current && !session.token_delivered {
            store::delivered(&self.db, &token_hash).await?;
        }
        if check_scopes && !asked_for_all(session.scopes.as_deref(), self.oidc.scopes()) {
            // Signed in before a release that asks for more (a new module's
            // scopes): its tokens can never get them, and every feature
            // behind them would answer 403. It ends like an expired session,
            // so the app signs in again and comes back with every scope; the
            // identity provider's own session is left alone, so that sign-in
            // can go straight through.
            tracing::info!(
                "a web session predates scopes the BFF now asks for; it ends so the next sign-in asks for them"
            );
            self.retire(&token_hash).await?;
            return Ok(None);
        }
        if let Some(idle_until) = touch_until(
            now,
            session.last_seen_at,
            self.timeouts.idle,
            session.expires_at,
        ) {
            store::touch(&self.db, &token_hash, now, idle_until).await?;
        }
        if session.current && now - session.rotated_at >= self.timeouts.rotate {
            let fresh = random_token();
            if store::rotate(&self.db, &token_hash, &hash(&fresh), now).await? {
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
            return self.open_access(session, &session.access_token).await;
        }
        self.refresh(session, None).await
    }

    /// Refresh the session's access token: when it is about to expire, or,
    /// given `refused` (the token the API just refused as invalid: its
    /// sign-in ended, campbooks-cloud#143), now, unless another request
    /// already replaced it. A refresh the identity provider refuses ends
    /// the session, so the app signs in again instead of failing until the
    /// refused token would have expired.
    async fn refresh(
        &self,
        session: &Session,
        refused: Option<&str>,
    ) -> Result<String, AccessError> {
        let lock = &self.refresh_locks[lock_index(session.id)];
        let _guard = lock.lock().await;
        let mut tx = store::begin_refresh(&self.db).await?;
        let Some(tokens) = store::lock_tokens(&mut tx, &session.presented).await? else {
            return Err(AccessError::SessionEnded);
        };
        let now = Utc::now();
        let current = match refused {
            // Replaced by another request while this one waited: use it.
            Some(refused) => {
                let Ok(held) = self.cipher.open(
                    &tokens.access_token,
                    &sealed_for(session.id, "access_token"),
                ) else {
                    // Ending the session deletes its row on another
                    // connection: this transaction's lock on it goes first
                    // (campbooks-cloud#259).
                    drop(tx);
                    return self.unreadable(&session.presented).await;
                };
                (held != refused).then_some(held)
            }
            None => None,
        };
        if let Some(held) = current {
            tx.commit().await?;
            return Ok(held);
        }
        if refused.is_none() && now + REFRESH_BEFORE_EXPIRY < tokens.access_expires_at {
            // Another request refreshed while this one waited.
            tx.commit().await?;
            return self.open_access(session, &tokens.access_token).await;
        }
        // A refused token is no longer valid, whenever it would expire.
        let still_valid = refused.is_none() && now < tokens.access_expires_at;
        let Some(sealed_refresh) = &tokens.refresh_token else {
            tx.commit().await?;
            return if still_valid {
                self.open_access(session, &tokens.access_token).await
            } else {
                self.end(&session.presented).await
            };
        };
        let Ok(refresh_token) = self
            .cipher
            .open_checked(sealed_refresh, &sealed_for(session.id, "refresh_token"))
        else {
            drop(tx);
            return self.unreadable(&session.presented).await;
        };
        match self.oidc.refresh(&refresh_token.value).await {
            Ok(fresh) => {
                let sealed = self.seal_refreshed(
                    session.id,
                    &fresh,
                    &refresh_token,
                    tokens.id_token.as_deref(),
                );
                store::update_tokens(
                    &mut tx,
                    &session.presented,
                    &store::Refreshed {
                        access_token: &sealed.access,
                        access_expires_at: now + expires_in(fresh.expires_in),
                        refresh_token: sealed
                            .refresh
                            .as_deref()
                            .or(Some(sealed_refresh.as_slice())),
                        id_token: sealed.id.as_deref(),
                    },
                )
                .await?;
                tx.commit().await?;
                Ok(fresh.access_token)
            }
            Err(OidcError::Rejected(reason)) => {
                tracing::info!(%reason, "the identity provider refused to refresh a web session; it ends");
                store::delete_session(&mut tx, &session.presented).await?;
                tx.commit().await?;
                Err(AccessError::SessionEnded)
            }
            Err(OidcError::Unavailable(reason)) => {
                drop(tx);
                if still_valid {
                    tracing::warn!(%reason, "could not refresh a web session yet; using its current access token");
                    self.open_access(session, &tokens.access_token).await
                } else {
                    Err(AccessError::Failed(Error::Unavailable(
                        campbooks_kernel::i18n::MESSAGES.reason("idp_unavailable"),
                    )))
                }
            }
        }
    }

    /// What a refresh stores, sealed with the current key: the new access
    /// token; the refresh token the identity provider sent back, or the one
    /// used when the previous instance secret sealed it; and the stored ID
    /// token, sealed again when the previous key sealed it
    /// (campbooks-cloud#191). `None` keeps what is stored.
    fn seal_refreshed(
        &self,
        id: Uuid,
        fresh: &oidc::TokenResponse,
        used: &campbooks_kernel::crypto::Opened,
        stored_id: Option<&[u8]>,
    ) -> SealedRefresh {
        let refresh = fresh
            .refresh_token
            .as_deref()
            .or_else(|| used.by_previous.then_some(used.value.as_str()))
            .map(|token| self.cipher.seal(token, &sealed_for(id, "refresh_token")));
        let context = sealed_for(id, "id_token");
        let id_token = stored_id
            .and_then(|sealed| self.cipher.open_checked(sealed, &context).ok())
            .filter(|opened| opened.by_previous)
            .map(|opened| self.cipher.seal(&opened.value, &context));
        SealedRefresh {
            access: self
                .cipher
                .seal(&fresh.access_token, &sealed_for(id, "access_token")),
            refresh,
            id: id_token,
        }
    }

    async fn open_access(&self, session: &Session, sealed: &[u8]) -> Result<String, AccessError> {
        match self
            .cipher
            .open(sealed, &sealed_for(session.id, "access_token"))
        {
            Ok(token) => Ok(token),
            Err(_) => self.unreadable(&session.presented).await,
        }
    }

    /// The session's tokens cannot be decrypted (e.g. `CAMPBOOKS_SECRET_KEY`
    /// changed): it ends, and the user signs in again.
    async fn unreadable(&self, presented: &[u8]) -> Result<String, AccessError> {
        tracing::warn!(
            "a web session's stored tokens could not be decrypted (was CAMPBOOKS_SECRET_KEY \
             changed?); the session ends"
        );
        self.end(presented).await
    }

    /// End the session that holds or held the token hashing to `presented`
    /// (current, previous or retired) everywhere it reaches: delete it,
    /// revoke its refresh token at the identity provider, and end the
    /// in-process issuer's session (its ID token is the proof). Whether the
    /// issuer session was ended here. Failures past the deletion are logged:
    /// the web session is gone either way.
    async fn terminate(&self, presented: &[u8]) -> sqlx::Result<bool> {
        let Some(store::Ended {
            id,
            refresh_token,
            id_token,
        }) = store::end_session(&self.db, presented).await?
        else {
            return Ok(false);
        };
        self.revoke_refresh(id, refresh_token).await;
        // The in-process issuer's session ends here, with the ID token as
        // proof (ADR 0014): nothing for the browser to visit or forge.
        if self.oidc.in_process()
            && let Ok(id_token) = self.cipher.open(&id_token, &sealed_for(id, "id_token"))
        {
            match self.oidc.end_issuer_session(&id_token).await {
                Ok(()) => return Ok(true),
                Err(err) => {
                    tracing::warn!(error = %err, "could not end the issuer session of an ended web session");
                }
            }
        }
        Ok(false)
    }

    /// End the session `presented` names here only: delete it and revoke
    /// its refresh token, leaving the identity provider's session as it is
    /// (a new sign-in may go straight through).
    async fn retire(&self, presented: &[u8]) -> sqlx::Result<()> {
        if let Some(ended) = store::end_session(&self.db, presented).await? {
            self.revoke_refresh(ended.id, ended.refresh_token).await;
        }
        Ok(())
    }

    /// Revoke an ended session's refresh token at the identity provider;
    /// failures are logged, the session is gone either way.
    async fn revoke_refresh(&self, id: Uuid, sealed: Option<Vec<u8>>) {
        let Some(sealed) = sealed else {
            return;
        };
        match self.cipher.open(&sealed, &sealed_for(id, "refresh_token")) {
            Ok(refresh_token) => {
                if let Err(err) = self.oidc.revoke(&refresh_token).await {
                    tracing::warn!(error = %err, "could not revoke an ended session's refresh token");
                }
            }
            Err(_) => {
                tracing::warn!("an ended session's refresh token could not be decrypted");
            }
        }
    }

    async fn end(&self, presented: &[u8]) -> Result<String, AccessError> {
        store::end_session(&self.db, presented).await?;
        Err(AccessError::SessionEnded)
    }
}

/// The sealed tokens a refresh stores ([`Inner::seal_refreshed`]).
struct SealedRefresh {
    access: Vec<u8>,
    refresh: Option<Vec<u8>>,
    id: Option<Vec<u8>>,
}

/// Lifetime of a new access token: the IdP's `expires_in`, or a cautious
/// default when it gives none.
fn expires_in(seconds: Option<u64>) -> TimeDelta {
    TimeDelta::seconds(seconds.and_then(|s| i64::try_from(s).ok()).unwrap_or(300))
}

/// The idle deadline to record a use at `now` with, when one is due: at
/// most every [`TOUCH_EVERY`], `idle` after the use and never past the
/// session's absolute end. `None` too when that end is not after `now`: the
/// database (by its own clock) still found the session, but this server's
/// clock is ahead of it, and `bff.touch_session` refuses a deadline that is
/// not after the use (campbooks-cloud#213, item 9). The session ends on
/// either clock soon enough; the request goes on.
fn touch_until(
    now: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    idle: TimeDelta,
    expires_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let idle_until = (now + idle).min(expires_at);
    (now - last_seen_at >= TOUCH_EVERY && now < idle_until).then_some(idle_until)
}

/// Whether a session that asked for `had` at sign-in asked for every scope in
/// `wanted` (both space-separated, in any order). A session that recorded
/// none (`None`, from before they were recorded) asked for none.
fn asked_for_all(had: Option<&str>, wanted: &str) -> bool {
    had.is_some_and(|had| {
        wanted
            .split_whitespace()
            .all(|scope| had.split_whitespace().any(|h| h == scope))
    })
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
