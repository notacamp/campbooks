//! The `campbooks-server` binary.

#![forbid(unsafe_code)]

mod admin_cli;

use std::{
    io::{Read as _, Write as _},
    net::{SocketAddr, TcpStream},
    process::ExitCode,
    time::Duration,
};

use std::sync::Arc;

use campbooks_kernel::{
    AppState, Config,
    blobs::BlobStore,
    config::BlobBackend,
    db::{self, RoleVerdict},
    events::{EventId, ReplayFrom},
    jobs::Worker,
};
use campbooks_server::{bff, telemetry::Telemetry};
use clap::{Parser, Subcommand};
use secrecy::ExposeSecret;
use sqlx::{Connection as _, PgConnection, PgPool};
use tokio::{sync::watch, task::JoinSet};

/// Port used when `CAMPBOOKS_BIND_ADDR` is unset or unparsable.
const DEFAULT_PORT: u16 = 8080;
/// How long the database pool waits for a connection.
const DB_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long `healthcheck` waits for the server.
const HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Parser)]
#[command(
    name = "campbooks-server",
    version,
    about = "The Campbooks server: HTTP API, job worker and tools."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP API and the job worker in one process (the default, and
    /// what the image runs).
    All,
    /// Run only the HTTP API (scale it on requests).
    Api,
    /// Run only the job worker: event deliveries, background jobs and
    /// schedules (scale it on queue depth).
    Worker,
    /// Same as `all`; kept for deployments that still run `serve`.
    #[command(hide = true)]
    Serve,
    /// Work with the domain event log.
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
    /// Manage instance administrators (ADR 0015): `grant`, `revoke`, `list`
    /// and the audit `log`. The only way to make someone an administrator;
    /// runs on the migration login.
    Admin {
        #[command(subcommand)]
        command: admin_cli::AdminCommand,
    },
    /// Work with the search index.
    Search {
        #[command(subcommand)]
        command: SearchCommand,
    },
    /// Print the OpenAPI document as JSON.
    Openapi,
    /// Print the OAuth scope catalogue as JSON (`[{name, description}]`), for
    /// the identity provider's bootstrap to mirror.
    Scopes,
    /// Exit 0 if the local server reports healthy, 1 otherwise. For container
    /// health checks (the runtime image has no shell or curl).
    Healthcheck {
        /// Port to probe on 127.0.0.1. Defaults to the port of
        /// `CAMPBOOKS_BIND_ADDR`, else 8080.
        #[arg(long)]
        port: Option<u16>,
    },
}

#[derive(Debug, Subcommand)]
enum EventsCommand {
    /// Deliver logged events to one consumer again, e.g. after fixing its
    /// handler or to rebuild a projection. Events that already have an
    /// unfinished delivery to it are skipped.
    Replay {
        /// The consumer's name (`<module>.<handler>`).
        consumer: String,
        /// Replay from this instant (RFC 3339, e.g. `2026-10-01T00:00:00Z`)
        /// or from this event id (inclusive). Default: the whole log.
        #[arg(long)]
        since: Option<String>,
    },
    /// Print the events catalogue as JSON (what `openapi/events.json`
    /// holds): every event the modules publish, its version, whether it is
    /// workspace-scoped, and its payload schema.
    Catalogue,
}

#[derive(Debug, Subcommand)]
enum SearchCommand {
    /// Rebuild a workspace's search index from its sources, in the
    /// background (a worker runs it). Search keeps answering meanwhile.
    Reindex {
        /// The workspace's id.
        #[arg(long)]
        workspace: String,
    },
}

/// Which parts of the server a process runs (ADR 0011).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    All,
    Api,
    Worker,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Api => "api",
            Self::Worker => "worker",
        }
    }

    fn api(self) -> bool {
        matches!(self, Self::All | Self::Api)
    }

    fn worker(self) -> bool {
        matches!(self, Self::All | Self::Worker)
    }
}

