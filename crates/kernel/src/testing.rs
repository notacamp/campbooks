//! Helpers for module tests. Enabled by the `test-util` feature; modules add
//! `campbooks-kernel = { workspace = true, features = ["test-util"] }` under
//! `[dev-dependencies]`. Scoped endpoints are tested against a [`TestIdp`].

mod idp;

pub use idp::{TestIdp, TestUser, WEB_CLIENT_ID, WEB_CLIENT_SECRET};

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

use crate::{
    AppState, Config, Module,
    blobs::BlobStore,
    config::{
        AuthConfig, BlobsConfig, ChangesConfig, Environment, JobsConfig, LogFormat, SignupConfig,
        TelemetryConfig,
    },
    db,
};

/// A config pointing at `database_url`, everything else default (no
/// identity provider: scoped endpoints answer 401 `auth_not_configured`).
pub fn config(database_url: &str) -> Config {
    Config {
        database_url: SecretString::from(database_url),
        database_migration_url: None,
        database_password_file: None,
        database_allow_rls_bypass: false,
        database_max_connections: 5,
        database_role: db::DEFAULT_RUNTIME_ROLE.to_owned(),
        env: Environment::Production,
        bind_addr: ([127, 0, 0, 1], 0).into(),
        public_url: "http://localhost:8080".to_owned(),
        run_migrations: false,
        log_format: LogFormat::Pretty,
        web_dir: None,
        auth: AuthConfig::default(),
        jobs: JobsConfig::default(),
        signup: SignupConfig::default(),
        blobs: BlobsConfig::default(),
        changes: ChangesConfig::default(),
        mail: crate::mail::MailConfig::default(),
        events: crate::config::EventsConfig::default(),
        telemetry: TelemetryConfig::default(),
        trusted_proxies: crate::rate_limit::TrustedProxies::default(),
        rate_limits: crate::config::RateLimitsConfig::default(),
        analytics: crate::config::AnalyticsConfig::default(),
        tracking: crate::config::TrackingConfig::default(),
    }
}

/// A pool on the same database as `db` (e.g. the one `#[sqlx::test]` hands
/// you, which connects as a superuser) whose connections run as the runtime
/// role, like the server's: row-level security applies.
///
/// # Panics
/// If the database refuses the connection or the role switch (the kernel
/// migration, which creates the role, has not run).
pub async fn app_role_pool(db: &PgPool) -> PgPool {
    let options = db::with_runtime_role(
        PgPoolOptions::new().max_connections(5),
        Some(db::DEFAULT_RUNTIME_ROLE),
    );
    options
        .connect_with(db::with_session_limits((*db.connect_options()).clone()))
        .await
        .unwrap_or_else(|e| panic!("connecting as the runtime role: {e}"))
}

/// Let workspace transactions add the instance-wide rows `registry` derives
/// ([`Registry::instance_rows_from_workspaces`](crate::jobs::Registry::instance_rows_from_workspaces)):
/// the test's own events and consumers, which no migration lists. `db` is
/// the owner's pool (the runtime role may only read the list).
///
/// # Panics
/// If the database refuses the insert.
pub async fn allow_instance_rows(db: &PgPool, registry: &crate::jobs::Registry) {
    let rows = registry.instance_rows_from_workspaces();
    sqlx::query(
        "INSERT INTO kernel.instance_rows_from_workspaces (row_kind, name)
         SELECT 'event', unnest($1::text[])
         UNION ALL SELECT 'job', unnest($2::text[])
         ON CONFLICT DO NOTHING",
    )
    .bind(&rows.events)
    .bind(&rows.kinds)
    .execute(db)
    .await
    .unwrap_or_else(|e| panic!("listing the test's instance rows: {e}"));
}

