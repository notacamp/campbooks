//! The request time limit (campbooks-cloud#63, #288).
//!
//! Every request gets a [`Deadline`]: [`DEFAULT`] after it arrived unless its
//! handler raises it for itself with [`Deadline::allow`], never above
//! [`MAX`]. A request still running at its deadline is stopped and answered
//! 503 problem+json `request_timed_out`.
//!
//! The limit covers the response head only: a body that streams after the
//! head (Server-Sent Events) is not cut. The BFF runs the API in process and
//! keeps the request's extensions, so a handler behind it raises the same
//! deadline the outer request is held to.
//!
//! A mutating request with an `Idempotency-Key` keeps its key when it is
//! stopped: the request may have committed before it was stopped, so the key
//! is not released, and retrying with it answers 409 (in flight, then
//! `idempotency_outcome_unknown`); check the result, then retry with a new
//! key. [`MAX`] stays below the idempotency layer's abandonment time, so a
//! request is always stopped before its key could count as abandoned.

use std::{
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio::time::Instant;

use crate::{Error, i18n::MESSAGES};

/// How long a request may run when its handler asks for nothing more.
pub const DEFAULT: Duration = Duration::from_secs(30);

/// The longest a handler may allow itself.
pub const MAX: Duration = Duration::from_secs(100);

// A request is stopped before its idempotency key could count as abandoned,
// so a running request is never taken for a dead one.
const _: () = assert!(MAX.as_secs() < crate::idempotency::ABANDONED_AFTER.as_secs());

/// When the running request is stopped. Cheap to clone: every clone is the
/// same deadline.
#[derive(Debug, Clone)]
pub struct Deadline(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    /// When the request arrived.
    started: Instant,
    /// Its limit from `started`, in milliseconds.
    limit_ms: AtomicU64,
}

impl Deadline {
    /// A deadline `limit` from now (at most [`MAX`]).
    pub fn new(limit: Duration) -> Self {
        Self(Arc::new(Inner {
            started: Instant::now(),
            limit_ms: AtomicU64::new(millis(limit.min(MAX))),
        }))
    }

    /// Let the request run `total` from its arrival, at most [`MAX`]. Never
    /// shortens it: a limit already longer stays.
    pub fn allow(&self, total: Duration) {
        self.0
            .limit_ms
            .fetch_max(millis(total.min(MAX)), Ordering::Relaxed);
    }

    /// The request's limit, from its arrival.
    pub fn limit(&self) -> Duration {
        Duration::from_millis(self.0.limit_ms.load(Ordering::Relaxed))
    }

    /// When the request is stopped.
    fn at(&self) -> Instant {
        self.0.started + self.limit()
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// The middleware: give the request a [`Deadline`] of `limit` (or keep the
/// one it carries) and stop it there. Install it inside the localizing layer,
/// so its problem is in the caller's language.
pub async fn middleware(
    State(limit): State<Duration>,
    mut request: Request,
    next: Next,
) -> Response {
    let deadline = if let Some(deadline) = request.extensions().get::<Deadline>() {
        deadline.clone()
    } else {
        let deadline = Deadline::new(limit);
        request.extensions_mut().insert(deadline.clone());
        deadline
    };
    let mut running = pin!(next.run(request));
    loop {
        let at = deadline.at();
        tokio::select! {
            response = &mut running => return response,
            () = tokio::time::sleep_until(at) => {
                // The handler may have allowed itself longer meanwhile.
                if Instant::now() >= deadline.at() {
                    return Error::Unavailable(MESSAGES.reason("request_timed_out"))
                        .into_response();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Extension, Router,
        body::Body,
        http::{Request, StatusCode, header},
        routing::get,
    };
    use tower::ServiceBuilder;

    use super::*;
    use crate::testing;

    /// `/slow` sleeps 5 s; `/allowed` raises its limit to 5 s and answers
    /// after 120 ms; `/unallowed` answers after 120 ms; all under `limit`.
    fn app(limit: Duration) -> Router {
        let state = testing::state(testing::unreachable_pool());
        let localize = axum::middleware::from_fn_with_state(state, crate::i18n::localize);
        Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    "done"
                }),
            )
            .route(
                "/allowed",
                get(|Extension(deadline): Extension<Deadline>| async move {
                    deadline.allow(Duration::from_secs(5));
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    "done"
                }),
            )
            .route(
                "/unallowed",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    "done"
                }),
            )
            .layer(
                ServiceBuilder::new()
                    .layer(localize)
                    .layer(axum::middleware::from_fn_with_state(limit, middleware)),
            )
    }

    /// A request still running past its limit is a 503 problem
    /// `request_timed_out`, in the caller's language (campbooks-cloud#63).
    #[tokio::test]
    async fn a_request_past_its_time_limit_is_a_503_problem() {
        let request = Request::get("/slow")
            .header(header::ACCEPT_LANGUAGE, "pt-PT")
            .body(Body::empty())
            .unwrap();
        let response = testing::send(&app(Duration::from_millis(50)), request).await;
        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/problem+json")
        );
        assert_eq!(response.json["code"], "request_timed_out");
        assert!(
            response.json["detail"]
                .as_str()
                .unwrap()
                .contains("Idempotency-Key"),
            "{}",
            response.json
        );
    }

    /// A handler that allows itself longer finishes; one that does not is
    /// stopped at the default (campbooks-cloud#288).
    #[tokio::test]
    async fn a_handler_may_allow_itself_longer() {
        let app = app(Duration::from_millis(50));
        let response = testing::get(&app, "/allowed").await;
        assert_eq!(response.status, StatusCode::OK);
        let response = testing::get(&app, "/unallowed").await;
        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.json["code"], "request_timed_out");
    }

    #[test]
    fn a_deadline_is_only_ever_raised_and_never_past_the_maximum() {
        let deadline = Deadline::new(DEFAULT);
        deadline.allow(Duration::from_secs(10));
        assert_eq!(deadline.limit(), DEFAULT, "never shortened");
        deadline.allow(Duration::from_secs(90));
        assert_eq!(deadline.limit(), Duration::from_secs(90));
        let clone = deadline.clone();
        clone.allow(Duration::from_hours(1));
        assert_eq!(deadline.limit(), MAX, "one deadline, capped");
        assert_eq!(Deadline::new(Duration::from_hours(1)).limit(), MAX);
    }
}