fn main() -> ExitCode {
    campbooks_server::telemetry::install_panic_hook();
    match Cli::parse().command.unwrap_or(Command::All) {
        Command::All | Command::Serve => serve(Role::All),
        Command::Api => serve(Role::Api),
        Command::Worker => serve(Role::Worker),
        Command::Events {
            command: EventsCommand::Replay { consumer, since },
        } => replay(&consumer, since.as_deref()),
        Command::Events {
            command: EventsCommand::Catalogue,
        } => match campbooks_server::events_catalogue() {
            Ok(catalogue) => print_json(&catalogue, "events catalogue"),
            Err(e) => fail(&e.to_string()),
        },
        Command::Admin { command } => admin_cli::run(command),
        Command::Search {
            command: SearchCommand::Reindex { workspace },
        } => reindex(&workspace),
        Command::Openapi => print_json(&campbooks_server::openapi(), "OpenAPI document"),
        Command::Scopes => print_json(&campbooks_server::scope_catalogue(), "scope catalogue"),
        Command::Healthcheck { port } => healthcheck(port.unwrap_or_else(port_from_env)),
    }
}

fn print_json(value: &impl serde::Serialize, what: &str) -> ExitCode {
    let result = serde_json::to_string_pretty(value)
        .map_err(std::io::Error::other)
        .and_then(|json| writeln!(std::io::stdout(), "{json}"));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("could not print the {what}: {e}")),
    }
}

fn serve(role: Role) -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    if let Err(e) = config.telemetry.check() {
        return fail(&e);
    }
    let telemetry = match Telemetry::new(&config.telemetry, role.as_str()) {
        Ok(telemetry) => telemetry,
        Err(e) => return fail(&e),
    };
    telemetry.install_subscriber(config.log_format);
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(e) => return fail(&e),
    };

    let result = runtime.block_on(run(config, role, &telemetry));
    // Stop every task first, then flush what they recorded.
    drop(runtime);
    telemetry.shutdown();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "server failed");
            ExitCode::FAILURE
        }
    }
}

/// Rate-limit counters keyed by the instance secret, so every process of the
/// install counts together; without one, each process counts alone.
fn rate_limits() -> Result<campbooks_kernel::rate_limit::RateLimits, String> {
    Ok(match campbooks_kernel::crypto::SecretKey::from_env()? {
        Some(secret) => campbooks_kernel::rate_limit::RateLimits::new(&secret),
        None => campbooks_kernel::rate_limit::RateLimits::default(),
    })
}

/// The signer of opaque values (cursors, the change feed's resume tokens)
/// keyed by the instance secret, so every process of the install verifies
/// them; without one, each process signs alone, which is said at startup.
fn signer() -> Result<campbooks_kernel::crypto::Signer, String> {
    if let Some(secret) = campbooks_kernel::crypto::SecretKey::from_env()? {
        return Ok(campbooks_kernel::crypto::Signer::new(&secret));
    }
    tracing::warn!(
        "CAMPBOOKS_SECRET_KEY is not set: search cursors and change feed resume tokens work \
         only in this process and until it restarts (clients then refetch); set it in production"
    );
    Ok(campbooks_kernel::crypto::Signer::default())
}

/// Startup checks of the settings `role` uses; the web BFF's settings when
/// it serves the API with web sign-in.
fn check(config: &Config, role: Role) -> Result<Option<bff::Settings>, String> {
    config.check()?;
    let bff = if role.api() {
        check_api_config(config)?
    } else {
        None
    };
    if role.worker() {
        config.jobs.check()?;
    }
    Ok(bff)
}

/// The instance's mailer (`CAMPBOOKS_MAIL__*`), saying which it is.
fn mailer(config: &Config) -> Result<campbooks_kernel::mail::MailerPort, String> {
    let mailer = config.mail.mailer()?;
    if mailer.is_configured() {
        tracing::info!("security notices are sent by mail");
    } else {
        tracing::info!("no mail transport configured; security notices are shown in the app only");
    }
    Ok(mailer)
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("could not start async runtime: {e}"))
}

