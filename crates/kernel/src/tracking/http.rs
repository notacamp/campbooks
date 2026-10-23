//! The HTTP layer that opens a request's [`TrackingContext`]: the matched
//! operation, the request's correlation, and an actor that the extractors
//! fill in once they authenticate the caller.
//!
//! [`operations::compose`](crate::operations::compose) puts it around every
//! catalogued route (so an operation invoked by id gets one too), the
//! server around its admin, BFF and issuer routes, and
//! [`testing::app`](crate::testing::app) around a module's routes.

use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};

use super::context::{self, Actor, TrackingContext};
use crate::{
    auth::AuthenticatedPrincipal,
    operations::{Caller, Catalogue, InvokedBy},
};

/// What [`middleware`] needs: the catalogue that names each route's
/// operation.
#[derive(Debug, Clone)]
pub struct HttpTracking {
    catalogue: Arc<Catalogue>,
}

impl HttpTracking {
    /// Name operations from `catalogue` (the composed OpenAPI document's, or
    /// the admin API's).
    pub fn new(catalogue: Arc<Catalogue>) -> Self {
        Self { catalogue }
    }

    /// Name operations from `spec`.
    pub fn from_spec(spec: &utoipa::openapi::OpenApi) -> Self {
        Self::new(Arc::new(Catalogue::from_openapi(spec)))
    }
}

/// `router` with every route in its request's tracking context
/// ([`middleware`]); a router with no routes is returned as it is.
pub fn around<S>(router: axum::Router<S>, tracking: HttpTracking) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if router.has_routes() {
        router.route_layer(axum::middleware::from_fn_with_state(tracking, middleware))
    } else {
        router
    }
}

/// Run the request in its tracking context.
///
/// - The operation is the one the matched route serves, if catalogued.
/// - The correlation is the enclosing context's (an operation invoked from
///   a job or another request), else the request's trace id, else a new one.
/// - The actor is `anonymous` until [`Scoped`](crate::auth::Scoped) or
///   [`Admin`](crate::admin::Admin) names the caller; an operation invoked
///   by the assistant or an automation is theirs, on behalf of the principal
///   it runs for.
pub async fn middleware(
    State(layer): State<HttpTracking>,
    request: Request,
    next: Next,
) -> Response {
    let operation = request
        .extensions()
        .get::<MatchedPath>()
        .and_then(|path| layer.catalogue.route(request.method(), path.as_str()))
        .map(|operation| operation.id.clone());
    let enclosing = context::current();
    let principal = request
        .extensions()
        .get::<AuthenticatedPrincipal>()
        .map(|AuthenticatedPrincipal(principal)| Actor::of(principal));
    let (actor, on_behalf_of) = match request.extensions().get::<InvokedBy>() {
        Some(InvokedBy(Caller::Assistant)) => (Actor::assistant(), principal),
        Some(InvokedBy(Caller::Automation)) => (Actor::automation(), principal),
        None => (Actor::anonymous(), None),
    };
    let correlation = enclosing
        .as_ref()
        .map_or_else(context::new_correlation, |outer| outer.correlation.clone());
    let tracking = TrackingContext {
        actor,
        on_behalf_of,
        operation,
        correlation,
        causation: enclosing.and_then(|outer| outer.causation),
    };
    context::scope(tracking, next.run(request)).await
}
