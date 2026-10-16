//! Per-caller limits of the authenticated API (`CAMPBOOKS_RATE_LIMITS__*`).
//!
//! One [`middleware`] counts every operation that requires a bearer scope,
//! however it is reached: [`compose`](crate::operations::compose) puts it on
//! every module route (so HTTP requests, the web BFF's proxied calls and
//! [`invoke`](crate::operations::invoke) all pass it) and the server on the
//! admin router. Public operations are not counted here, and neither is a
//! request whose token does not verify (its handler answers 401).
//!
//! **Classes**, from the operation catalogue: [`Class::Read`] for an
//! operation that does not mutate, [`Class::Write`] for one that does,
//! [`Class::Send`] for one declaring
//! [`RateLimit::Send`](crate::operations::RateLimit::Send), and
//! [`Class::Admin`] for the admin API. Each has its own count per window.
//!
//! **Who is counted**: a person per app (`sub` and the token's `azp`), a
//! `client_credentials` client per client id, automation and the assistant
//! acting for a person per (person, caller). Sends are counted per person
//! across all of those.
//!
//! **Where**: reads, writes and admin calls in this process's memory (a
//! token bucket, see `bucket`): with several API processes a caller may get
//! that many times the budget. Sends in Postgres
//! ([`RateLimits`]), shared by every process.
//!
//! Over a limit the answer is [`Error::RateLimited`]: 429 `rate_limited`
//! with `Retry-After`, before the handler runs.

use std::{sync::Arc, time::Duration};

use axum::{
    extract::{MatchedPath, Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse as _, Response},
};
use sqlx::PgPool;

use super::{
    Limit, RateKey, RateLimits, Refusal,
    bucket::{Buckets, DEFAULT_CAPACITY, Verdict},
};
use crate::{
    AppState, Error,
    auth::{AuthenticatedPrincipal, Principal, PrincipalKind, TokenVerifier, bearer_token},
    clock::{Clock, SystemClock},
    config::RateLimitsConfig,
    operations::{Caller, Catalogue, InvokedBy, OperationInfo, RateLimit},
    telemetry::Metrics,
};

/// The name of the Postgres limit sends are counted under.
const SEND_LIMIT: &str = "api.send";

/// What an operation is counted as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// An operation that does not mutate.
    Read,
    /// One that mutates.
    Write,
    /// One that sends mail in the caller's name.
    Send,
    /// An admin API operation.
    Admin,
}

impl Class {
    /// `read`, `write`, `send` or `admin`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Send => "send",
            Self::Admin => "admin",
        }
    }

    /// The class of a catalogued operation.
    pub fn of(operation: &OperationInfo) -> Self {
        let declared = operation.meta.as_ref().and_then(|meta| meta.rate_limit);
        match declared {
            Some(RateLimit::Send) => Self::Send,
            None if operation.mutates() => Self::Write,
            None => Self::Read,
        }
    }
}

/// Who made a request on a principal's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Via {
    /// The app the token was issued to.
    App,
    /// An automation rule, in-process.
    Automation,
    /// The assistant, in-process.
    Assistant,
}

impl Via {
    const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Automation => "automation",
            Self::Assistant => "assistant",
        }
    }
}

impl From<Caller> for Via {
    fn from(caller: Caller) -> Self {
        match caller {
            Caller::Automation => Self::Automation,
            Caller::Assistant => Self::Assistant,
        }
    }
}

/// The per-caller limits of one process: the settings and the in-memory
/// buckets. Cheap to clone; clones share the buckets. In
/// [`AppState::api_limits`].
#[derive(Debug, Clone)]
pub struct ApiLimits {
    config: Arc<RateLimitsConfig>,
    buckets: Buckets,
}

impl Default for ApiLimits {
    fn default() -> Self {
        Self::new(&RateLimitsConfig::default())
    }
}

impl ApiLimits {
    /// Limits for `config`, timed by the system clock.
    pub fn new(config: &RateLimitsConfig) -> Self {
        Self::with_clock(config, Arc::new(SystemClock))
    }