async fn run(
    config: Config,
    role: Role,
    telemetry: &Telemetry,
) -> Result<(), Box<dyn std::error::Error>> {
    let bff = check(&config, role)?;
    let composition = campbooks_server::Composition::from_env(&config)?;
    log_composition(&composition);
    let registry = campbooks_server::registry_with(&composition)?;
    if config.run_migrations {
        migrate(&config).await?;
    }
    let db = connect(&config).await?;
    check_row_security(&db, &config).await?;
    telemetry.observe_pool(&db, config.database_max_connections);

    let blobs = blob_store(&config, &db).await?;
    let mailer = mailer(&config)?;

    let bind_addr = config.bind_addr;
    let metrics_addr = config.telemetry.metrics_addr;
    let state = campbooks_server::with_composed_ports(
        AppState::new(db.clone(), config)
            .with_rate_limits(rate_limits()?)
            .with_signer(signer()?)
            .with_metrics(telemetry.metrics())
            .with_registry(registry)
            .with_blobs(blobs)
            .with_mailer(mailer),
        &composition,
    );
    // Flipped to `true` on Ctrl-C/SIGTERM, or when one part stops, so the
    // other parts stop too.
    let stop = Arc::new(watch::channel(false).0);
    let mut parts: JoinSet<Result<(), String>> = JoinSet::new();

    check_answers(&composition, &stop);

    if role.api() {
        if let Some(issuer) = &composition.identity.issuer {
            // Fail closed: never serve with signing keys this secret cannot open.
            issuer.keys.check(&db).await?;
        }
        let app = campbooks_server::router_with(state.clone(), bff, &composition);
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("could not bind {bind_addr}: {e}"))?;
        tracing::info!(addr = %bind_addr, version = env!("CARGO_PKG_VERSION"), "campbooks-server listening");
        let stopped = stopped(&stop);
        parts.spawn(async move {
            // The peer address is what rate limits count by (or, behind
            // `CAMPBOOKS_TRUSTED_PROXIES`, what vouches for X-Forwarded-For).
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(stopped)
            .await
            .map_err(|e| format!("http server failed: {e}"))
        });
    }
    if let (Some(router), Some(addr)) = (telemetry.metrics_router(), metrics_addr) {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("could not bind CAMPBOOKS_TELEMETRY__METRICS_ADDR={addr}: {e}"))?;
        tracing::info!(%addr, "serving metrics at /metrics (keep this port private)");
        let stopped = stopped(&stop);
        parts.spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(stopped)
                .await
                .map_err(|e| format!("metrics listener failed: {e}"))
        });
    }
    if let Some(endpoint) = state.config.telemetry.otlp_endpoint() {
        tracing::info!(
            endpoint,
            sample_ratio = state.config.telemetry.sample_ratio,
            "exporting traces and metrics over OTLP"
        );
    }
    if role.worker() {
        let worker = Worker::new(state);
        let stopped = stopped(&stop);
        parts.spawn(async move {
            worker.run(stopped).await;
            Ok(())
        });
    }

    let signal = Arc::clone(&stop);
    tokio::spawn(async move {
        shutdown_signal().await;
        signal.send_replace(true);
    });

    let mut result = Ok(());
    while let Some(part) = parts.join_next().await {
        if let Err(e) = part.map_err(|e| e.to_string()).and_then(|ended| ended) {
            tracing::error!(error = %e, "stopping");
            result = Err(e);
        }
        stop.send_replace(true);
    }
    db.close().await;
    result?;
    tracing::info!("shut down cleanly");
    Ok(())
}