/// A pool on the same database as `db` whose connections run as the
/// runtime role **in the kernel scope** for the whole session: for tests
/// that inspect or adjust the kernel's tables (the event log, the queue)
/// across workspaces, as the kernel's own machinery does. A transaction
/// that sets nothing sees none of their rows (campbooks-cloud#129).
///
/// # Panics
/// If the database refuses the connection or the role switch.
pub async fn kernel_scope_pool(db: &PgPool) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SELECT set_config('role', $1, false), set_config($2, 'on', false)")
                    .bind(db::DEFAULT_RUNTIME_ROLE)
                    .bind(db::KERNEL_SCOPE_SETTING)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*db.connect_options()).clone())
        .await
        .unwrap_or_else(|e| panic!("connecting in the kernel scope: {e}"))
}

/// The name of the kernel scope's setting, for tests that inspect the
/// policies (the constant itself is the kernel's own).
pub fn kernel_scope_setting() -> &'static str {
    db::KERNEL_SCOPE_SETTING
}

/// The name of the maintenance scope's setting, for tests that inspect the
/// policies (the constant itself is the kernel's own).
pub fn maintenance_scope_setting() -> &'static str {
    db::MAINTENANCE_SCOPE_SETTING
}

/// The name of the setting naming the idempotency layer's caller, for tests
/// that inspect the policies (the constant itself is the kernel's own).
pub fn idempotency_caller_setting() -> &'static str {
    db::IDEMPOTENCY_CALLER_SETTING
}

/// Run `sql` on `db`, panicking on failure (test setup).
async fn exec(db: &PgPool, sql: String) {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(db)
        .await
        .unwrap_or_else(|e| panic!("test setup failed: {e}"));
}

const TEST_LOGIN_PASSWORD: &str = "test-login-password";

/// The name of the test login for `db`'s database: roles belong to the
/// cluster, so each `#[sqlx::test]` database gets its own.
async fn test_login_name(db: &PgPool) -> String {
    let database = sqlx::query_scalar!(r#"SELECT current_database()::text AS "name!""#)
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| panic!("reading the database name: {e}"));
    let safe: String = database
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("cb_login{safe}")
}

/// A pool on `db`'s database logged in as a **dedicated runtime login**, as
/// a split deployment configures: `LOGIN`, not a superuser, no `BYPASSRLS`,
/// a member of the runtime role, owning nothing. Unlike [`app_role_pool`]
/// no `SET ROLE` is involved, so there is nothing to `RESET ROLE` back to.
/// Pair with [`drop_runtime_login`] at the end of the test.
///
/// # Panics
/// If `db`'s user cannot create roles or the kernel migration has not run.
pub async fn runtime_login_pool(db: &PgPool) -> PgPool {
    let name = create_login(db).await;
    exec(db, format!("GRANT {} TO {name}", db::DEFAULT_RUNTIME_ROLE)).await;
    login_pool(db, &name).await
}

/// A pool logged in as the existing role `name` (password
/// [`runtime_login_pool`] uses), for tests that shape a role themselves.
///
/// # Panics
/// If the login is refused.
pub async fn login_pool(db: &PgPool, name: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .connect_with(
            (*db.connect_options())
                .clone()
                .username(name)
                .password(TEST_LOGIN_PASSWORD),
        )
        .await
        .unwrap_or_else(|e| panic!("connecting as {name}: {e}"))
}

/// Create a role that can sign in with the test password, for
/// [`login_pool`]; returns its name. Drop it with [`drop_runtime_login`].
///
/// # Panics
/// If `db`'s user cannot create roles.
pub async fn create_login(db: &PgPool) -> String {
    let name = test_login_name(db).await;
    exec(
        db,
        format!(
            "CREATE ROLE {name} LOGIN PASSWORD '{TEST_LOGIN_PASSWORD}' NOSUPERUSER NOBYPASSRLS"
        ),
    )
    .await;
    name
}

/// Drop the login of [`runtime_login_pool`] / [`create_login`] (and what it
/// owns or was granted), once its pool is closed.
///
/// # Panics
/// If the role cannot be dropped.
pub async fn drop_runtime_login(db: &PgPool) {
    let name = test_login_name(db).await;
    exec(db, format!("DROP OWNED BY {name}")).await;
    exec(db, format!("DROP ROLE {name}")).await;
}

