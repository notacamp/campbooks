//! Helpers for module tests. Enabled by the `test-util` feature; modules add
//! `campbooks-kernel = { workspace = true, features = ["test-util"] }` under
//! `[dev-dependencies]`.

use std::time::Duration;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use secrecy::SecretString;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

use crate::{AppState, Config, Module, config::LogFormat};

/// A config pointing at `database_url`, everything else default.
pub fn config(database_url: &str) -> Config {
    Config {
        database_url: SecretString::from(database_url),
        database_max_connections: 5,
        bind_addr: ([127, 0, 0, 1], 0).into(),
        public_url: "http://localhost:8080".to_owned(),
        run_migrations: false,
        log_format: LogFormat::Pretty,
    }
}

/// App state over `db` (e.g. the pool `#[sqlx::test]` hands you).
pub fn state(db: PgPool) -> AppState {
    AppState::new(db, config("postgres://test@localhost/test"))
}

/// A pool whose every acquire fails quickly: for "database is down" paths.
pub fn unreachable_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(300))
        // Port 1 on loopback: nothing listens, so connects are refused at once.
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:1/none")
        .unwrap_or_else(|e| unreachable!("static URL parses: {e}"))
}

/// A module's routes as a plain router over `state`.
pub fn app(module: Module, state: AppState) -> Router {
    module.router.with_state(state).into()
}

/// A response, read to the end.
#[derive(Debug)]
pub struct TestResponse {
    /// Status code.
    pub status: StatusCode,
    /// `Content-Type` header, if any.
    pub content_type: Option<String>,
    /// Body parsed as JSON (`Null` when empty or not JSON).
    pub json: serde_json::Value,
}

/// `GET uri` against `app`.
pub async fn get(app: &Router, uri: &str) -> TestResponse {
    let request = Request::get(uri)
        .body(Body::empty())
        .unwrap_or_else(|e| unreachable!("GET request builds: {e}"));
    send(app, request).await
}

/// Send any request through `app` in-process.
pub async fn send(app: &Router, request: Request<Body>) -> TestResponse {
    let response = match app.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response
        .into_body()
        .collect()
        .await
        .map(http_body_util::Collected::to_bytes)
        .unwrap_or_default();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    TestResponse {
        status,
        content_type,
        json,
    }
}
