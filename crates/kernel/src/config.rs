//! Runtime configuration, read from the environment.
//!
//! Every variable is prefixed with `CAMPBOOKS_`; nested keys are separated by
//! a double underscore, e.g. `CAMPBOOKS_AI__DECIDE__MODEL` is `ai.decide.model`.
//! Other crates extract their own sections from [`env_source`] so there is one
//! convention for the whole server.

use std::net::SocketAddr;

use figment::{Figment, providers::Env};
use secrecy::SecretString;
use serde::Deserialize;

/// Environment variable prefix shared by every Campbooks setting.
pub const ENV_PREFIX: &str = "CAMPBOOKS_";

/// Separator between nested keys in environment variable names.
pub const ENV_SEPARATOR: &str = "__";

/// The environment as a configuration source: `CAMPBOOKS_*` variables, keys
/// lower-cased, nested on `__`.
///
/// Crates that own a config section (e.g. `campbooks-ai`) extract it with
/// [`Figment::extract_inner`].
pub fn env_source() -> Figment {
    Figment::from(Env::prefixed(ENV_PREFIX).split(ENV_SEPARATOR))
}

/// Log output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// One JSON object per line; for production and log shippers.
    #[default]
    Json,
    /// Human-readable, coloured output; for local development.
    Pretty,
}

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Postgres connection string (`CAMPBOOKS_DATABASE_URL`). Secret: it
    /// usually embeds a password.
    pub database_url: SecretString,

    /// Maximum connections in the database pool.
    #[serde(default = "defaults::database_max_connections")]
    pub database_max_connections: u32,

    /// Address the HTTP server binds to.
    #[serde(default = "defaults::bind_addr")]
    pub bind_addr: SocketAddr,

    /// Public base URL of this instance, used to build absolute links.
    #[serde(default = "defaults::public_url")]
    pub public_url: String,

    /// Apply pending migrations on startup.
    #[serde(default = "defaults::run_migrations")]
    pub run_migrations: bool,

    /// Log output format.
    #[serde(default)]
    pub log_format: LogFormat,
}

impl Config {
    /// Load from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_figment(&env_source())
    }

    /// Load from any figment source (used by tests to avoid touching the
    /// process environment).
    pub fn from_figment(figment: &Figment) -> Result<Self, ConfigError> {
        figment.extract().map_err(|e| ConfigError(Box::new(e)))
    }
}

/// Configuration could not be loaded; the message names the offending key.
#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(#[source] Box<figment::Error>);

mod defaults {
    use std::net::{Ipv4Addr, SocketAddr};

    pub fn database_max_connections() -> u32 {
        10
    }

    pub fn bind_addr() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080))
    }

    pub fn public_url() -> String {
        "http://localhost:8080".to_owned()
    }

    pub fn run_migrations() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;
    use secrecy::ExposeSecret;

    use super::*;

    fn figment(pairs: &[(&str, &str)]) -> Figment {
        let map: std::collections::BTreeMap<_, _> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Figment::from(Serialized::defaults(map))
    }

    #[test]
    fn applies_defaults() {
        let config = Config::from_figment(&figment(&[(
            "database_url",
            "postgres://u:p@db.example.com/campbooks",
        )]))
        .unwrap();

        assert_eq!(config.bind_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.public_url, "http://localhost:8080");
        assert!(config.run_migrations);
        assert_eq!(config.log_format, LogFormat::Json);
        assert_eq!(config.database_max_connections, 10);
        assert_eq!(
            config.database_url.expose_secret(),
            "postgres://u:p@db.example.com/campbooks"
        );
    }

    #[test]
    fn database_url_is_required() {
        let err = Config::from_figment(&figment(&[])).unwrap_err();
        assert!(err.to_string().contains("database_url"), "{err}");
    }

    #[test]
    fn parses_overrides() {
        let config = Config::from_figment(&figment(&[
            ("database_url", "postgres://localhost/x"),
            ("bind_addr", "127.0.0.1:9000"),
            ("log_format", "pretty"),
        ]))
        .unwrap();
        assert_eq!(config.bind_addr.port(), 9000);
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn debug_redacts_database_url() {
        let config =
            Config::from_figment(&figment(&[("database_url", "postgres://u:hunter2@h/x")]))
                .unwrap();
        assert!(!format!("{config:?}").contains("hunter2"));
    }
}
