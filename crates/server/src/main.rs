//! The `campbooks-server` binary.

#![forbid(unsafe_code)]

use std::{
    io::{Read as _, Write as _},
    net::{SocketAddr, TcpStream},
    process::ExitCode,
    time::Duration,
};

use campbooks_kernel::{AppState, Config, config::LogFormat};
use clap::{Parser, Subcommand};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

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
    about = "The Campbooks API server."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP server (the default).
    Serve,
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

fn main() -> ExitCode {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => serve(),
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

fn serve() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    init_tracing(config.log_format);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return fail(&format!("could not start async runtime: {e}")),
    };
    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "server failed");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
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

    if let Some(dir) = campbooks_server::web::configured_dir(&config) {
        campbooks_server::web::check_dir(dir)?;
        tracing::info!(dir = %dir.display(), "serving the web client");
    } else {
        tracing::info!("CAMPBOOKS_WEB_DIR unset: serving the API only");
    }

    let db = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(DB_ACQUIRE_TIMEOUT)
        .connect(config.database_url.expose_secret())
        .await
        .map_err(|e| format!("could not connect to the database: {e}"))?;

    if config.run_migrations {
        sqlx::migrate!("../../migrations")
            .run(&db)
            .await
            .map_err(|e| format!("migrations failed: {e}"))?;
        tracing::info!("migrations applied");
    }

    let bind_addr = config.bind_addr;
    let state = AppState::new(db.clone(), config);
    let app = campbooks_server::router(state);
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| format!("could not bind {bind_addr}: {e}"))?;
    tracing::info!(addr = %bind_addr, version = env!("CARGO_PKG_VERSION"), "campbooks-server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    db.close().await;
    tracing::info!("shut down cleanly");
    Ok(())
}

fn init_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn"));
    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Json => registry
            .with(fmt::layer().json().flatten_event(true))
            .init(),
        LogFormat::Pretty => registry.with(fmt::layer().pretty()).init(),
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
    tracing::info!("shutdown signal received, draining connections");
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