/// The configured blob store, checked to answer before the server starts.
async fn blob_store(config: &Config, db: &PgPool) -> Result<BlobStore, String> {
    let blobs = BlobStore::from_config(&config.blobs, db.clone())?;
    let backend = blobs.backend().unwrap_or_default();
    blobs.probe().await.map_err(|e| {
        format!("blob storage (CAMPBOOKS_BLOBS__BACKEND={backend}) does not answer: {e}")
    })?;
    match config.blobs.backend {
        BlobBackend::Fs => {
            tracing::info!(backend, path = %config.blobs.path.display(), "storing blobs");
        }
        BlobBackend::S3 => tracing::info!(
            backend,
            bucket = config.blobs.s3.bucket.as_deref().unwrap_or_default(),
            endpoint = config.blobs.s3.endpoint().unwrap_or("<aws>"),
            "storing blobs"
        ),
    }
    Ok(blobs)
}

/// The decide role's answers check: now, then periodically, never per
/// request (`campbooks_ai::decide::check`). Every process keeps its own: the
/// API shows it in the admin System panel, the worker gates the model pass
/// on it.
fn check_answers(composition: &campbooks_server::Composition, stop: &watch::Sender<bool>) {
    if let Some(gateway) = composition.system.ai.clone() {
        let stopped = stopped(stop);
        tokio::spawn(async move { gateway.keep_answers_checked(stopped).await });
    }
}

/// Resolves once `stop` is flipped.
fn stopped(stop: &watch::Sender<bool>) -> impl Future<Output = ()> + Send + 'static {
    let mut stop = stop.subscribe();
    async move {
        let _ = stop.wait_for(|stopped| *stopped).await;
    }
}

/// Apply pending migrations on a connection of their own, as the migration
/// login (`CAMPBOOKS_DATABASE_MIGRATION_URL`, else the database URL): the
/// runtime pool may not change the schema.
async fn migrate(config: &Config) -> Result<(), String> {
    let options = if config.split_database_logins() {
        // The migration URL carries its own credentials.
        db::connect_options(config.migration_url(), None)?
    } else {
        db::connect_options(config.migration_url(), config.database_password_file())?
    };
    let mut conn = PgConnection::connect_with(&options)
        .await
        .map_err(|e| format!("could not connect to the database to migrate: {e}"))?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK)
        .execute(&mut conn)
        .await
        .map_err(|e| format!("could not take the migration lock: {e}"))?;
    let migrated = migrate_locked(&mut conn).await;
    // Closing the connection releases the lock too; unlock first anyway.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK)
        .execute(&mut conn)
        .await;
    let _ = conn.close().await;
    migrated?;
    tracing::info!("migrations applied");
    Ok(())
}

/// The session advisory lock every Campbooks process holds while it
/// migrates, so no process takes another's index build in flight (not valid
/// until it ends) for a failed one.
const MIGRATION_LOCK: i64 = 0x6361_6d70_6d69_6772;

/// Under [`MIGRATION_LOCK`]: drop what failed index builds left, then
/// migrate.
async fn migrate_locked(conn: &mut PgConnection) -> Result<(), String> {
    drop_invalid_indexes(conn).await?;
    sqlx::migrate!("../../migrations")
        .run(conn)
        .await
        .map_err(|e| format!("migrations failed: {e}"))
}

/// The schemas the migrations create and own (`<schema>__<name>.sql`): the
/// only ones whose invalid indexes [`drop_invalid_indexes`] touches.
const MIGRATED_SCHEMAS: [&str; 11] = [
    "admin",
    "assistant",
    "attention",
    "bff",
    "connectors",
    "conversations",
    "identity",
    "kernel",
    "search",
    "system",
    "workspaces",
];

