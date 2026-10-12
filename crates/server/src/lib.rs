//! `campbooks-server` — composes the modules into one HTTP API.
//!
//! This is the only crate that knows the full list of modules. Adding a module
//! means adding one line to [`modules`].

#![forbid(unsafe_code)]

use std::time::Duration;

use axum::{
    Json, Router,
    http::{HeaderName, Request, StatusCode},
    routing::get,
};
use campbooks_kernel::{AppState, Error, Module, Problem};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use utoipa::OpenApi as _;
use utoipa_axum::router::OpenApiRouter;

/// Longest a request may take before the server answers 503.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Path the OpenAPI document is served at.
pub const OPENAPI_PATH: &str = "/openapi.json";

/// Header carrying the request id, generated when the client sends none.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Every module, in mount order.
pub fn modules() -> Vec<Module> {
    vec![campbooks_system::module()]
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Campbooks API",
        description = "The Campbooks HTTP API. Errors are RFC 9457 `application/problem+json`.",
        license(name = "Sustainable Use License", identifier = "LicenseRef-Sustainable-Use"),
    ),
    components(schemas(Problem)),
    tags((name = "system", description = "Instance health and version.")),
)]
struct ApiDoc;

fn api() -> OpenApiRouter<AppState> {
    let mut doc = ApiDoc::openapi();
    env!("CARGO_PKG_VERSION").clone_into(&mut doc.info.version);
    modules()
        .into_iter()
        .fold(OpenApiRouter::with_openapi(doc), |api, module| {
            api.merge(module.router)
        })
}

/// The OpenAPI document for every module's routes.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api().into_openapi()
}

/// The full HTTP application: module routes, `/openapi.json`, a problem+json
/// 404 fallback, and the middleware stack (request id, tracing, timeout,
/// compression).
pub fn router(state: AppState) -> Router {
    let (routes, spec) = api().split_for_parts();
    let spec = std::sync::Arc::new(spec);
    let request_id = HeaderName::from_static(REQUEST_ID_HEADER);

    routes
        .route(
            OPENAPI_PATH,
            get(move || async move { Json(spec.as_ref().clone()) }),
        )
        .fallback(|| async { Error::NotFound })
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<_>| {
                            let id = request
                                .extensions()
                                .get::<RequestId>()
                                .and_then(|id| id.header_value().to_str().ok())
                                .unwrap_or_default();
                            tracing::info_span!(
                                "http",
                                method = %request.method(),
                                uri = %request.uri(),
                                request_id = %id,
                            )
                        })
                        // One INFO line per request (status + latency).
                        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
                )
                .layer(PropagateRequestIdLayer::new(request_id))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::SERVICE_UNAVAILABLE,
                    REQUEST_TIMEOUT,
                ))
                .layer(CompressionLayer::new()),
        )
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use campbooks_kernel::testing;
    use tower::ServiceExt;

    use super::*;

    fn app() -> Router {
        router(testing::state(testing::unreachable_pool()))
    }

    #[test]
    fn openapi_lists_every_module_route() {
        let spec = serde_json::to_value(openapi()).unwrap();
        assert_eq!(spec["info"]["title"], "Campbooks API");
        assert_eq!(spec["info"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            spec["paths"]["/api/v1/system/health"]["get"]["operationId"],
            "system_health"
        );
        assert_eq!(
            spec["paths"]["/api/v1/system/version"]["get"]["operationId"],
            "system_version"
        );
        assert!(spec["components"]["schemas"]["Problem"].is_object());
    }

    #[test]
    fn module_names_are_unique() {
        let mut names: Vec<_> = modules().iter().map(|m| m.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[tokio::test]
    async fn serves_openapi_json() {
        let response = testing::get(&app(), OPENAPI_PATH).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["info"]["title"], "Campbooks API");
    }

    #[tokio::test]
    async fn unknown_routes_are_problem_json_404() {
        let response = testing::get(&app(), "/api/v1/nope").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/problem+json")
        );
    }

    #[tokio::test]
    async fn propagates_or_generates_request_ids() {
        let request = Request::get("/api/v1/system/version")
            .header(REQUEST_ID_HEADER, "abc-123")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.headers()[REQUEST_ID_HEADER], "abc-123");

        let request = Request::get("/api/v1/system/version")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert!(!response.headers()[REQUEST_ID_HEADER].is_empty());
    }
}