    /// Limits for `config` whose in-memory buckets are timed by `clock`
    /// (tests move a `FixedClock` instead of sleeping; sends are timed by
    /// the database).
    pub fn with_clock(config: &RateLimitsConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            config: Arc::new(config.clone()),
            buckets: Buckets::new(clock, DEFAULT_CAPACITY),
        }
    }

    /// The count and window of `class`; `None` when it is off.
    pub fn limit(&self, class: Class) -> Option<(u32, Duration)> {
        let config = &self.config;
        if !config.enabled {
            return None;
        }
        let (max, seconds) = match class {
            Class::Read => (config.reads, config.reads_window_seconds),
            Class::Write => (config.writes, config.writes_window_seconds),
            Class::Send => (config.sends, config.sends_window_seconds),
            Class::Admin => (config.admin, config.admin_window_seconds),
        };
        (max > 0).then(|| (max, Duration::from_secs(seconds.max(1))))
    }
}

/// What [`middleware`] needs. Install it with
/// `router.route_layer(axum::middleware::from_fn_with_state(layer, rate_limit::api::middleware))`.
#[derive(Debug, Clone)]
pub struct CallerLimits {
    limits: ApiLimits,
    sends: RateLimits,
    db: PgPool,
    auth: TokenVerifier,
    metrics: Metrics,
    /// The module routes' catalogue; `None` on the admin router, whose
    /// every operation is [`Class::Admin`].
    catalogue: Option<Arc<Catalogue>>,
}

impl CallerLimits {
    /// For the module routes of `catalogue`.
    pub fn api(state: &AppState, catalogue: Arc<Catalogue>) -> Self {
        Self::new(state, Some(catalogue))
    }

    /// For the admin router.
    pub fn admin(state: &AppState) -> Self {
        Self::new(state, None)
    }

    fn new(state: &AppState, catalogue: Option<Arc<Catalogue>>) -> Self {
        Self {
            limits: state.api_limits.clone(),
            sends: state.rate_limits.clone(),
            db: state.db.clone(),
            auth: state.auth.clone(),
            metrics: state.metrics.clone(),
            catalogue,
        }
    }

    /// The class of the request's operation and its id (or route); `None`
    /// for a public operation.
    fn class_of(&self, request: &Request) -> Option<(Class, String)> {
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(|path| path.as_str().to_owned())
            .unwrap_or_default();
        let Some(catalogue) = &self.catalogue else {
            return Some((Class::Admin, route));
        };
        match catalogue.route(request.method(), &route) {
            Some(operation) if operation.scopes.is_empty() => None,
            Some(operation) => Some((Class::of(operation), operation.id.clone())),
            // Not catalogued (`cargo xtask arch` forbids it): by method.
            None if matches!(*request.method(), Method::GET | Method::HEAD) => {
                Some((Class::Read, route))
            }
            None => Some((Class::Write, route)),
        }
    }

    /// Count one hit of `principal` (through `via`) in `class`.
    async fn count(
        &self,
        class: Class,
        principal: &Principal,
        via: Via,
        operation: &str,
    ) -> Result<(), Error> {
        let Some((max, window)) = self.limits.limit(class) else {
            return Ok(());
        };
        let who = match principal.kind {
            PrincipalKind::User => format!("user:{}", principal.subject),
            PrincipalKind::Client => format!("client:{}", principal.subject),
        };
        let refusal = if class == Class::Send {
            let limit = Limit::new(SEND_LIMIT, max, window);
            self.sends
                .hit(&self.db, &limit, &RateKey::text(&who))
                .await?
        } else {
            let app = match (via, principal.kind, &principal.client_id) {
                (Via::App, PrincipalKind::User, Some(client)) => client.as_str(),
                _ => "",
            };
            match self
                .limits
                .buckets
                .hit(&(class.as_str(), &who, via.as_str(), app), max, window)
            {
                Verdict::Allowed => None,
                Verdict::Refused { retry_after, first } => Some(Refusal { retry_after, first }),
            }
        };
        let Some(Refusal { retry_after, first }) = refusal else {
            return Ok(());
        };
        let kind = principal.kind.as_str();
        self.metrics
            .rate_limited(class.as_str(), kind, via.as_str());
        if first {
            tracing::info!(
                rate_limit.class = class.as_str(),
                principal.kind = kind,
                rate_limit.caller = via.as_str(),
                operation,
                retry_after,
                "a caller reached an API rate limit"
            );
        }
        Err(Error::RateLimited { retry_after })
    }
}