/// Drop the invalid indexes of [`MIGRATED_SCHEMAS`]. `CREATE INDEX
/// CONCURRENTLY` leaves one behind when the build fails or its process dies
/// partway; the migration that builds it (`IF NOT EXISTS`) would otherwise
/// keep it, and Postgres never uses it. Run under [`MIGRATION_LOCK`]: a
/// migration's build still in flight holds it. Other schemas (an operator's
/// own, built by hand) are left alone. Returns the indexes dropped, as
/// `schema.name`.
async fn drop_invalid_indexes(conn: &mut PgConnection) -> Result<Vec<String>, String> {
    let invalid: Vec<(String, String, String)> = sqlx::query_as(
        r"SELECT n.nspname::text, c.relname::text, format('%I.%I', n.nspname, c.relname)
            FROM pg_index i
            JOIN pg_class c ON c.oid = i.indexrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
           WHERE NOT i.indisvalid AND n.nspname = ANY($1)
           ORDER BY 1, 2",
    )
    .bind(&MIGRATED_SCHEMAS[..])
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("could not look for invalid indexes: {e}"))?;
    let mut dropped = Vec::with_capacity(invalid.len());
    for (schema, index, quoted) in invalid {
        tracing::warn!(%schema, %index, "dropping an invalid index a failed concurrent build left");
        // `quoted` is quoted by `format('%I.%I')` above.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP INDEX CONCURRENTLY IF EXISTS {quoted}"
        )))
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("could not drop the invalid index {schema}.{index}: {e}"))?;
        dropped.push(format!("{schema}.{index}"));
    }
    Ok(dropped)
}

/// The pool every request and job uses. A privileged login switches each
/// connection to the runtime role (`CAMPBOOKS_DATABASE_ROLE`), so row-level
/// security applies; an unprivileged one already is subject to it.
async fn connect(config: &Config) -> Result<PgPool, String> {
    let options = db::connect_options(
        config.database_url.expose_secret(),
        config.database_password_file(),
    )?;
    db::connect_runtime(
        options,
        config.database_max_connections,
        DB_ACQUIRE_TIMEOUT,
        config.database_role(),
    )
    .await
    .map_err(|e| match config.database_role() {
        Some(role) => format!(
            "could not connect to the database (as runtime role `{role}` if the login is \
             privileged; run the migrations, or check CAMPBOOKS_DATABASE_ROLE): {e}"
        ),
        None => format!("could not connect to the database: {e}"),
    })
}

/// Refuse to serve when row-level security would protect nothing, warn when
/// it holds only through a switch a SQL injection could undo.
///
/// Refusing (rather than warning) is deliberate: workspace isolation is the
/// product's core promise, and a warning in a log nobody reads is how it
/// silently stops being true. `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true`
/// overrides, for setups that cannot create a second login yet.
async fn check_row_security(db: &PgPool, config: &Config) -> Result<(), String> {
    let report = match db::inspect_role(db).await {
        Ok(report) => report,
        Err(e) => {
            tracing::warn!(error = %e, "could not check the database role");
            return Ok(());
        }
    };
    match report.verdict(config.split_database_logins()) {
        RoleVerdict::Sound => Ok(()),
        RoleVerdict::Escapable(why) => {
            tracing::warn!(
                "{why}. Create a dedicated non-superuser login for CAMPBOOKS_DATABASE_URL \
                 and move the current URL to CAMPBOOKS_DATABASE_MIGRATION_URL (deploy/README.md)"
            );
            Ok(())
        }
        RoleVerdict::Unsafe(why) if config.database_allow_rls_bypass => {
            tracing::warn!(
                "{why}; starting anyway because CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS is set"
            );
            Ok(())
        }
        RoleVerdict::Unsafe(why) => Err(format!(
            "refusing to start: {why}. Use a dedicated non-superuser login (deploy/README.md), \
             or set CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true to accept that workspace \
             isolation then rests on the queries alone"
        )),
    }
}