/// App state over `db` (e.g. the pool `#[sqlx::test]` hands you), storing
/// blobs in memory and binding outbound requests with [`binding`].
pub fn state(db: PgPool) -> AppState {
    let config = config("postgres://test@localhost/test");
    let blobs = BlobStore::memory(&config.blobs, db.clone());
    AppState::new(db, config)
        .with_blobs(blobs)
        .with_binding(binding())
}

/// The binding tests sign and verify sends and provider changes with: a
/// fixed test key, as the server's would be derived from the instance
/// secret.
pub fn binding() -> crate::binding::Binding {
    crate::binding::Binding::new(&crate::crypto::SecretKey::from_bytes(
        [0x5a; crate::crypto::SECRET_KEY_LEN],
    ))
}

/// A pool whose every acquire fails quickly: for "database is down" paths.
pub fn unreachable_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(300))
        // Port 1 on loopback: nothing listens, so connects are refused at once.
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:1/none")
        .unwrap_or_else(|e| unreachable!("static URL parses: {e}"))
}

/// A module's routes as a plain router over `state`, behind the
/// [`localize`](crate::i18n::localize) layer (so problems answer in the
/// request's locale, as in the server) and the
/// [tracking context](crate::tracking::http) layer (so what a handler
/// tracks names its operation and caller). When `state` has no access
/// catalogue yet, the module's own resource types are installed.
pub fn app(module: Module, state: AppState) -> Router {
    let state = if state.resources.is_empty() {
        match crate::authz::Catalogue::new([&module.resources]) {
            Ok(resources) => state.with_resources(resources),
            Err(_) => state,
        }
    } else {
        state
    };
    let (router, spec) = module.router.split_for_parts();
    let router = router.route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::i18n::localize,
    ));
    crate::tracking::http::around(
        router,
        crate::tracking::http::HttpTracking::from_spec(&spec),
    )
    .with_state(state)
}

/// The same state, writing tracked events to the outbox
/// (`CAMPBOOKS_TRACKING__SINK=outbox`), so a test can read them back with
/// [`tracked_events`].
pub fn tracking_outbox(state: AppState) -> AppState {
    let tracker = crate::tracking::Tracker::new(&crate::config::TrackingConfig {
        sink: crate::config::TrackingSink::Outbox,
    });
    state.with_tracker(tracker)
}

/// One row of `kernel.tracked_events`, as [`tracked_events`] reads it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrackedRow {
    /// The event's id.
    pub id: uuid::Uuid,
    /// Its workspace, if any.
    pub workspace_id: Option<uuid::Uuid>,
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: i32,
    /// What the kernel wrote around it.
    pub envelope: serde_json::Value,
    /// Its properties and snapshots.
    pub body: serde_json::Value,
}

/// The tracked events named `name` in the outbox, oldest first. `db` must
/// read past row-level security (the pool `#[sqlx::test]` hands you): the
/// runtime role may only append to the outbox.
///
/// # Panics
/// If the database refuses the read.
pub async fn tracked_events(db: &PgPool, name: &str) -> Vec<TrackedRow> {
    sqlx::query_as(
        "SELECT id, workspace_id, name, version, envelope, body
           FROM kernel.tracked_events WHERE name = $1 ORDER BY id",
    )
    .bind(name)
    .fetch_all(db)
    .await
    .unwrap_or_else(|e| panic!("reading tracked events: {e}"))
}

/// A module's admin routes (its [`admin_router`](Module::admin_router)) as
/// the server serves them: behind the [`audit`](crate::admin::audit)
/// middleware (so [`Admin`](crate::admin::Admin) answers) and the
/// [`localize`](crate::i18n::localize) layer. Empty when it has none.
pub fn admin_app(module: Module, state: AppState) -> Router {
    let Some(admin) = module.admin_router else {
        return Router::new();
    };
    let (routes, spec) = admin.split_for_parts();
    routes
        .route_layer(axum::middleware::from_fn_with_state(
            crate::admin::AuditLayer::new(&state, &spec),
            crate::admin::audit,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::i18n::localize,
        ))
        .with_state(state)
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
