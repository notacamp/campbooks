//! Telemetry through the production pipeline (`campbooks_server::telemetry`):
//! what a representative request and the job it causes put in spans and log
//! lines, and what the Prometheus page shows.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{
    io::Write,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use campbooks_kernel::{
    AppState,
    blobs::BlobStore,
    config::{BlobsConfig, LogFormat, TelemetryConfig},
    jobs::{self, EnqueueOptions, PruneJobs, Worker},
    testing::{self, TestIdp, TestResponse},
};
use campbooks_server::{registry, router, telemetry, with_ports};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_subscriber::{fmt::MakeWriter, prelude::*};

const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
const ALICE: &str = "alice@example.com";
const BOB: &str = "bob@example.com";
const COOKIE_SECRET: &str = "cookie-secret-3f9a";

/// Log lines written by the fmt layer.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    fn lines(&self) -> Vec<serde_json::Value> {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
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

impl<'a> MakeWriter<'a> for Logs {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> TestResponse {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::COOKIE, format!("session={COOKIE_SECRET}"))
        // A client-chosen request id that would leak an address if logged.
        .header("x-request-id", ALICE)
        .header("traceparent", TRACEPARENT);
    let body = match body {
        Some(json) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    testing::send(app, request.body(body).unwrap()).await
}

fn attribute(span: &SpanData, key: &str) -> Option<String> {
    span.attributes
        .iter()
        .find(|kv| kv.key.as_str() == key)
        .map(|kv| kv.value.to_string())
}

fn spans_named<'a>(spans: &'a [SpanData], prefix: &str) -> Vec<&'a SpanData> {
    spans
        .iter()
        .filter(|span| span.name.starts_with(prefix))
        .collect()
}

/// A request carrying a bearer token, a session cookie, an email address in
/// its query string and request id and an email claim, and the job it
/// enqueues: no secret or address reaches a span or a log line, and the
/// useful fields do.
#[sqlx::test(migrations = "../../migrations")]
async fn requests_and_jobs_carry_no_personal_data_or_secrets(db: PgPool) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let logs = Logs::default();
    let subscriber = tracing_subscriber::registry().with(telemetry::layers(
        LogFormat::Json,
        logs.clone(),
        provider.tracer("test"),
        true,
        None,
    ));
    let _guard = tracing::subscriber::set_default(subscriber);

    let idp = TestIdp::start().await;
    let pool = testing::app_role_pool(&db).await;
    let state = with_ports(
        idp.state(pool.clone())
            .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
            .with_registry(registry().unwrap()),
    );
    let app = router(state.clone());
    let mut claims = idp.claims("workspaces:read workspaces:write workspaces:admin");
    claims["email"] = ALICE.into();
    claims["email_verified"] = true.into();
    claims["name"] = "Alice Example".into();
    let token = idp.token(&claims);

    let created = send(
        &app,
        Method::POST,
        &format!("/api/v1/workspaces?email={ALICE}"),
        &token,
        Some(serde_json::json!({ "name": "Acme", "kind": "business" })),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let workspace = created.json["id"].as_str().unwrap().to_owned();

    let invited = send(
        &app,
        Method::POST,
        &format!("/api/v1/w/{workspace}/invitations"),
        &token,
        Some(serde_json::json!({ "email": BOB })),
    )
    .await;
    assert_eq!(invited.status, StatusCode::CREATED, "{:?}", invited.json);

    let deleted = send(
        &app,
        Method::DELETE,
        &format!("/api/v1/w/{workspace}"),
        &token,
        None,
    )
    .await;
    assert!(deleted.status.is_success(), "{:?}", deleted.json);
    // The purge waits for its grace period; end it now.
    sqlx::query!("UPDATE workspaces.workspaces SET purge_after = now()")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query!("UPDATE kernel.jobs SET run_at = now() WHERE state = 'pending'")
        .execute(&db)
        .await
        .unwrap();
    let stats = Worker::new(state).run_until_idle().await;
    assert!(stats.succeeded >= 1, "{stats:?}");

    let spans = exporter.get_finished_spans().unwrap();
    let lines = logs.lines();

    // Nothing secret or personal, anywhere.
    let everything = format!("{spans:?}\n{}", serde_json::to_string(&lines).unwrap());
    for secret in [token.as_str(), COOKIE_SECRET, ALICE, BOB, "Alice Example"] {
        assert!(
            !everything.contains(secret),
            "telemetry contains {secret:?}:\n{everything}"
        );
    }

    // Request spans: named by route template, with what an operator needs.
    let invite = spans_named(&spans, "POST /api/v1/w/{workspace_id}/invitations");
    let [invite] = invite.as_slice() else {
        panic!("one invite span: {spans:#?}");
    };
    assert_eq!(
        attribute(invite, "http.route").as_deref(),
        Some("/api/v1/w/{workspace_id}/invitations")
    );
    assert_eq!(
        attribute(invite, "http.response.status_code").as_deref(),
        Some("201")
    );
    assert_eq!(
        attribute(invite, "workspace.id").as_deref(),
        Some(workspace.as_str())
    );
    assert_eq!(attribute(invite, "principal.kind").as_deref(), Some("user"));
    assert_eq!(attribute(invite, "request_id").as_deref(), Some("invalid"));
    assert_eq!(invite.span_context.trace_id().to_string(), TRACE_ID);
    assert_eq!(
        invite.parent_span_id.to_string(),
        "00f067aa0ba902b7",
        "the incoming traceparent is the parent"
    );

    // The IdP fetch is a client span and the IdP got the trace.
    assert!(!spans_named(&spans, "GET idp").is_empty(), "{spans:#?}");
    let sent = idp
        .last_traceparent()
        .expect("the IdP received a traceparent");
    assert!(sent.contains(TRACE_ID), "{sent}");

    // The purge job links to the request that enqueued it.
    let purge = spans_named(&spans, "job workspaces.purge_workspace");
    let [purge] = purge.as_slice() else {
        panic!("one purge span: {spans:#?}");
    };
    assert_eq!(
        attribute(purge, "job.outcome").as_deref(),
        Some("succeeded")
    );
    assert_eq!(
        attribute(purge, "workspace.id").as_deref(),
        Some(workspace.as_str())
    );
    let delete = spans_named(&spans, "DELETE /api/v1/w/{workspace_id}");
    let [delete] = delete.as_slice() else {
        panic!("one delete span: {spans:#?}");
    };
    assert!(
        purge
            .links
            .iter()
            .any(|link| link.span_context.span_id() == delete.span_context.span_id()),
        "the job links to the DELETE request: {:?}",
        purge.links
    );

    // Log lines inside the request carry its trace id, and the request line
    // names the route, not the URI.
    let finished: Vec<_> = lines
        .iter()
        .filter(|line| line["message"] == "request finished")
        .collect();
    assert_eq!(finished.len(), 3, "{lines:#?}");
    for line in finished {
        assert_eq!(line["span"]["trace_id"], TRACE_ID, "{line}");
        assert!(line["span"]["http.route"].is_string(), "{line}");
        assert!(line["span"].get("uri").is_none(), "{line}");
    }
}