/// `events replay <consumer> [--since …]`.
fn replay(consumer: &str, since: Option<&str>) -> ExitCode {
    let from = match parse_since(since) {
        Ok(from) => from,
        Err(e) => return fail(&e),
    };
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    let registry = match campbooks_server::registry() {
        Ok(registry) => registry,
        Err(e) => return fail(&e.to_string()),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(e) => return fail(&e),
    };
    let replayed = runtime.block_on(async {
        let db = connect(&config).await?;
        let state = AppState::new(db.clone(), config).with_registry(registry);
        let replayed = state
            .events
            .replay(&db, consumer, from)
            .await
            .map_err(|e| e.to_string());
        db.close().await;
        replayed
    });
    match replayed {
        Ok(count) => {
            let _ = writeln!(
                std::io::stdout(),
                "enqueued {count} delivery(ies) to `{consumer}`; a worker runs them"
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

/// `search reindex --workspace <id>`.
fn reindex(workspace: &str) -> ExitCode {
    let Ok(workspace) = workspace.parse::<campbooks_kernel::tenancy::WorkspaceId>() else {
        return fail("--workspace must be a workspace id (a UUID)");
    };
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    let registry = match campbooks_server::registry() {
        Ok(registry) => registry,
        Err(e) => return fail(&e.to_string()),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(e) => return fail(&e),
    };
    let restarted = runtime.block_on(async {
        let db = connect(&config).await?;
        let state =
            campbooks_server::with_ports(AppState::new(db.clone(), config).with_registry(registry));
        let restarted = campbooks_search::reindex(&state, workspace)
            .await
            .map_err(|e| e.to_string());
        db.close().await;
        restarted
    });
    match restarted {
        Ok(sources) => {
            let _ = writeln!(
                std::io::stdout(),
                "reindexing {} source(s) of the workspace; a worker runs it",
                sources.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

/// `--since`: an event id or an RFC 3339 instant.
fn parse_since(since: Option<&str>) -> Result<ReplayFrom, String> {
    let Some(since) = since else {
        return Ok(ReplayFrom::Beginning);
    };
    if let Ok(id) = since.parse::<EventId>() {
        return Ok(ReplayFrom::Event(id));
    }
    chrono::DateTime::parse_from_rfc3339(since)
        .map(|at| ReplayFrom::Time(at.to_utc()))
        .map_err(|_| format!("--since {since}: expected an event id or an RFC 3339 time"))
}

/// Startup checks of the API role; the web BFF's settings when web sign-in
/// is on.
fn check_api_config(config: &Config) -> Result<Option<bff::Settings>, String> {
    config.auth.check()?;
    if let Some(issuer) = config.auth.issuer() {
        tracing::info!(
            issuer,
            audience = %config.auth.audience,
            jwks_url = config.auth.jwks_url().unwrap_or("<discovered>"),
            "validating access tokens"
        );
    } else {
        tracing::warn!(
            "CAMPBOOKS_AUTH__ISSUER unset: no identity provider, so every endpoint that \
             requires a scope answers 401 auth_not_configured"
        );
    }

    if let Some(dir) = campbooks_server::web::configured_dir(config) {
        campbooks_server::web::check_dir(dir)?;
        tracing::info!(dir = %dir.display(), "serving the web client");
    } else {
        tracing::info!("CAMPBOOKS_WEB_DIR unset: serving the API only");
    }

    let scopes: Vec<&str> = campbooks_server::scope_catalogue()
        .iter()
        .map(|scope| scope.name)
        .collect();
    match bff::config::load(config, &scopes)? {
        bff::Loaded::Enabled(settings) => {
            tracing::info!(
                client_id = %settings.client_id,
                origin = %settings.public_origin,
                idp = %settings.internal_issuer,
                "web sign-in (BFF) enabled"
            );
            if !settings.secure_cookies {
                tracing::warn!(
                    "CAMPBOOKS_PUBLIC_URL is plain http: the session cookie is sent without \
                     `Secure` and the `__Host-` prefix. Fine on localhost; serve the app over \
                     https anywhere else"
                );
            }
            Ok(Some(*settings))
        }
        bff::Loaded::Disabled(reason) => {
            tracing::warn!(reason, "web sign-in (BFF) disabled: /bff/* answers 503");
            Ok(None)
        }
    }
}

/// Say which providers and which identity provider this instance runs with.
fn log_composition(composition: &campbooks_server::Composition) {
    if composition.connectors.google.is_some() {
        tracing::info!("Google accounts can be connected");
    } else {
        tracing::info!("CAMPBOOKS_GOOGLE__CLIENT_ID unset: Google accounts cannot be connected");
    }
    if composition.connectors.zoho.is_some() {
        tracing::info!("Zoho Mail accounts can be connected");
    } else {
        tracing::info!("CAMPBOOKS_ZOHO__CLIENT_ID unset: Zoho Mail accounts cannot be connected");
    }
    if let Some(issuer) = &composition.identity.issuer {
        let providers: Vec<&str> = issuer
            .upstreams
            .iter()
            .map(|u| u.provider().as_str())
            .collect();
        tracing::info!(
            issuer = %issuer.url,
            providers = ?providers,
            "this instance is its own identity provider (OAuth 2.1 issuer)"
        );
    } else {
        tracing::info!(
            "the in-process identity provider is off: CAMPBOOKS_AUTH__ISSUER names another one, \
             or CAMPBOOKS_SECRET_KEY is unset"
        );
    }
}

/// Resolves on Ctrl-C or SIGTERM (what `docker stop` sends).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received, draining connections and jobs");
}

fn port_from_env() -> u16 {
    std::env::var("CAMPBOOKS_BIND_ADDR")
        .ok()
        .and_then(|addr| addr.parse::<SocketAddr>().ok())
        .map_or(DEFAULT_PORT, |addr| addr.port())
}

/// A dependency-free HTTP/1.1 probe of the health endpoint.
fn healthcheck(port: u16) -> ExitCode {
    match probe(port) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => fail("unhealthy"),
        Err(e) => fail(&format!("healthcheck failed: {e}")),
    }
}

fn probe(port: u16) -> std::io::Result<bool> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, HEALTHCHECK_TIMEOUT)?;
    stream.set_read_timeout(Some(HEALTHCHECK_TIMEOUT))?;
    stream.set_write_timeout(Some(HEALTHCHECK_TIMEOUT))?;
    write!(
        stream,
        "GET /api/v1/system/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut head = [0_u8; 32];
    let read = stream.read(&mut head)?;
    let status_line = String::from_utf8_lossy(&head[..read]);
    Ok(status_line.starts_with("HTTP/1.1 200"))
}

fn fail(message: &str) -> ExitCode {
    // Best effort: if stderr is gone there is nowhere left to report to.
    let _ = writeln!(std::io::stderr(), "campbooks-server: {message}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_accepts_an_event_id_or_a_time() {
        assert_eq!(parse_since(None), Ok(ReplayFrom::Beginning));
        let id = EventId::new();
        assert_eq!(
            parse_since(Some(&id.to_string())),
            Ok(ReplayFrom::Event(id))
        );
        let Ok(ReplayFrom::Time(at)) = parse_since(Some("2026-10-01T12:00:00+02:00")) else {
            panic!("an RFC 3339 time parses");
        };
        assert_eq!(at.to_rfc3339(), "2026-10-01T10:00:00+00:00");
        assert!(
            parse_since(Some("yesterday"))
                .unwrap_err()
                .contains("--since")
        );
    }

    fn config(split: bool, allow: bool) -> Config {
        let mut config = campbooks_kernel::testing::config("postgres://test@localhost/test");
        config.database_allow_rls_bypass = allow;
        if split {
            config.database_migration_url = Some(secrecy::SecretString::from(
                "postgres://owner@localhost/test",
            ));
        }
        config
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_failed_concurrent_index_build_is_dropped_before_migrating(db: PgPool) {
        let mut conn = db.acquire().await.unwrap();
        // A unique index over duplicates fails partway and stays, invalid:
        // one in a schema the migrations own, one in an operator's own.
        for schema in ["conversations", "public"] {
            for sql in [
                format!("CREATE TABLE {schema}.twice (n integer)"),
                format!("INSERT INTO {schema}.twice VALUES (1), (1)"),
                format!("CREATE INDEX twice_valid_idx ON {schema}.twice (n)"),
            ] {
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .execute(&mut *conn)
                    .await
                    .unwrap();
            }
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE UNIQUE INDEX CONCURRENTLY twice_n_idx ON {schema}.twice (n)"
            )))
            .execute(&mut *conn)
            .await
            .unwrap_err();
        }
        let dropped = drop_invalid_indexes(&mut conn).await.unwrap();
        assert_eq!(dropped, vec!["conversations.twice_n_idx".to_owned()]);
        let left: Vec<String> = sqlx::query_scalar(
            "SELECT schemaname || '.' || indexname FROM pg_indexes
              WHERE tablename = 'twice' ORDER BY 1",
        )
        .fetch_all(&mut *conn)
        .await
        .unwrap();
        assert_eq!(
            left,
            [
                "conversations.twice_valid_idx",
                "public.twice_n_idx",
                "public.twice_valid_idx"
            ]
        );
        // The migrations, already applied, run again cleanly.
        migrate_locked(&mut conn).await.unwrap();
    }

    #[test]
    fn every_migration_belongs_to_a_swept_schema() {
        for migration in sqlx::migrate!("../../migrations").iter() {
            // `<schema>__<name>.sql`; sqlx turns `_` into spaces.
            let schema = migration.description.split("  ").next().unwrap_or_default();
            assert!(
                MIGRATED_SCHEMAS.contains(&schema),
                "{} names schema `{schema}`: add it to MIGRATED_SCHEMAS",
                migration.version
            );
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn startup_refuses_a_login_that_bypasses_row_security(db: PgPool) {
        use campbooks_kernel::testing;

        // A superuser straight into the pool: nothing protects the rows.
        let err = check_row_security(&db, &config(false, false))
            .await
            .unwrap_err();
        assert!(err.contains("refusing to start"), "{err}");
        // ... unless the operator says so.
        check_row_security(&db, &config(false, true)).await.unwrap();

        // A superuser that switches role: tolerated alone (with a warning),
        // refused once a separate migration login was configured.
        let switched = testing::app_role_pool(&db).await;
        check_row_security(&switched, &config(false, false))
            .await
            .unwrap();
        assert!(
            check_row_security(&switched, &config(true, false))
                .await
                .is_err()
        );

        // The dedicated login passes either way.
        let login = testing::runtime_login_pool(&db).await;
        check_row_security(&login, &config(true, false))
            .await
            .unwrap();
        check_row_security(&login, &config(false, false))
            .await
            .unwrap();
        login.close().await;
        testing::drop_runtime_login(&db).await;
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_pool_switches_role_only_for_a_privileged_login(db: PgPool) {
        use campbooks_kernel::testing;

        let timeout = std::time::Duration::from_secs(5);
        let privileged = db::connect_runtime(
            (*db.connect_options()).clone(),
            2,
            timeout,
            Some(db::DEFAULT_RUNTIME_ROLE),
        )
        .await
        .unwrap();
        assert_eq!(
            db::inspect_role(&privileged).await.unwrap().user,
            db::DEFAULT_RUNTIME_ROLE
        );

        // The dedicated login keeps its own identity: no SET ROLE to undo.
        let login = testing::runtime_login_pool(&db).await;
        let pool = db::connect_runtime(
            (*login.connect_options()).clone(),
            2,
            timeout,
            Some(db::DEFAULT_RUNTIME_ROLE),
        )
        .await
        .unwrap();
        let report = db::inspect_role(&pool).await.unwrap();
        assert_eq!(report.user, report.login);
        pool.close().await;
        login.close().await;
        testing::drop_runtime_login(&db).await;
    }

    #[test]
    fn roles() {
        assert!(Role::All.api() && Role::All.worker());
        assert!(Role::Api.api() && !Role::Api.worker());
        assert!(!Role::Worker.api() && Role::Worker.worker());
    }
}
