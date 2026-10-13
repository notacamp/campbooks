//! The `campbooks-server` binary.

#![forbid(unsafe_code)]

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
    db,
    events::{EventId, ReplayFrom},
    jobs::Worker,
};
use campbooks_server::telemetry::Telemetry;
use clap::{Parser, Subcommand};
use secrecy::ExposeSecret;
use sqlx::{Connection as _, PgConnection, PgPool, postgres::PgPoolOptions};
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
    config.check()?;
    if role.api() {
        check_api_config(&config)?;
    }
    if role.worker() {
        config.jobs.check()?;
    }
    let registry = campbooks_server::registry()?;
    if config.run_migrations {
        migrate(&config).await?;
    }
    let db = connect(&config).await?;
    warn_if_row_security_is_bypassed(&db).await;
    telemetry.observe_pool(&db, config.database_max_connections);

    let blobs = blob_store(&config, &db).await?;

    let bind_addr = config.bind_addr;
    let metrics_addr = config.telemetry.metrics_addr;
    let state = campbooks_server::with_ports(
        AppState::new(db.clone(), config)
            .with_metrics(telemetry.metrics())
            .with_registry(registry)
            .with_blobs(blobs),
    );
    // Flipped to `true` on Ctrl-C/SIGTERM, or when one part stops, so the
    // other parts stop too.
    let stop = Arc::new(watch::channel(false).0);
    let mut parts: JoinSet<Result<(), String>> = JoinSet::new();

    if role.api() {
        let app = campbooks_server::router(state.clone());
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("could not bind {bind_addr}: {e}"))?;
        tracing::info!(addr = %bind_addr, version = env!("CARGO_PKG_VERSION"), "campbooks-server listening");
        let stopped = stopped(&stop);
        parts.spawn(async move {
            axum::serve(listener, app)
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

/// Resolves once `stop` is flipped.
fn stopped(stop: &watch::Sender<bool>) -> impl Future<Output = ()> + Send + 'static {
    let mut stop = stop.subscribe();
    async move {
        let _ = stop.wait_for(|stopped| *stopped).await;
    }
}

/// Apply pending migrations on a connection of its own, as the login user
/// (the pool runs as the runtime role, which may not change the schema).
async fn migrate(config: &Config) -> Result<(), String> {
    let mut conn = PgConnection::connect(config.database_url.expose_secret())
        .await
        .map_err(|e| format!("could not connect to the database: {e}"))?;
    sqlx::migrate!("../../migrations")
        .run(&mut conn)
        .await
        .map_err(|e| format!("migrations failed: {e}"))?;
    let _ = conn.close().await;
    tracing::info!("migrations applied");
    Ok(())
}

/// The pool every request and job uses: each connection switches to the
/// runtime role (`CAMPBOOKS_DATABASE_ROLE`), so row-level security applies.
async fn connect(config: &Config) -> Result<PgPool, String> {
    let options = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(DB_ACQUIRE_TIMEOUT)
        // An event per acquire, for the pool-wait metric and span events;
        // the log filter keeps it out of the logs.
        .acquire_time_level(log::LevelFilter::Debug);
    db::with_runtime_role(options, config.database_role())
        .connect(config.database_url.expose_secret())
        .await
        .map_err(|e| match config.database_role() {
            Some(role) => format!(
                "could not connect to the database as runtime role `{role}` (run the \
                 migrations, or check CAMPBOOKS_DATABASE_ROLE): {e}"
            ),
            None => format!("could not connect to the database: {e}"),
        })
}

async fn warn_if_row_security_is_bypassed(db: &PgPool) {
    match db::bypasses_row_security(db).await {
        Ok(false) => {}
        Ok(true) => tracing::warn!(
            "the database role the server queries as bypasses row-level security (a \
             superuser or BYPASSRLS): workspace isolation then rests on queries alone; \
             set CAMPBOOKS_DATABASE_ROLE (default campbooks_app)"
        ),
        Err(e) => tracing::warn!(error = %e, "could not check the database role"),
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

fn check_api_config(config: &Config) -> Result<(), String> {
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
    Ok(())
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

    #[test]
    fn roles() {
        assert!(Role::All.api() && Role::All.worker());
        assert!(Role::Api.api() && !Role::Api.worker());
        assert!(!Role::Worker.api() && Role::Worker.worker());
    }
}