/// Count the request against its caller's limit (see the module docs);
/// 429 `rate_limited` when it is over. A caller authenticated by token is
/// attached to the request, so later layers and the handler do not verify
/// the token again.
pub async fn middleware(
    State(layer): State<CallerLimits>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some((class, operation)) = layer.class_of(&request) else {
        return next.run(request).await;
    };
    if layer.limits.limit(class).is_none() {
        return next.run(request).await;
    }
    let injected = request
        .extensions()
        .get::<AuthenticatedPrincipal>()
        .map(|AuthenticatedPrincipal(principal)| principal.clone());
    let (principal, via) = if let Some(principal) = injected {
        let via = request
            .extensions()
            .get::<InvokedBy>()
            .map_or(Via::App, |InvokedBy(caller)| Via::from(*caller));
        (principal, via)
    } else {
        let Ok(token) = bearer_token(request.headers()) else {
            return next.run(request).await;
        };
        let Ok(principal) = layer.auth.verify(token).await else {
            // Not counted: the handler answers 401.
            return next.run(request).await;
        };
        request
            .extensions_mut()
            .insert(AuthenticatedPrincipal(principal.clone()));
        (principal, Via::App)
    };
    match layer.count(class, &principal, via, &operation).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_come_from_the_catalogue() {
        let spec = serde_json::json!({"paths": {
            "/r": {
                "get": {"operationId": "m.r", "x-campbooks": {"mutates": false, "automation": false,
                    "assistant": false, "risk": "low", "records": []}},
                "post": {"operationId": "m.w", "x-campbooks": {"mutates": true, "automation": false,
                    "assistant": false, "risk": "low", "records": []}},
                "put": {"operationId": "m.s", "x-campbooks": {"mutates": true, "automation": false,
                    "assistant": false, "risk": "high", "records": [], "rate_limit": "send"}},
                "patch": {"operationId": "m.q", "x-campbooks": {"mutates": false, "automation": false,
                    "assistant": false, "risk": "low", "records": [], "query_in": "body"}},
                "delete": {"operationId": "m.bare"}
            }
        }});
        let catalogue = Catalogue::from_json(&spec);
        let class = |id: &str| Class::of(catalogue.get(id).unwrap());
        assert_eq!(class("m.r"), Class::Read);
        assert_eq!(class("m.w"), Class::Write);
        assert_eq!(class("m.s"), Class::Send);
        assert_eq!(class("m.q"), Class::Read, "a read served by a body");
        assert_eq!(class("m.bare"), Class::Write, "no metadata: by method");
    }

    #[test]
    fn a_class_at_zero_or_everything_disabled_is_off() {
        let defaults = ApiLimits::default();
        assert_eq!(
            defaults.limit(Class::Read),
            Some((600, Duration::from_mins(1)))
        );
        assert_eq!(
            defaults.limit(Class::Write),
            Some((120, Duration::from_mins(1)))
        );
        assert_eq!(
            defaults.limit(Class::Send),
            Some((30, Duration::from_mins(10)))
        );
        assert_eq!(
            defaults.limit(Class::Admin),
            Some((120, Duration::from_mins(1)))
        );
        let writes_off = ApiLimits::new(&RateLimitsConfig {
            writes: 0,
            ..RateLimitsConfig::default()
        });
        assert_eq!(writes_off.limit(Class::Write), None);
        assert!(writes_off.limit(Class::Read).is_some());
        let off = ApiLimits::new(&RateLimitsConfig::off());
        for class in [Class::Read, Class::Write, Class::Send, Class::Admin] {
            assert_eq!(off.limit(class), None, "{}", class.as_str());
        }
    }
}
