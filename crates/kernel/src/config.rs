//! Runtime configuration, read from the environment.
//!
//! Every variable is prefixed with `CAMPBOOKS_`; nested keys are separated by
//! a double underscore, e.g. `CAMPBOOKS_AI__DECIDE__MODEL` is `ai.decide.model`.
//! Other crates extract their own sections from [`env_source`] so there is one
//! convention for the whole server.

use std::{net::SocketAddr, path::PathBuf};

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

    /// Directory holding the built web client (`CAMPBOOKS_WEB_DIR`). When
    /// set, the server serves it next to the API; unset (or empty) means
    /// API-only.
    #[serde(default)]
    pub web_dir: Option<PathBuf>,

    /// How the API validates OAuth access tokens (`CAMPBOOKS_AUTH__*`).
    #[serde(default)]
    pub auth: AuthConfig,
}

/// Access-token validation settings (`CAMPBOOKS_AUTH__*`). The API is an
/// OAuth 2.1 resource server (ADR 0004): it trusts JWTs signed by the
/// identity provider at [`issuer`](Self::issuer).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuthConfig {
    /// The identity provider's issuer, compared byte for byte with a token's
    /// `iss` (`CAMPBOOKS_AUTH__ISSUER`, e.g. `http://localhost:8081/auth/v1/`
    /// — keep the IdP's trailing slash). Unset (or empty) means tokens cannot
    /// be validated: every scoped endpoint answers 401 `auth_not_configured`.
    #[serde(default)]
    pub issuer: Option<String>,

    /// The audience this API must find in a token's `aud`
    /// (`CAMPBOOKS_AUTH__AUDIENCE`).
    #[serde(default = "defaults::audience")]
    pub audience: String,

    /// Where to fetch the IdP's signing keys (`CAMPBOOKS_AUTH__JWKS_URL`).
    /// Unset (or empty) means: discover it from the issuer's
    /// `.well-known/openid-configuration`. Set it when the server reaches the
    /// IdP on an internal address (e.g. `http://idp:8080/auth/v1/oidc/certs`
    /// in docker compose).
    #[serde(default)]
    pub jwks_url: Option<String>,

    /// Clock skew tolerated on `exp`, `nbf` and `iat`, in seconds
    /// (`CAMPBOOKS_AUTH__LEEWAY_SECONDS`).
    #[serde(default = "defaults::leeway_seconds")]
    pub leeway_seconds: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            issuer: None,
            audience: defaults::audience(),
            jwks_url: None,
            leeway_seconds: defaults::leeway_seconds(),
        }
    }
}

impl AuthConfig {
    /// The configured issuer; `None` when unset or blank.
    pub fn issuer(&self) -> Option<&str> {
        non_blank(self.issuer.as_deref())
    }

    /// The configured JWKS URL; `None` when unset or blank (discover it).
    pub fn jwks_url(&self) -> Option<&str> {
        non_blank(self.jwks_url.as_deref())
    }

    /// Startup check: when an issuer is set, it and the JWKS URL must be
    /// `http(s)` URLs and the audience must not be blank. The message names
    /// the offending setting.
    pub fn check(&self) -> Result<(), String> {
        let Some(issuer) = self.issuer() else {
            return Ok(());
        };
        check_url("CAMPBOOKS_AUTH__ISSUER", issuer)?;
        if let Some(jwks_url) = self.jwks_url() {
            check_url("CAMPBOOKS_AUTH__JWKS_URL", jwks_url)?;
        }
        if self.audience.trim().is_empty() {
            return Err("CAMPBOOKS_AUTH__AUDIENCE must not be blank".to_owned());
        }
        Ok(())
    }
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn check_url(setting: &str, value: &str) -> Result<(), String> {
    match reqwest::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.has_host() => Ok(()),
        Ok(_) => Err(format!("{setting}={value} must be an http(s) URL")),
        Err(e) => Err(format!("{setting}={value} is not a URL: {e}")),
    }
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

    pub fn audience() -> String {
        "campbooks-api".to_owned()
    }

    pub fn leeway_seconds() -> u64 {
        60
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
        assert_eq!(config.web_dir, None);
        assert_eq!(config.auth, AuthConfig::default());
        assert_eq!(config.auth.issuer(), None);
        assert_eq!(config.auth.audience, "campbooks-api");
        assert_eq!(config.auth.leeway_seconds, 60);
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
            ("web_dir", "/app/web"),
        ]))
        .unwrap();
        assert_eq!(config.bind_addr.port(), 9000);
        assert_eq!(config.log_format, LogFormat::Pretty);
        assert_eq!(config.web_dir, Some(PathBuf::from("/app/web")));
    }

    #[test]
    fn parses_auth_section() {
        // `CAMPBOOKS_AUTH__ISSUER` etc. land under the `auth` key.
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "auth",
                serde_json::json!({
                    "issuer": "http://localhost:8081/auth/v1/",
                    "jwks_url": "http://idp:8080/auth/v1/oidc/certs",
                    "audience": "my-api",
                    "leeway_seconds": 5,
                }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        assert_eq!(config.auth.issuer(), Some("http://localhost:8081/auth/v1/"));
        assert_eq!(
            config.auth.jwks_url(),
            Some("http://idp:8080/auth/v1/oidc/certs")
        );
        assert_eq!(config.auth.audience, "my-api");
        assert_eq!(config.auth.leeway_seconds, 5);
        assert!(config.auth.check().is_ok());
    }

    #[test]
    fn blank_auth_urls_count_as_unset() {
        let auth = AuthConfig {
            issuer: Some("  ".into()),
            jwks_url: Some(String::new()),
            ..AuthConfig::default()
        };
        assert_eq!(auth.issuer(), None);
        assert_eq!(auth.jwks_url(), None);
        assert!(auth.check().is_ok());
    }

    #[test]
    fn auth_check_names_the_bad_setting() {
        let bad_issuer = AuthConfig {
            issuer: Some("localhost:8081".into()),
            ..AuthConfig::default()
        };
        assert!(
            bad_issuer
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_AUTH__ISSUER")
        );

        let bad_jwks = AuthConfig {
            issuer: Some("http://localhost:8081/auth/v1/".into()),
            jwks_url: Some("ftp://idp/certs".into()),
            ..AuthConfig::default()
        };
        assert!(
            bad_jwks
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_AUTH__JWKS_URL")
        );

        let blank_audience = AuthConfig {
            issuer: Some("http://localhost:8081/auth/v1/".into()),
            audience: " ".into(),
            ..AuthConfig::default()
        };
        assert!(
            blank_audience
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_AUTH__AUDIENCE")
        );
    }

    #[test]
    fn debug_redacts_database_url() {
        let config =
            Config::from_figment(&figment(&[("database_url", "postgres://u:hunter2@h/x")]))
                .unwrap();
        assert!(!format!("{config:?}").contains("hunter2"));
    }
}