async fn scrape(page: Router) -> String {
    let response =
        tower::ServiceExt::oneshot(page, Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

/// The one series of `metric` whose labels include every one of `labels`.
fn series<'a>(text: &'a str, metric: &str, labels: &[&str]) -> &'a str {
    let matching: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with(&format!("{metric}{{")))
        .filter(|line| labels.iter().all(|label| line.contains(label)))
        .collect();
    let [line] = matching.as_slice() else {
        panic!("one {metric} series with {labels:?}, got {matching:?}:\n{text}");
    };
    line
}

/// The Prometheus page: requests by route template (two workspaces, one
/// series, no ids), jobs by kind and outcome, queue depth, the pool, build
/// info.
#[sqlx::test(migrations = "../../migrations")]
async fn the_metrics_page_has_low_cardinality_series(db: PgPool) {
    let config = TelemetryConfig {
        metrics_addr: Some(([127, 0, 0, 1], 0).into()),
        ..TelemetryConfig::default()
    };
    let telemetry = telemetry::Telemetry::new(&config, "all").unwrap();
    let subscriber = tracing_subscriber::registry().with(telemetry::layers(
        LogFormat::Json,
        std::io::sink,
        telemetry.tracer(),
        false,
        telemetry.pool_wait(),
    ));
    let _guard = tracing::subscriber::set_default(subscriber);
    let pool = campbooks_kernel::db::with_runtime_role(
        PgPoolOptions::new()
            .max_connections(5)
            .acquire_time_level(log::LevelFilter::Debug),
        Some(campbooks_kernel::db::DEFAULT_RUNTIME_ROLE),
    )
    .connect_with((*db.connect_options()).clone())
    .await
    .unwrap();
    telemetry.observe_pool(&pool, 5);
    let state = with_ports(
        AppState::new(
            pool.clone(),
            testing::config("postgres://test@localhost/test"),
        )
        .with_metrics(telemetry.metrics())
        .with_registry(registry().unwrap()),
    );

    let app = router(state.clone());
    let ids = [
        "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
        "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c",
    ];
    for id in ids {
        let response = testing::get(&app, &format!("/api/v1/w/{id}/members")).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        testing::get(&app, "/metrics").await.status,
        StatusCode::NOT_FOUND,
        "/metrics is not on the API listener"
    );

    let mut tx = pool.begin().await.unwrap();
    jobs::enqueue(&mut tx, &PruneJobs, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stats = Worker::new(state)
        .run(tokio::time::sleep(Duration::from_millis(500)))
        .await;
    assert!(stats.succeeded >= 1, "{stats:?}");

    let text = scrape(telemetry.metrics_router().unwrap()).await;
    let requests = series(
        &text,
        "http_server_request_duration_seconds_count",
        &[r#"http_route="/api/v1/w/{workspace_id}/members""#],
    );
    assert!(
        requests.contains(r#"http_response_status_code="401""#),
        "{requests}"
    );
    assert!(
        requests.contains(r#"http_request_method="GET""#),
        "{requests}"
    );
    assert!(requests.ends_with(" 2"), "{requests}");
    for id in ids {
        assert!(!text.contains(id), "{text}");
    }
    let kind = r#"job_kind="kernel.prune_jobs""#;
    series(
        &text,
        "campbooks_job_duration_seconds_count",
        &[kind, r#"job_outcome="succeeded""#],
    );
    series(&text, "campbooks_job_wait_seconds_count", &[kind]);
    let queue = r#"job_queue="maintenance""#;
    for state in ["ready", "scheduled", "running", "failed"] {
        series(
            &text,
            "campbooks_jobs",
            &[queue, &format!(r#"job_state="{state}""#)],
        );
    }
    series(&text, "campbooks_jobs_oldest_ready_age_seconds", &[queue]);
    series(
        &text,
        "db_client_connection_count",
        &[r#"db_client_connection_state="idle""#],
    );
    series(&text, "db_client_connection_wait_time_seconds_count", &[]);
    series(&text, "campbooks_build_info", &[r#"campbooks_role="all""#]);
    telemetry.shutdown();
}
