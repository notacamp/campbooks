//! The request log (`campbooks_server::telemetry::observe`): one line per
//! request, named after the route template, and a web BFF request after the
//! API route it ran through in-process (`served_route`). Its own test binary:
//! the log capture is a thread-local subscriber, which other tests registering
//! the same callsites at the same time can starve.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{
    io::Write,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, Uri},
    middleware,
    routing::get,
};
use campbooks_kernel::config::TelemetryConfig;
use campbooks_server::telemetry::{self, Telemetry};
use serde_json::Value;
use tower::ServiceExt as _;
use tracing_subscriber::prelude::*;

const MEMBERS: &str = "/api/v1/w/{workspace_id}/members";
const WORKSPACE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";

/// Log lines written by the fmt layer.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    /// The `request finished` lines, in order.
    fn finished(&self) -> Vec<Value> {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|line| line["message"] == "request finished")
            .collect()
    }
}

impl Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// An API with one route, reached directly and, as the BFF reaches it, by a
/// catch-all route that runs the request through it in-process.
fn app(metrics: campbooks_kernel::telemetry::Metrics) -> Router {
    let api: Router = Router::new()
        .route(MEMBERS, get(|| async { "members" }))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .route_layer(middleware::from_fn(telemetry::served_route));
    let forward = api.clone();
    Router::new()
        .route(MEMBERS, get(|| async { "members" }))
        .route(
            "/bff/api/{*path}",
            get(move |mut request: Request| {
                let api = forward.clone();
                async move {
                    let path = request
                        .uri()
                        .path()
                        .strip_prefix("/bff")
                        .unwrap()
                        .to_owned();
                    *request.uri_mut() = Uri::try_from(path).unwrap();
                    request
                        .extensions_mut()
                        .remove::<axum::extract::MatchedPath>();
                    api.oneshot(request).await.unwrap()
                }
            }),
        )
        .layer(middleware::from_fn_with_state(metrics, telemetry::observe))
}

#[tokio::test]
async fn requests_through_the_bff_are_logged_under_the_api_route() {
    let telemetry = Telemetry::new(&TelemetryConfig::default(), "all").unwrap();
    let logs = Logs::default();
    let writer = logs.clone();
    let _guard = tracing::subscriber::set_default(
        tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .flatten_event(true)
                .with_writer(move || writer.clone()),
        ),
    );
    let app = app(telemetry.metrics());
    for uri in [
        format!("/bff/api/v1/w/{WORKSPACE}/members?q=boilerquote"),
        "/bff/api/v1/nope".to_owned(),
        format!("/api/v1/w/{WORKSPACE}/members"),
    ] {
        let request = Request::get(uri).body(Body::empty()).unwrap();
        app.clone().oneshot(request).await.unwrap();
    }

    let finished = logs.finished();
    let [through_bff, unmatched, direct] = finished.as_slice() else {
        panic!("three request lines: {finished:#?}");
    };
    // Named after the API route it ran through, marked as come through the BFF.
    let span = &through_bff["span"];
    assert_eq!(span["http.route"], MEMBERS, "{through_bff}");
    assert_eq!(span["otel.name"], format!("GET {MEMBERS}"));
    assert_eq!(span["campbooks.via"], "bff");
    assert_eq!(through_bff["status"], 200);
    assert!(through_bff["latency_ms"].is_u64(), "{through_bff}");
    // Neither the path nor the query string is logged (the workspace id is
    // logged only when the route's extractor records it, as an id).
    let line = through_bff.to_string();
    assert!(!line.contains(&format!("/w/{WORKSPACE}")), "{line}");
    assert!(!line.contains("boilerquote"), "{line}");
    // No API route matched: the BFF's own route.
    assert_eq!(
        unmatched["span"]["http.route"], "/bff/api/{*path}",
        "{unmatched}"
    );
    assert!(
        unmatched["span"].get("campbooks.via").is_none(),
        "{unmatched}"
    );
    assert_eq!(unmatched["status"], 404);
    // Reached directly: not marked.
    assert_eq!(direct["span"]["http.route"], MEMBERS);
    assert!(direct["span"].get("campbooks.via").is_none(), "{direct}");
}
