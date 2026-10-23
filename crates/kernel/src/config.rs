//! Runtime configuration, read from the environment.
//!
//! Every variable is prefixed with `CAMPBOOKS_`; nested keys are separated by
//! a double underscore, e.g. `CAMPBOOKS_AI__DECIDE__MODEL` is `ai.decide.model`.
//! Other crates extract their own sections from [`env_source`] so there is one
//! convention for the whole server.

use std::{net::SocketAddr, path::PathBuf};

use figment::{Figment, providers::Env};
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

    /// Login used only to apply migrations
    /// (`CAMPBOOKS_DATABASE_MIGRATION_URL`): the schema owner, or a
    /// superuser. When set, `database_url` should be a dedicated
    /// non-superuser login that is a member of the runtime role (see
    /// [`crate::db`]). Unset, migrations run as `database_url`. Secret.
    #[serde(default)]
    pub database_migration_url: Option<SecretString>,

    /// File holding the password for `database_url`
    /// (`CAMPBOOKS_DATABASE_PASSWORD_FILE`), so the URL can omit it and the
    /// secret stays out of the environment.
    #[serde(default)]
    pub database_password_file: Option<PathBuf>,

    /// Start even though the runtime login bypasses row-level security
    /// (`CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS`, default `false`). An escape
    /// hatch for setups that cannot yet create a separate login; workspace
    /// isolation then rests on the queries alone.
    #[serde(default)]
    pub database_allow_rls_bypass: bool,

    /// Maximum connections in the database pool.
    #[serde(default = "defaults::database_max_connections")]
    pub database_max_connections: u32,

    /// The role every pooled connection switches to (`SET ROLE`) before it
    /// runs a query (`CAMPBOOKS_DATABASE_ROLE`, default `campbooks_app`). It
    /// is subject to row-level security even when the login user is a
    /// superuser; migrations still run as the login user. Set it empty to
    /// query as the login user (which must then not bypass row-level
    /// security). See [`crate::db`].
    #[serde(default = "defaults::database_role")]
    pub database_role: String,

    /// Which kind of deployment this is (`CAMPBOOKS_ENV`, `production` by
    /// default). `development` enables conveniences that must never run in
    /// production, such as logging invitation links.
    #[serde(default)]
    pub env: Environment,

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

    /// The background job worker (`CAMPBOOKS_JOBS__*`).
    #[serde(default)]
    pub jobs: JobsConfig,

    /// Who may create a first workspace on this instance
    /// (`CAMPBOOKS_SIGNUP__*`).
    #[serde(default)]
    pub signup: SignupConfig,

    /// Where message bodies and files are stored (`CAMPBOOKS_BLOBS__*`).
    #[serde(default)]
    pub blobs: BlobsConfig,
    /// The client change feed (`CAMPBOOKS_CHANGES__*`).
    #[serde(default)]
    pub changes: ChangesConfig,

    /// The instance's own mail (`CAMPBOOKS_MAIL__*`); none by default.
    #[serde(default)]
    pub mail: crate::mail::MailConfig,
    /// The event log (`CAMPBOOKS_EVENTS__*`).
    #[serde(default)]
    pub events: EventsConfig,

    /// Traces and metrics export (`CAMPBOOKS_TELEMETRY__*`); off by default.
    #[serde(default)]
    pub telemetry: TelemetryConfig,

    /// Reverse proxies whose `X-Forwarded-For` names the client, for rate
    /// limits (`CAMPBOOKS_TRUSTED_PROXIES`, comma-separated addresses or CIDR
    /// blocks). Empty by default: the socket peer is the client.
    #[serde(default)]
    pub trusted_proxies: crate::rate_limit::TrustedProxies,

    /// Per-caller limits of the authenticated API
    /// (`CAMPBOOKS_RATE_LIMITS__*`; [`rate_limit::api`](crate::rate_limit::api)).
    #[serde(default)]
    pub rate_limits: RateLimitsConfig,

    /// Usage analytics: the screens the app shows, relayed to a Matomo
    /// instance (`CAMPBOOKS_ANALYTICS__*`); off by default.
    #[serde(default)]
    pub analytics: AnalyticsConfig,

    /// Tracked events (`CAMPBOOKS_TRACKING__*`, ADR 0020); off by default.
    #[serde(default)]
    pub tracking: TrackingConfig,
}

impl Config {
    /// Whether a separate migration login is configured.
    pub fn split_database_logins(&self) -> bool {
        non_blank(
            self.database_migration_url
                .as_ref()
                .map(SecretString::expose_secret),
        )
        .is_some()
    }

    /// The URL migrations connect with: the migration login when one is
    /// configured, otherwise the runtime URL.
    pub fn migration_url(&self) -> &str {
        non_blank(
            self.database_migration_url
                .as_ref()
                .map(SecretString::expose_secret),
        )
        .unwrap_or_else(|| self.database_url.expose_secret())
    }

    /// The runtime password file, `None` when unset or blank.
    pub fn database_password_file(&self) -> Option<&std::path::Path> {
        self.database_password_file
            .as_deref()
            .filter(|p| !p.as_os_str().is_empty())
    }

    /// The runtime database role; `None` when unset or blank (query as the
    /// login user).
    pub fn database_role(&self) -> Option<&str> {
        non_blank(Some(self.database_role.as_str()))
    }

    /// What an instance administrator may see of this configuration: only
    /// enumerated facts, never a secret, password, key, code or a URL's
    /// credentials, path or query (ADR 0015).
    pub fn summary(&self) -> ConfigSummary {
        ConfigSummary {
            environment: self.env,
            public_origin: origin_of(&self.public_url),
            signup_policy: self.signup.policy,
            blob_store: self.blobs.backend,
            database_logins: if self.split_database_logins() {
                DatabaseLogins::Split
            } else {
                DatabaseLogins::Single
            },
            migrations: if self.run_migrations {
                Migrations::OnStart
            } else {
                Migrations::Separately
            },
            serves_web_client: self.web_dir.is_some(),
            exports_telemetry: self.telemetry.otlp_endpoint().is_some(),
            metrics_endpoint: self.telemetry.metrics_addr.is_some(),
        }
    }

    /// Startup check of the settings the API and worker share: the runtime
    /// role is a plain identifier and the signup settings are usable. The
    /// message names the offending setting.
    pub fn check(&self) -> Result<(), String> {
        if let Some(role) = self.database_role()
            && !is_identifier(role)
        {
            return Err(format!(
                "CAMPBOOKS_DATABASE_ROLE={role} must be a lower-case identifier \
                 (letters, digits, underscores)"
            ));
        }
        self.signup.allow_list()?;
        self.changes.check()?;
        self.mail.check()?;
        self.events.check()?;
        self.rate_limits.check()?;
        self.analytics.check()?;
        self.tracking.check()?;
        self.telemetry.check()
    }
}

/// The non-secret facts of a [`Config`] an administrator sees
/// ([`Config::summary`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ConfigSummary {
    /// `CAMPBOOKS_ENV`.
    pub environment: Environment,
    /// The scheme, host and port of `CAMPBOOKS_PUBLIC_URL`.
    pub public_origin: Option<String>,
    /// `CAMPBOOKS_SIGNUP__POLICY` (never the codes).
    pub signup_policy: SignupPolicy,
    /// `CAMPBOOKS_BLOBS__BACKEND`.
    pub blob_store: BlobBackend,
    /// Whether migrations use a login of their own.
    pub database_logins: DatabaseLogins,
    /// When migrations run (`CAMPBOOKS_RUN_MIGRATIONS`).
    pub migrations: Migrations,
    /// Whether the server serves the web client (`CAMPBOOKS_WEB_DIR`).
    pub serves_web_client: bool,
    /// Whether traces and metrics are exported over OTLP.
    pub exports_telemetry: bool,
    /// Whether a Prometheus endpoint is open.
    pub metrics_endpoint: bool,
}

/// How the server signs in to the database (`deploy/README.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseLogins {
    /// One login for requests and migrations (switching to the runtime
    /// role with `SET ROLE`).
    Single,
    /// A dedicated runtime login, and the migration login
    /// (`CAMPBOOKS_DATABASE_MIGRATION_URL`) for migrations only.
    Split,
}

/// When schema migrations run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Migrations {
    /// The server applies them when it starts.
    OnStart,
    /// The operator runs them separately.
    Separately,
}

/// `scheme://host[:port]` of `url`, without credentials, path, query or
/// fragment; `None` when it is not an absolute URL with a host. For showing
/// where a service is without showing anything secret.
pub fn origin_of(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url.trim()).ok()?;
    let host = url.host_str()?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}

/// Telemetry export settings (`CAMPBOOKS_TELEMETRY__*`). With neither
/// [`otlp_endpoint`](Self::otlp_endpoint) nor
/// [`metrics_addr`](Self::metrics_addr) set, nothing is exported and no
/// extra listener opens.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Base URL of an OTLP/HTTP receiver (`CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT`,
    /// e.g. `http://collector:4318`). Traces go to `<url>/v1/traces`, metrics
    /// to `<url>/v1/metrics`. Unset (or empty) means no OTLP export.
    pub otlp_endpoint: Option<String>,
    /// Headers sent with every export (`CAMPBOOKS_TELEMETRY__OTLP_HEADERS`),
    /// as `key=value,key=value`, e.g. an authorization header for a hosted
    /// collector. Secret.
    pub otlp_headers: Option<SecretString>,
    /// Share of new traces kept, from 0 to 1
    /// (`CAMPBOOKS_TELEMETRY__SAMPLE_RATIO`, default 1). A caller's sampling
    /// decision (an incoming `traceparent`) wins.
    pub sample_ratio: f64,
    /// Address of a separate listener serving Prometheus metrics at
    /// `GET /metrics` (`CAMPBOOKS_TELEMETRY__METRICS_ADDR`, e.g.
    /// `0.0.0.0:9464`). Unauthenticated: keep it off the internet. Unset
    /// means no listener.
    pub metrics_addr: Option<SocketAddr>,
    /// DSN of a Sentry-compatible error tracker (`GlitchTip`, Sentry) that
    /// receives the server's panics and errors
    /// (`CAMPBOOKS_TELEMETRY__ERROR_DSN`). Reports hold ids and error kinds
    /// only. Secret. Unset (or empty) means no report leaves the process.
    pub error_dsn: Option<SecretString>,
    /// The deployment error reports and client telemetry are filed under
    /// (`CAMPBOOKS_TELEMETRY__ENVIRONMENT`, e.g. `production`, `staging`;
    /// default `production`).
    pub environment: Option<String>,
    /// Where the server forwards the app's own error and timing reports, a
    /// Grafana Faro receiver (`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL`,
    /// e.g. `http://alloy:12347/collect`). Unset (or empty) means the app's
    /// reports are refused and nothing is forwarded.
    pub client_collector_url: Option<String>,
    /// API key sent to the client collector as `x-api-key`
    /// (`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_KEY`). Secret.
    pub client_collector_key: Option<SecretString>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: None,
            otlp_headers: None,
            sample_ratio: 1.0,
            metrics_addr: None,
            error_dsn: None,
            environment: None,
            client_collector_url: None,
            client_collector_key: None,
        }
    }
}

impl TelemetryConfig {
    /// The OTLP endpoint without a trailing slash; `None` when unset or blank.
    pub fn otlp_endpoint(&self) -> Option<&str> {
        non_blank(self.otlp_endpoint.as_deref()).map(|url| url.trim_end_matches('/'))
    }

    /// The export headers, trimmed, without empty entries.
    pub fn otlp_headers(&self) -> Vec<(String, String)> {
        self.otlp_headers
            .as_ref()
            .map(|headers| {
                headers
                    .expose_secret()
                    .split(',')
                    .filter_map(|pair| pair.split_once('='))
                    .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                    .filter(|(k, _)| !k.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The error tracker's DSN, trimmed; `None` when unset or blank.
    pub fn error_dsn(&self) -> Option<&str> {
        non_blank(
            self.error_dsn
                .as_ref()
                .map(secrecy::ExposeSecret::expose_secret),
        )
    }

    /// The deployment name: [`environment`](Self::environment), else
    /// `production`.
    pub fn environment(&self) -> &str {
        non_blank(self.environment.as_deref()).unwrap_or("production")
    }

    /// The client collector's URL, trimmed; `None` when unset or blank.
    pub fn client_collector_url(&self) -> Option<&str> {
        non_blank(self.client_collector_url.as_deref())
    }

    /// Whether metrics leave the process (OTLP or the Prometheus listener).
    pub fn exports_metrics(&self) -> bool {
        self.otlp_endpoint().is_some() || self.metrics_addr.is_some()
    }

    /// Startup check: the endpoint is an http(s) URL and the ratio is
    /// between 0 and 1. The message names the offending setting (never the
    /// headers' values).
    pub fn check(&self) -> Result<(), String> {
        if let Some(endpoint) = self.otlp_endpoint() {
            check_url("CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT", endpoint)?;
        }
        if let Some(url) = self.client_collector_url() {
            check_url("CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL", url)?;
        }
        if let Some(environment) = non_blank(self.environment.as_deref())
            && !environment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(format!(
                "CAMPBOOKS_TELEMETRY__ENVIRONMENT={environment} must be letters, digits, \
                 '-', '_' or '.'"
            ));
        }
        if !(0.0..=1.0).contains(&self.sample_ratio) {
            return Err(format!(
                "CAMPBOOKS_TELEMETRY__SAMPLE_RATIO={} must be between 0 and 1",
                self.sample_ratio
            ));
        }
        if let Some(headers) = &self.otlp_headers
            && headers
                .expose_secret()
                .split(',')
                .map(str::trim)
                .any(|pair| !pair.is_empty() && !pair.contains('='))
        {
            return Err("CAMPBOOKS_TELEMETRY__OTLP_HEADERS must be key=value pairs \
                        separated by commas"
                .to_owned());
        }
        Ok(())
    }
}

/// Shortest usage analytics visitor key accepted, in characters.
pub const MIN_VISITOR_KEY_CHARS: usize = 32;

/// Usage analytics (`CAMPBOOKS_ANALYTICS__*`): the server relays the screens
/// the app shows, as route templates, to a [Matomo](https://matomo.org)
/// instance's HTTP tracking API. Off unless the URL, the site id and the
/// visitor key are all set (blank counts as unset): self-hosted instances
/// and development send nothing.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AnalyticsConfig {
    /// Base URL of the Matomo instance
    /// (`CAMPBOOKS_ANALYTICS__MATOMO_URL`, e.g.
    /// `https://analytics.example.com`). Hits go to `<url>/matomo.php`.
    pub matomo_url: Option<String>,
    /// The Matomo site the app's screens are recorded under
    /// (`CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID`, a positive whole number).
    #[serde(deserialize_with = "text_or_number")]
    pub matomo_site_id: Option<String>,
    /// Key of the daily visitor id
    /// (`CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY`, at least
    /// [`MIN_VISITOR_KEY_CHARS`] characters, e.g. from
    /// `openssl rand -base64 32`): Matomo sees an HMAC of the account and
    /// the day under it, never the account. Secret.
    pub matomo_visitor_key: Option<SecretString>,
}

/// The Matomo settings of an [`AnalyticsConfig`] with all three set
/// ([`AnalyticsConfig::matomo`]). Its `Debug` hides the visitor key.
#[derive(Clone, Copy)]
pub struct Matomo<'a> {
    /// The base URL, without a trailing slash.
    pub url: &'a str,
    /// The site id.
    pub site_id: &'a str,
    visitor_key: &'a str,
}

impl Matomo<'_> {
    /// The visitor key's bytes, for the HMAC of the daily visitor id.
    pub fn visitor_key(&self) -> &[u8] {
        self.visitor_key.as_bytes()
    }
}

impl std::fmt::Debug for Matomo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Matomo")
            .field("url", &self.url)
            .field("site_id", &self.site_id)
            .field("visitor_key", &"<redacted>")
            .finish()
    }
}

impl AnalyticsConfig {
    /// The Matomo settings when the URL, the site id and the visitor key are
    /// all set; `None` (analytics off) otherwise.
    pub fn matomo(&self) -> Option<Matomo<'_>> {
        Some(Matomo {
            url: non_blank(self.matomo_url.as_deref())?.trim_end_matches('/'),
            site_id: non_blank(self.matomo_site_id.as_deref())?,
            visitor_key: non_blank(
                self.matomo_visitor_key
                    .as_ref()
                    .map(SecretString::expose_secret),
            )?,
        })
    }

    /// Startup check of the settings that are set: an http(s) URL, a
    /// positive site id and a long enough key. The message names the
    /// offending setting, never the key.
    pub fn check(&self) -> Result<(), String> {
        if let Some(url) = non_blank(self.matomo_url.as_deref()) {
            check_url("CAMPBOOKS_ANALYTICS__MATOMO_URL", url)?;
        }
        if let Some(site_id) = non_blank(self.matomo_site_id.as_deref())
            && !site_id.parse::<u32>().is_ok_and(|id| id > 0)
        {
            return Err(format!(
                "CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID={site_id} must be a positive whole number"
            ));
        }
        if let Some(key) = non_blank(
            self.matomo_visitor_key
                .as_ref()
                .map(SecretString::expose_secret),
        ) && key.chars().count() < MIN_VISITOR_KEY_CHARS
        {
            return Err(format!(
                "CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY must be at least \
                 {MIN_VISITOR_KEY_CHARS} characters (e.g. `openssl rand -base64 32`)"
            ));
        }
        Ok(())
    }
}

/// A setting that may arrive as text or, from the environment, as a number
/// (`2`): kept as text.
fn text_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TextOrNumber {
        Text(String),
        Number(u64),
    }
    Ok(
        Option::<TextOrNumber>::deserialize(deserializer)?.map(|value| match value {
            TextOrNumber::Text(text) => text,
            TextOrNumber::Number(number) => number.to_string(),
        }),
    )
}

/// The kind of deployment (`CAMPBOOKS_ENV`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    /// A real deployment: no development conveniences.
    #[default]
    Production,
    /// A developer's machine.
    Development,
}

/// Who may create a workspace when they are not a member of any yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignupPolicy {
    /// Anyone who can sign in.
    #[default]
    Open,
    /// Nobody: people join by invitation.
    InviteOnly,
    /// Nobody until an instance administrator approves them; the request is
    /// recorded.
    Approval,
    /// Whoever presents a valid signup code: one configured in the
    /// environment, or one an administrator minted.
    Code,
}

/// Signup settings (`CAMPBOOKS_SIGNUP__*`). The policy applies only to a user
/// with no workspace creating one (the instance's first user is always
/// allowed); `allowed` applies to every sign-in.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SignupConfig {
    /// `CAMPBOOKS_SIGNUP__POLICY`: `open` (default), `invite_only`,
    /// `approval` or `code`.
    pub policy: SignupPolicy,
    /// `CAMPBOOKS_SIGNUP__CODES`: comma-separated codes accepted under the
    /// `code` policy. Each can be used any number of times. Secret. This is
    /// the legacy path: administrators mint single-use codes in the admin
    /// section, which need no setting.
    pub codes: Option<SecretString>,
    /// `CAMPBOOKS_SIGNUP__ALLOWED`: comma-separated addresses and domains
    /// allowed to sign in or create an account at all (see [`AllowList`]).
    /// Unset or empty means no restriction.
    pub allowed: Option<String>,
}

impl SignupConfig {
    /// The parsed allow-list ([`AllowList::default`], which lets everyone
    /// in, when unset or blank).
    pub fn allow_list(&self) -> Result<AllowList, String> {
        AllowList::parse(self.allowed.as_deref().unwrap_or_default())
    }

    /// The configured codes, trimmed, without empty entries.
    pub fn codes(&self) -> Vec<&str> {
        self.codes
            .as_ref()
            .map(|codes| {
                codes
                    .expose_secret()
                    .split(',')
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Who may sign in at all (`CAMPBOOKS_SIGNUP__ALLOWED`): a comma-separated
/// list where an entry with an `@` is one exact address and any other entry
/// is a domain matching `*@domain` exactly (no subdomains). Case-insensitive;
/// entries are trimmed and empty ones ignored. No entries means no
/// restriction. It applies on top of [`SignupPolicy`] and invitations never
/// bypass it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowList {
    addresses: Vec<String>,
    domains: Vec<String>,
}

impl AllowList {
    /// Parse `value`; the error names the offending entry's position, not
    /// its text.
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut list = Self::default();
        for (n, entry) in value
            .split(',')
            .map(str::trim)
            .enumerate()
            .filter(|(_, e)| !e.is_empty())
        {
            let entry = entry.to_lowercase();
            let bad = |what: &str| {
                format!(
                    "CAMPBOOKS_SIGNUP__ALLOWED entry {} is not {what} \
                     (write one address or one domain per entry)",
                    n + 1
                )
            };
            if let Some((local, domain)) = entry.split_once('@') {
                if local.is_empty()
                    || local.chars().any(|c| c.is_whitespace() || c == '@')
                    || !is_domain(domain)
                {
                    return Err(bad("an address"));
                }
                list.addresses.push(entry);
            } else {
                if !is_domain(&entry) {
                    return Err(bad("a domain"));
                }
                list.domains.push(entry);
            }
        }
        Ok(list)
    }

    /// Whether any entry is set, so the list restricts anyone.
    pub fn is_restricted(&self) -> bool {
        !self.addresses.is_empty() || !self.domains.is_empty()
    }

    /// How many entries it holds (for counts in logs, never the values).
    pub fn len(&self) -> usize {
        self.addresses.len() + self.domains.len()
    }

    /// Whether it holds no entries.
    pub fn is_empty(&self) -> bool {
        !self.is_restricted()
    }

    /// Whether `email` may get in. An unrestricted list lets everyone in
    /// (even no address); a restricted one only a matching address.
    pub fn permits(&self, email: Option<&str>) -> bool {
        if !self.is_restricted() {
            return true;
        }
        let Some(email) = email.map(|e| e.trim().to_lowercase()) else {
            return false;
        };
        if self.addresses.contains(&email) {
            return true;
        }
        match email.rsplit_once('@') {
            Some((local, domain)) if !local.is_empty() => self.domains.iter().any(|d| d == domain),
            _ => false,
        }
    }
}

/// A plausible DNS name: dot-separated labels of letters, digits and inner
/// hyphens, at least two labels.
fn is_domain(value: &str) -> bool {
    let mut labels = 0;
    for label in value.split('.') {
        labels += 1;
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return false;
        }
    }
    labels >= 2
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && value.len() <= 63
}

/// Which object store holds the blobs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BlobBackend {
    /// A directory on local disk (`CAMPBOOKS_BLOBS__PATH`); the self-host
    /// default.
    #[default]
    Fs,
    /// Any S3-compatible bucket (`CAMPBOOKS_BLOBS__S3__*`).
    S3,
}

/// Blob storage settings (`CAMPBOOKS_BLOBS__*`, ADR 0011): message bodies,
/// attachments and files live outside Postgres.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BlobsConfig {
    /// `fs` (default) or `s3` (`CAMPBOOKS_BLOBS__BACKEND`).
    pub backend: BlobBackend,
    /// The directory of the `fs` backend (`CAMPBOOKS_BLOBS__PATH`), created
    /// on startup if missing. The image mounts the `blobs-data` volume here.
    pub path: PathBuf,
    /// The bucket of the `s3` backend (`CAMPBOOKS_BLOBS__S3__*`).
    pub s3: S3Config,
    /// Largest blob accepted, in bytes (`CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`).
    pub max_object_bytes: u64,
    /// How long a blob nothing references is kept before the sweeper deletes
    /// it, counted from its last upload or detach
    /// (`CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS`).
    pub sweep_grace_hours: u64,
}

impl Default for BlobsConfig {
    fn default() -> Self {
        Self {
            backend: BlobBackend::Fs,
            path: PathBuf::from("/data/blobs"),
            s3: S3Config::default(),
            max_object_bytes: 50 * 1024 * 1024,
            sweep_grace_hours: 24,
        }
    }
}

/// An S3-compatible bucket (`CAMPBOOKS_BLOBS__S3__*`): AWS S3, Garage,
/// `MinIO`, Ceph, Cloudflare R2 and the like.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct S3Config {
    /// Bucket name (`CAMPBOOKS_BLOBS__S3__BUCKET`); required.
    pub bucket: Option<String>,
    /// The store's URL (`CAMPBOOKS_BLOBS__S3__ENDPOINT`, e.g.
    /// `https://s3.example.com`); unset means AWS. Plain `http://` is
    /// accepted, for a store on a private network.
    pub endpoint: Option<String>,
    /// Region (`CAMPBOOKS_BLOBS__S3__REGION`); default `us-east-1`, which
    /// most other stores accept as well.
    pub region: Option<String>,
    /// Access key id (`CAMPBOOKS_BLOBS__S3__ACCESS_KEY_ID`). With the secret
    /// unset too, credentials come from the environment's instance role.
    pub access_key_id: Option<String>,
    /// Secret access key (`CAMPBOOKS_BLOBS__S3__SECRET_ACCESS_KEY`).
    pub secret_access_key: Option<SecretString>,
    /// Address the bucket as `<endpoint>/<bucket>` instead of
    /// `<bucket>.<endpoint>` (`CAMPBOOKS_BLOBS__S3__PATH_STYLE`). Most
    /// self-hosted stores need `true`.
    pub path_style: bool,
}

impl S3Config {
    /// The configured endpoint; `None` when unset or blank.
    pub fn endpoint(&self) -> Option<&str> {
        non_blank(self.endpoint.as_deref())
    }

    /// The configured region, or `us-east-1`.
    pub fn region(&self) -> &str {
        non_blank(self.region.as_deref()).unwrap_or("us-east-1")
    }
}

impl BlobsConfig {
    /// Startup check: limits are positive and the chosen backend has what it
    /// needs. The message names the offending setting.
    pub fn check(&self) -> Result<(), String> {
        if self.max_object_bytes == 0 {
            return Err("CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES must be at least 1".to_owned());
        }
        if self.sweep_grace_hours == 0 {
            return Err("CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS must be at least 1".to_owned());
        }
        match self.backend {
            BlobBackend::Fs if self.path.as_os_str().is_empty() => {
                Err("CAMPBOOKS_BLOBS__PATH must not be empty".to_owned())
            }
            BlobBackend::Fs => Ok(()),
            BlobBackend::S3 => self.s3.check(),
        }
    }
}

impl S3Config {
    fn check(&self) -> Result<(), String> {
        if non_blank(self.bucket.as_deref()).is_none() {
            return Err("CAMPBOOKS_BLOBS__S3__BUCKET must be set when \
                        CAMPBOOKS_BLOBS__BACKEND=s3"
                .to_owned());
        }
        if let Some(endpoint) = self.endpoint() {
            check_url("CAMPBOOKS_BLOBS__S3__ENDPOINT", endpoint)?;
        }
        let key = non_blank(self.access_key_id.as_deref()).is_some();
        let secret = self
            .secret_access_key
            .as_ref()
            .is_some_and(|s| !s.expose_secret().trim().is_empty());
        if key != secret {
            return Err("CAMPBOOKS_BLOBS__S3__ACCESS_KEY_ID and \
                        CAMPBOOKS_BLOBS__S3__SECRET_ACCESS_KEY must be set together"
                .to_owned());
        }
        Ok(())
    }
}

/// The port a worker-only process answers its health probe on by default.
pub const DEFAULT_WORKER_HEALTH_PORT: u16 = 8081;

/// Background job worker settings (`CAMPBOOKS_JOBS__*`, ADR 0012).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct JobsConfig {
    /// Jobs one worker process runs at the same time
    /// (`CAMPBOOKS_JOBS__CONCURRENCY`). Each running job may hold a database
    /// connection, so keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` above it.
    pub concurrency: usize,
    /// Jobs of one workspace that may run at once in one queue, across all
    /// workers (`CAMPBOOKS_JOBS__PER_WORKSPACE`). Keeps one busy workspace
    /// from starving the others.
    pub per_workspace: u32,
    /// How often an idle worker looks for work when no enqueue notification
    /// arrives (`CAMPBOOKS_JOBS__POLL_INTERVAL_MS`). Enqueues wake workers
    /// at once through `LISTEN/NOTIFY`; the poll is the safety net.
    pub poll_interval_ms: u64,
    /// How long a claimed job is reserved for its worker
    /// (`CAMPBOOKS_JOBS__LEASE_SECONDS`). The worker renews it every third of
    /// that while the job runs; a job whose lease runs out (its worker died)
    /// is handed to another worker.
    pub lease_seconds: u64,
    /// How long succeeded jobs are kept before the hourly prune deletes them
    /// (`CAMPBOOKS_JOBS__RETENTION_HOURS`).
    pub retention_hours: u64,
    /// How long failed jobs are kept for inspection before the hourly prune
    /// deletes them (`CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS`, default 30).
    pub failed_retention_days: u64,
    /// How long a stopping worker waits for running jobs to finish before it
    /// hands them back to the queue (`CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS`).
    /// Keep it below the container stop timeout (10 s by default).
    pub shutdown_grace_seconds: u64,
    /// First retry delay in milliseconds (`CAMPBOOKS_JOBS__RETRY_BASE_MS`);
    /// it doubles with every attempt, with jitter.
    pub retry_base_ms: u64,
    /// Longest retry delay in seconds (`CAMPBOOKS_JOBS__RETRY_MAX_SECONDS`).
    pub retry_max_seconds: u64,
    /// Where a worker-only process (`campbooks-server worker`) answers its
    /// health probe, `GET /healthz` (`CAMPBOOKS_JOBS__HEALTH_ADDR`, default
    /// `127.0.0.1:8081`, inside the container only); `campbooks-server
    /// healthcheck --worker` probes it (campbooks-cloud#185). A process that
    /// serves the API answers `/api/v1/system/health` instead.
    pub health_addr: std::net::SocketAddr,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            per_workspace: 2,
            poll_interval_ms: 5_000,
            lease_seconds: 60,
            retention_hours: 168,
            failed_retention_days: 30,
            shutdown_grace_seconds: 8,
            retry_base_ms: 1_000,
            retry_max_seconds: 3_600,
            health_addr: std::net::SocketAddr::from(([127, 0, 0, 1], DEFAULT_WORKER_HEALTH_PORT)),
        }
    }
}

impl JobsConfig {
    /// Startup check: every value is in a range the worker can run with. The
    /// message names the offending setting.
    pub fn check(&self) -> Result<(), String> {
        let checks = [
            ("CONCURRENCY", self.concurrency >= 1, "at least 1"),
            ("PER_WORKSPACE", self.per_workspace >= 1, "at least 1"),
            (
                "POLL_INTERVAL_MS",
                self.poll_interval_ms >= 10,
                "at least 10",
            ),
            ("LEASE_SECONDS", self.lease_seconds >= 3, "at least 3"),
            ("RETENTION_HOURS", self.retention_hours >= 1, "at least 1"),
            (
                "FAILED_RETENTION_DAYS",
                self.failed_retention_days >= 1,
                "at least 1",
            ),
            ("RETRY_BASE_MS", self.retry_base_ms >= 1, "at least 1"),
            (
                "RETRY_MAX_SECONDS",
                self.retry_max_seconds.saturating_mul(1_000) >= self.retry_base_ms,
                "at least RETRY_BASE_MS",
            ),
        ];
        match checks.iter().find(|(_, ok, _)| !ok) {
            Some((name, _, rule)) => Err(format!("CAMPBOOKS_JOBS__{name} must be {rule}")),
            None => Ok(()),
        }
    }
}

/// Where tracked events go (`CAMPBOOKS_TRACKING__SINK`, ADR 0020).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackingSink {
    /// Nowhere: `track` returns at once and nothing else runs (the default).
    #[default]
    None,
    /// The outbox `kernel.tracked_events`, in the transaction of the change;
    /// nothing publishes it (for tests and inspection).
    Outbox,
    /// The outbox, published to RabbitMQ by the worker's relay
    /// (`CAMPBOOKS_TRACKING__AMQP_URL`, campbooks-cloud#321).
    Amqp,
}

/// Tracked events settings (`CAMPBOOKS_TRACKING__*`, ADR 0020). Its `Debug`
/// never shows the AMQP URL (a secret: it carries the broker's password).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TrackingConfig {
    /// Where tracked events go (`CAMPBOOKS_TRACKING__SINK`: `none`, the
    /// default, `outbox` or `amqp`).
    pub sink: TrackingSink,
    /// The broker the relay publishes to (`CAMPBOOKS_TRACKING__AMQP_URL`,
    /// `amqp://user:password@host:5672/%2f` or `amqps://…`), required with
    /// the sink `amqp`. Secret: never logged, not even its host.
    pub amqp_url: Option<SecretString>,
    /// The topic exchange events are published to
    /// (`CAMPBOOKS_TRACKING__EXCHANGE`, default `campbooks.events`), with
    /// routing key `<name>.v<version>`.
    pub exchange: String,
    /// The stream queue bound to the exchange when the relay declares its
    /// topology (`CAMPBOOKS_TRACKING__STREAM`, default `campbooks.events`).
    pub stream: String,
    /// How long the stream keeps events (`CAMPBOOKS_TRACKING__STREAM_MAX_AGE`,
    /// the stream's `x-max-age`: a number and a unit among `Y`, `M`, `D`, `h`,
    /// `m`, `s`; default `30D`).
    pub stream_max_age: String,
    /// Declare the exchange, the stream and their binding at start
    /// (`CAMPBOOKS_TRACKING__DECLARE_TOPOLOGY`, default `true`). Off, the
    /// relay only publishes, which suits a broker user that may write but
    /// not configure.
    pub declare_topology: bool,
    /// Events per batch, published and confirmed in one transaction
    /// (`CAMPBOOKS_TRACKING__BATCH_SIZE`, 1 to 5000, default 200).
    pub batch_size: u32,
    /// How often an idle relay looks for events when no notification arrives
    /// (`CAMPBOOKS_TRACKING__POLL_INTERVAL_MS`, at least 50, default 1000).
    pub poll_interval_ms: u64,
    /// How long published rows stay in the outbox before the hourly prune
    /// deletes them (`CAMPBOOKS_TRACKING__PUBLISHED_RETENTION_HOURS`, at
    /// least 1, default 24). Unpublished rows are never pruned.
    pub published_retention_hours: u64,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            sink: TrackingSink::None,
            amqp_url: None,
            exchange: "campbooks.events".to_owned(),
            stream: "campbooks.events".to_owned(),
            stream_max_age: "30D".to_owned(),
            declare_topology: true,
            batch_size: 200,
            poll_interval_ms: 1_000,
            published_retention_hours: 24,
        }
    }
}

impl TrackingConfig {
    /// Most events per batch.
    pub const MAX_BATCH_SIZE: u32 = 5_000;
    /// Shortest poll interval accepted, in milliseconds.
    pub const MIN_POLL_INTERVAL_MS: u64 = 50;

    /// The AMQP URL, `None` when unset or blank. Never log it.
    pub fn amqp_url(&self) -> Option<&str> {
        non_blank(self.amqp_url.as_ref().map(SecretString::expose_secret))
    }

    /// Startup check: with the sink `amqp`, an `amqp` or `amqps` URL; names
    /// AMQP accepts; a stream age RabbitMQ understands; usable numbers. The
    /// message names the offending setting, never the URL or any part of it.
    pub fn check(&self) -> Result<(), String> {
        if self.sink == TrackingSink::Amqp {
            let Some(url) = self.amqp_url() else {
                return Err("CAMPBOOKS_TRACKING__AMQP_URL must be set when \
                            CAMPBOOKS_TRACKING__SINK=amqp"
                    .to_owned());
            };
            let scheme_ok = url::Url::parse(url)
                .is_ok_and(|u| matches!(u.scheme(), "amqp" | "amqps") && u.host_str().is_some());
            if !scheme_ok {
                return Err(
                    "CAMPBOOKS_TRACKING__AMQP_URL must be an amqp:// or amqps:// URL \
                            with a host"
                        .to_owned(),
                );
            }
        }
        for (name, value) in [("EXCHANGE", &self.exchange), ("STREAM", &self.stream)] {
            if !is_amqp_name(value) {
                return Err(format!(
                    "CAMPBOOKS_TRACKING__{name}={value} must be 1 to 255 letters, digits, \
                     dots, dashes, underscores or colons"
                ));
            }
        }
        if !is_stream_age(&self.stream_max_age) {
            return Err(format!(
                "CAMPBOOKS_TRACKING__STREAM_MAX_AGE={} must be a positive number followed by \
                 Y, M, D, h, m or s (e.g. 30D)",
                self.stream_max_age
            ));
        }
        let checks = [
            (
                "BATCH_SIZE",
                (1..=Self::MAX_BATCH_SIZE).contains(&self.batch_size),
                "1 to 5000",
            ),
            (
                "POLL_INTERVAL_MS",
                self.poll_interval_ms >= Self::MIN_POLL_INTERVAL_MS,
                "at least 50",
            ),
            (
                "PUBLISHED_RETENTION_HOURS",
                self.published_retention_hours >= 1,
                "at least 1",
            ),
        ];
        match checks.iter().find(|(_, ok, _)| !ok) {
            Some((name, _, rule)) => Err(format!("CAMPBOOKS_TRACKING__{name} must be {rule}")),
            None => Ok(()),
        }
    }
}

/// An exchange or queue name AMQP 0-9-1 accepts (a short string) and that
/// reads well in logs: letters, digits, `.`, `-`, `_` and `:`.
fn is_amqp_name(name: &str) -> bool {
    (1..=255).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':'))
}

/// A RabbitMQ stream `x-max-age`: digits, then one unit.
fn is_stream_age(age: &str) -> bool {
    let Some((digits, unit)) = age.split_at_checked(age.len().saturating_sub(1)) else {
        return false;
    };
    matches!(unit, "Y" | "M" | "D" | "h" | "m" | "s")
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && digits.parse::<u64>().is_ok_and(|n| n > 0)
}

/// Event log settings (`CAMPBOOKS_EVENTS__*`, ADR 0012).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct EventsConfig {
    /// How long published events are kept before the hourly prune deletes
    /// them (`CAMPBOOKS_EVENTS__RETENTION_DAYS`, default 90). Replaying a
    /// consumer reaches back this far at most; an event still waiting to be
    /// delivered is never pruned.
    pub retention_days: u64,
}

impl Default for EventsConfig {
    fn default() -> Self {
        Self { retention_days: 90 }
    }
}

impl EventsConfig {
    /// Startup check: the retention is at least a day.
    pub fn check(&self) -> Result<(), String> {
        if self.retention_days >= 1 {
            Ok(())
        } else {
            Err("CAMPBOOKS_EVENTS__RETENTION_DAYS must be at least 1".to_owned())
        }
    }
}

/// Client change feed settings (`CAMPBOOKS_CHANGES__*`, ADR 0011).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ChangesConfig {
    /// How often an idle stream sends a heartbeat
    /// (`CAMPBOOKS_CHANGES__HEARTBEAT_SECONDS`). Keep it below the idle
    /// timeout of any proxy in front of the server.
    pub heartbeat_seconds: u64,
    /// Longest a stream stays open before it ends cleanly and the client
    /// reconnects with its cursor (`CAMPBOOKS_CHANGES__MAX_CONNECTION_SECONDS`).
    /// A stream also ends when the caller's access token expires.
    pub max_connection_seconds: u64,
    /// How often an idle stream looks for changes when no notification
    /// arrives (`CAMPBOOKS_CHANGES__POLL_INTERVAL_MS`); notifications wake
    /// streams at once, the poll is the safety net.
    pub poll_interval_ms: u64,
    /// How long changes are kept (`CAMPBOOKS_CHANGES__RETENTION_HOURS`);
    /// a client resuming with a token older than this (less a few minutes)
    /// is told to `reset`. Only the token's age decides, never how many
    /// changes came after it, so a client learns nothing of changes it may
    /// not see.
    pub retention_hours: u64,
    /// Streams one caller may hold open at once, per server process
    /// (`CAMPBOOKS_CHANGES__MAX_STREAMS_PER_PRINCIPAL`); more answer 429.
    pub max_streams_per_principal: usize,
    /// Streams one caller (a person, or a client) may open per window,
    /// across every process (`CAMPBOOKS_CHANGES__OPENS_PER_WINDOW`); more
    /// answer 429 `rate_limited`. A stream resumed from an old token reads
    /// up to the retention's worth of changes, so a client reconnecting in
    /// a loop is held back here. 0 turns the limit off.
    pub opens_per_window: u32,
    /// The window of `OPENS_PER_WINDOW`, in seconds
    /// (`CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS`).
    pub opens_window_seconds: u64,
    /// The catch-up tick, in milliseconds
    /// (`CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS`, 50 to 1000): a stream behind
    /// the head sends one page per tick, each a tick after its read began,
    /// so when a page leaves does not depend on how many changes the caller
    /// may not see it read past. A stream that caught up sends a new
    /// change at once.
    pub catch_up_tick_ms: u64,
}

impl Default for ChangesConfig {
    fn default() -> Self {
        Self {
            heartbeat_seconds: 25,
            max_connection_seconds: 900,
            poll_interval_ms: 5_000,
            retention_hours: 72,
            max_streams_per_principal: 8,
            opens_per_window: 30,
            opens_window_seconds: 60,
            catch_up_tick_ms: 50,
        }
    }
}

impl ChangesConfig {
    /// Shortest catch-up tick accepted, in milliseconds.
    pub const MIN_CATCH_UP_TICK_MS: u64 = 50;
    /// Longest catch-up tick accepted, in milliseconds.
    pub const MAX_CATCH_UP_TICK_MS: u64 = 1_000;

    /// Startup check: every value is in a usable range. The message names
    /// the offending setting.
    pub fn check(&self) -> Result<(), String> {
        let checks = [
            (
                "HEARTBEAT_SECONDS",
                self.heartbeat_seconds >= 1,
                "at least 1",
            ),
            (
                "MAX_CONNECTION_SECONDS",
                self.max_connection_seconds >= self.heartbeat_seconds,
                "at least HEARTBEAT_SECONDS",
            ),
            (
                "POLL_INTERVAL_MS",
                self.poll_interval_ms >= 10,
                "at least 10",
            ),
            ("RETENTION_HOURS", self.retention_hours >= 1, "at least 1"),
            (
                "MAX_STREAMS_PER_PRINCIPAL",
                self.max_streams_per_principal >= 1,
                "at least 1",
            ),
            // Counted by `kernel.take_rate_limit`, which refuses a window
            // longer than a day (campbooks-cloud#213): a longer one would
            // fail every stream's open.
            (
                "OPENS_WINDOW_SECONDS",
                (1..=RateLimitsConfig::MAX_WINDOW_SECONDS).contains(&self.opens_window_seconds),
                "between 1 and 86400",
            ),
            // Shorter would let a long scan show through the tick; longer
            // than a second makes catching up slow (and could pass the
            // shortest heartbeat).
            (
                "CATCH_UP_TICK_MS",
                (Self::MIN_CATCH_UP_TICK_MS..=Self::MAX_CATCH_UP_TICK_MS)
                    .contains(&self.catch_up_tick_ms),
                "between 50 and 1000",
            ),
        ];
        match checks.iter().find(|(_, ok, _)| !ok) {
            Some((name, _, rule)) => Err(format!("CAMPBOOKS_CHANGES__{name} must be {rule}")),
            None => Ok(()),
        }
    }
}

/// Per-caller limits of the authenticated API (`CAMPBOOKS_RATE_LIMITS__*`,
/// [`rate_limit::api`](crate::rate_limit::api)). Each class allows a count
/// per window; a count of 0 turns that class off. Reads, writes and admin
/// calls are counted per server process; sends across all of them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RateLimitsConfig {
    /// Whether any per-caller limit applies (`CAMPBOOKS_RATE_LIMITS__ENABLED`,
    /// default `true`).
    pub enabled: bool,
    /// Reads per window (`CAMPBOOKS_RATE_LIMITS__READS`, default 600).
    pub reads: u32,
    /// The read window (`CAMPBOOKS_RATE_LIMITS__READS_WINDOW_SECONDS`, 60).
    pub reads_window_seconds: u64,
    /// Writes per window (`CAMPBOOKS_RATE_LIMITS__WRITES`, default 120).
    pub writes: u32,
    /// The write window (`CAMPBOOKS_RATE_LIMITS__WRITES_WINDOW_SECONDS`, 60).
    pub writes_window_seconds: u64,
    /// Mail sends per window, per person across all their apps
    /// (`CAMPBOOKS_RATE_LIMITS__SENDS`, default 30).
    pub sends: u32,
    /// The send window (`CAMPBOOKS_RATE_LIMITS__SENDS_WINDOW_SECONDS`, 600).
    pub sends_window_seconds: u64,
    /// Admin API calls per window (`CAMPBOOKS_RATE_LIMITS__ADMIN`, default
    /// 120).
    pub admin: u32,
    /// The admin window (`CAMPBOOKS_RATE_LIMITS__ADMIN_WINDOW_SECONDS`, 60).
    pub admin_window_seconds: u64,
}

impl Default for RateLimitsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reads: 600,
            reads_window_seconds: 60,
            writes: 120,
            writes_window_seconds: 60,
            sends: 30,
            sends_window_seconds: 600,
            admin: 120,
            admin_window_seconds: 60,
        }
    }
}

impl RateLimitsConfig {
    /// Longest window accepted, in seconds (a day).
    pub const MAX_WINDOW_SECONDS: u64 = 86_400;

    /// No per-caller limit at all (`ENABLED=false`).
    pub fn off() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Startup check: every window is between a second and a day. The
    /// message names the offending setting.
    pub fn check(&self) -> Result<(), String> {
        let windows = [
            ("READS_WINDOW_SECONDS", self.reads_window_seconds),
            ("WRITES_WINDOW_SECONDS", self.writes_window_seconds),
            ("SENDS_WINDOW_SECONDS", self.sends_window_seconds),
            ("ADMIN_WINDOW_SECONDS", self.admin_window_seconds),
        ];
        match windows
            .iter()
            .find(|(_, seconds)| !(1..=Self::MAX_WINDOW_SECONDS).contains(seconds))
        {
            Some((name, _)) => Err(format!(
                "CAMPBOOKS_RATE_LIMITS__{name} must be between 1 and {}",
                Self::MAX_WINDOW_SECONDS
            )),
            None => Ok(()),
        }
    }
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

    pub fn database_role() -> String {
        crate::db::DEFAULT_RUNTIME_ROLE.to_owned()
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
    fn the_summary_never_holds_a_secret() {
        let mut config = Config::from_figment(&figment(&[(
            "database_url",
            "postgres://app:db-pass-1@db.example.com/campbooks",
        )]))
        .unwrap();
        config.database_migration_url = Some(SecretString::from(
            "postgres://owner:db-pass-2@db.example.com/campbooks",
        ));
        "https://user:url-pass-3@app.example.com:8443/x?k=url-key-4"
            .clone_into(&mut config.public_url);
        config.signup.policy = SignupPolicy::Code;
        config.signup.codes = Some(SecretString::from("signup-code-5,signup-code-6"));
        config.blobs.backend = BlobBackend::S3;
        config.blobs.s3.access_key_id = Some("access-key-7".to_owned());
        config.blobs.s3.secret_access_key = Some(SecretString::from("secret-key-8"));
        config.telemetry.otlp_headers =
            Some(SecretString::from("authorization=Bearer otlp-token-9"));
        config.auth.issuer = Some("https://id.example.com".to_owned());
        let summary = config.summary();
        assert_eq!(
            summary.public_origin.as_deref(),
            Some("https://app.example.com:8443")
        );
        assert_eq!(summary.signup_policy, SignupPolicy::Code);
        assert_eq!(summary.blob_store, BlobBackend::S3);
        assert_eq!(summary.database_logins, DatabaseLogins::Split);
        assert_eq!(summary.migrations, Migrations::OnStart);
        let json = serde_json::to_string(&summary).unwrap();
        for planted in [
            "db-pass-1",
            "db-pass-2",
            "url-pass-3",
            "url-key-4",
            "signup-code-5",
            "signup-code-6",
            "access-key-7",
            "secret-key-8",
            "otlp-token-9",
            "user",
        ] {
            assert!(
                !json.contains(planted),
                "a planted configuration value leaked into the summary"
            );
        }
    }

    #[test]
    fn origins_drop_credentials_paths_and_queries() {
        assert_eq!(
            origin_of("http://u:p@ollama:11434/v1?key=x").as_deref(),
            Some("http://ollama:11434")
        );
        assert_eq!(
            origin_of(" https://api.example.com/v1 ").as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(
            origin_of("http://[::1]:8000/").as_deref(),
            Some("http://[::1]:8000")
        );
        assert_eq!(origin_of("not a url"), None);
        assert_eq!(origin_of("mailto:a@example.com"), None);
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
        assert_eq!(config.jobs, JobsConfig::default());
        assert_eq!(config.jobs.failed_retention_days, 30);
        assert_eq!(config.events.retention_days, 90);
        assert!(config.jobs.check().is_ok());
        assert_eq!(config.database_role(), Some("campbooks_app"));
        assert_eq!(config.env, Environment::Production);
        assert_eq!(config.signup.policy, SignupPolicy::Open);
        assert!(config.check().is_ok());
        assert_eq!(config.blobs.backend, BlobBackend::Fs);
        assert_eq!(config.blobs.path, PathBuf::from("/data/blobs"));
        assert_eq!(config.blobs.max_object_bytes, 52_428_800);
        assert_eq!(config.blobs.sweep_grace_hours, 24);
        assert!(config.blobs.check().is_ok());
        assert_eq!(config.changes, ChangesConfig::default());
        assert_eq!(config.changes.max_connection_seconds, 900);
        assert_eq!(config.telemetry.otlp_endpoint(), None);
        assert_eq!(config.telemetry.metrics_addr, None);
        assert!((config.telemetry.sample_ratio - 1.0).abs() < f64::EPSILON);
        assert!(!config.telemetry.exports_metrics());
        assert_eq!(
            config.database_url.expose_secret(),
            "postgres://u:p@db.example.com/campbooks"
        );
    }

    #[test]
    fn migrations_use_their_own_login_when_one_is_set() {
        let config =
            Config::from_figment(&figment(&[("database_url", "postgres://rt@h/d")])).unwrap();
        assert!(!config.split_database_logins());
        assert_eq!(config.migration_url(), "postgres://rt@h/d");
        assert_eq!(config.database_password_file(), None);
        assert!(!config.database_allow_rls_bypass);

        let config = Config::from_figment(&figment(&[
            ("database_url", "postgres://rt@h/d"),
            ("database_migration_url", "postgres://owner:pw@h/d"),
            ("database_password_file", ""),
        ]))
        .unwrap();
        assert!(config.split_database_logins());
        assert_eq!(config.migration_url(), "postgres://owner:pw@h/d");
        assert_eq!(
            config.database_password_file(),
            None,
            "blank counts as unset"
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
    fn retention_is_configurable_and_at_least_a_day() {
        let figment = figment(&[("database_url", "postgres://localhost/x")])
            .merge(Serialized::default(
                "events",
                serde_json::json!({ "retention_days": 30 }),
            ))
            .merge(Serialized::default(
                "jobs",
                serde_json::json!({ "failed_retention_days": 7 }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        assert_eq!(config.events.retention_days, 30);
        assert_eq!(config.jobs.failed_retention_days, 7);
        assert!(config.check().is_ok());

        let mut zero = config.clone();
        zero.events.retention_days = 0;
        assert!(
            zero.check()
                .unwrap_err()
                .contains("CAMPBOOKS_EVENTS__RETENTION_DAYS")
        );
        let jobs = JobsConfig {
            failed_retention_days: 0,
            ..JobsConfig::default()
        };
        assert!(jobs.check().unwrap_err().contains("FAILED_RETENTION_DAYS"));
    }

    #[test]
    fn rate_limits_have_defaults_and_bounded_windows() {
        let defaults =
            Config::from_figment(&figment(&[("database_url", "postgres://localhost/x")]))
                .unwrap()
                .rate_limits;
        assert_eq!(defaults, RateLimitsConfig::default());
        assert!(defaults.enabled);
        assert_eq!((defaults.reads, defaults.reads_window_seconds), (600, 60));
        assert_eq!((defaults.writes, defaults.writes_window_seconds), (120, 60));
        assert_eq!((defaults.sends, defaults.sends_window_seconds), (30, 600));
        assert_eq!((defaults.admin, defaults.admin_window_seconds), (120, 60));

        // `CAMPBOOKS_RATE_LIMITS__WRITES=0` etc. land under `rate_limits`;
        // what is not set keeps its own class's default.
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "rate_limits",
                serde_json::json!({ "enabled": false, "writes": 0, "sends": 5 }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        assert!(!config.rate_limits.enabled);
        assert_eq!(config.rate_limits.writes, 0);
        assert_eq!(config.rate_limits.sends, 5);
        assert_eq!(config.rate_limits.sends_window_seconds, 600);
        assert!(config.check().is_ok());

        for (name, broken) in [
            (
                "READS_WINDOW_SECONDS",
                RateLimitsConfig {
                    reads_window_seconds: 0,
                    ..RateLimitsConfig::default()
                },
            ),
            (
                "SENDS_WINDOW_SECONDS",
                RateLimitsConfig {
                    sends_window_seconds: 86_401,
                    ..RateLimitsConfig::default()
                },
            ),
        ] {
            let mut config = config.clone();
            config.rate_limits = broken;
            assert!(
                config
                    .check()
                    .unwrap_err()
                    .contains(&format!("CAMPBOOKS_RATE_LIMITS__{name}")),
                "{name}"
            );
        }
    }

    #[test]
    fn parses_jobs_section() {
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "jobs",
                serde_json::json!({ "concurrency": 16, "poll_interval_ms": 250 }),
            ));
        let jobs = Config::from_figment(&figment).unwrap().jobs;
        assert_eq!(jobs.concurrency, 16);
        assert_eq!(jobs.poll_interval_ms, 250);
        assert_eq!(jobs.lease_seconds, JobsConfig::default().lease_seconds);
    }

    #[test]
    fn parses_blobs_section() {
        // `CAMPBOOKS_BLOBS__S3__BUCKET` etc. land under `blobs.s3`.
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "blobs",
                serde_json::json!({
                    "backend": "s3",
                    "max_object_bytes": 1024,
                    "s3": {
                        "bucket": "campbooks",
                        "endpoint": "https://s3.example.com",
                        "access_key_id": "key",
                        "secret_access_key": "hunter2",
                        "path_style": true,
                    },
                }),
            ));
        let blobs = Config::from_figment(&figment).unwrap().blobs;
        assert_eq!(blobs.backend, BlobBackend::S3);
        assert_eq!(blobs.max_object_bytes, 1024);
        assert_eq!(blobs.sweep_grace_hours, 24);
        assert_eq!(blobs.s3.bucket.as_deref(), Some("campbooks"));
        assert_eq!(blobs.s3.endpoint(), Some("https://s3.example.com"));
        assert_eq!(blobs.s3.region(), "us-east-1");
        assert!(blobs.s3.path_style);
        assert!(blobs.check().is_ok());
        assert!(!format!("{blobs:?}").contains("hunter2"));
    }

    #[test]
    fn blobs_check_names_the_bad_setting() {
        let s3 = |s3: S3Config| BlobsConfig {
            backend: BlobBackend::S3,
            s3,
            ..BlobsConfig::default()
        };
        let cases = [
            (s3(S3Config::default()), "CAMPBOOKS_BLOBS__S3__BUCKET"),
            (
                s3(S3Config {
                    bucket: Some("b".into()),
                    endpoint: Some("s3.example.com".into()),
                    ..S3Config::default()
                }),
                "CAMPBOOKS_BLOBS__S3__ENDPOINT",
            ),
            (
                s3(S3Config {
                    bucket: Some("b".into()),
                    access_key_id: Some("key".into()),
                    ..S3Config::default()
                }),
                "CAMPBOOKS_BLOBS__S3__SECRET_ACCESS_KEY",
            ),
            (
                BlobsConfig {
                    max_object_bytes: 0,
                    ..BlobsConfig::default()
                },
                "CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES",
            ),
            (
                BlobsConfig {
                    sweep_grace_hours: 0,
                    ..BlobsConfig::default()
                },
                "CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS",
            ),
            (
                BlobsConfig {
                    path: PathBuf::new(),
                    ..BlobsConfig::default()
                },
                "CAMPBOOKS_BLOBS__PATH",
            ),
        ];
        for (config, setting) in cases {
            let err = config.check().unwrap_err();
            assert!(err.contains(setting), "{setting}: {err}");
        }
    }

    #[test]
    fn jobs_check_names_the_bad_setting() {
        let zero = JobsConfig {
            concurrency: 0,
            ..JobsConfig::default()
        };
        assert!(
            zero.check()
                .unwrap_err()
                .contains("CAMPBOOKS_JOBS__CONCURRENCY")
        );
        let short_lease = JobsConfig {
            lease_seconds: 1,
            ..JobsConfig::default()
        };
        assert!(
            short_lease
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_JOBS__LEASE_SECONDS")
        );
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
    fn parses_signup_env_and_role() {
        let figment = figment(&[
            ("database_url", "postgres://localhost/x"),
            ("env", "development"),
            ("database_role", ""),
        ])
        .merge(Serialized::default(
            "signup",
            serde_json::json!({ "policy": "code", "codes": " alpha, ,beta " }),
        ));
        let config = Config::from_figment(&figment).unwrap();
        assert_eq!(config.env, Environment::Development);
        assert_eq!(config.database_role(), None);
        assert_eq!(config.signup.policy, SignupPolicy::Code);
        assert_eq!(config.signup.codes(), ["alpha", "beta"]);
        assert!(config.check().is_ok());
        assert!(!format!("{config:?}").contains("alpha"));
    }

    #[test]
    fn the_allow_list_matches_addresses_and_whole_domains_only() {
        let list = AllowList::parse(" Ana@Example.com, ,example.org ,").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.permits(Some("ana@example.com")));
        assert!(list.permits(Some(" ANA@EXAMPLE.COM ")));
        assert!(!list.permits(Some("bob@example.com")));
        assert!(list.permits(Some("anyone@example.org")));
        assert!(list.permits(Some("ANYONE@Example.Org")));
        assert!(!list.permits(Some("anyone@sub.example.org")));
        assert!(!list.permits(Some("anyone@notexample.org")));
        assert!(!list.permits(Some("example.org")));
        assert!(!list.permits(Some("@example.org")));
        assert!(!list.permits(None));
    }

    #[test]
    fn an_empty_allow_list_restricts_nobody() {
        for value in ["", "  ", " , ,"] {
            let list = AllowList::parse(value).unwrap();
            assert!(!list.is_restricted());
            assert!(list.permits(Some("anyone@example.com")));
            assert!(list.permits(None));
        }
        assert!(SignupConfig::default().allow_list().unwrap().is_empty());
    }

    #[test]
    fn a_malformed_allow_list_stops_startup_without_echoing_it() {
        for value in [
            "ana@",
            "@example.com",
            "a@b@example.com",
            "localhost",
            "example..com",
            "-x.example.com",
            "exa mple.com",
            "*.example.com",
            "ana@example",
        ] {
            let err = AllowList::parse(value).unwrap_err();
            assert!(err.contains("CAMPBOOKS_SIGNUP__ALLOWED"), "{value}: {err}");
            assert!(!err.contains(value), "{value}: {err}");
        }
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "signup",
                serde_json::json!({ "allowed": "ok.example.com, nope" }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        assert!(config.check().unwrap_err().contains("entry 2"));
    }

    #[test]
    fn trusted_proxies_come_from_one_comma_separated_setting() {
        let config = Config::from_figment(&figment(&[
            ("database_url", "postgres://localhost/x"),
            ("trusted_proxies", "10.0.0.0/8, 192.0.2.1"),
        ]))
        .unwrap();
        assert!(config.trusted_proxies.contains("10.9.8.7".parse().unwrap()));
        assert!(
            !config
                .trusted_proxies
                .contains("203.0.113.1".parse().unwrap())
        );
        let err = Config::from_figment(&figment(&[
            ("database_url", "postgres://localhost/x"),
            ("trusted_proxies", "proxy"),
        ]))
        .unwrap_err();
        assert!(
            err.to_string().contains("CAMPBOOKS_TRUSTED_PROXIES"),
            "{err}"
        );
    }

    #[test]
    fn check_names_the_bad_setting() {
        let mut config = Config::from_figment(&figment(&[
            ("database_url", "postgres://localhost/x"),
            ("database_role", "app; DROP TABLE x"),
        ]))
        .unwrap();
        assert!(
            config
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_DATABASE_ROLE")
        );
        config.database_role = String::new();
        config.changes.heartbeat_seconds = 0;
        assert!(
            config
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_CHANGES__HEARTBEAT_SECONDS")
        );
        config.changes = ChangesConfig::default();
        config.changes.max_connection_seconds = 10;
        assert!(
            config
                .check()
                .unwrap_err()
                .contains("CAMPBOOKS_CHANGES__MAX_CONNECTION_SECONDS")
        );
        config.changes = ChangesConfig::default();
        for seconds in [0, RateLimitsConfig::MAX_WINDOW_SECONDS + 1] {
            config.changes.opens_window_seconds = seconds;
            assert!(
                config.check().unwrap_err().contains(
                    "CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS must be between 1 and 86400"
                ),
                "{seconds}"
            );
        }
        config.changes.opens_window_seconds = RateLimitsConfig::MAX_WINDOW_SECONDS;
        assert!(
            config.changes.check().is_ok(),
            "a day is the longest window"
        );
        config.changes = ChangesConfig::default();
        for ms in [0, 49, 1_001] {
            config.changes.catch_up_tick_ms = ms;
            assert!(
                config
                    .check()
                    .unwrap_err()
                    .contains("CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS must be between 50 and 1000"),
                "{ms}"
            );
        }
        for ms in [50, 1_000] {
            config.changes.catch_up_tick_ms = ms;
            assert!(config.changes.check().is_ok(), "{ms}");
        }
        config.changes = ChangesConfig::default();
        // The `code` policy needs no environment code: administrators mint
        // single-use codes in the database.
        config.signup.policy = SignupPolicy::Code;
        assert!(config.check().is_ok());
        for policy in ["open", "invite_only", "approval"] {
            let figment = figment(&[("database_url", "postgres://localhost/x")]).merge(
                Serialized::default("signup", serde_json::json!({ "policy": policy })),
            );
            assert!(
                Config::from_figment(&figment).unwrap().check().is_ok(),
                "{policy}"
            );
        }
    }

    #[test]
    fn parses_telemetry_section() {
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "telemetry",
                serde_json::json!({
                    "otlp_endpoint": "http://collector:4318/",
                    "otlp_headers": "authorization=Bearer hunter2, x-tenant = cloud ,",
                    "sample_ratio": 0.25,
                    "metrics_addr": "127.0.0.1:9464",
                }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        let telemetry = &config.telemetry;
        assert_eq!(telemetry.otlp_endpoint(), Some("http://collector:4318"));
        assert_eq!(
            telemetry.otlp_headers(),
            [
                ("authorization".to_owned(), "Bearer hunter2".to_owned()),
                ("x-tenant".to_owned(), "cloud".to_owned()),
            ]
        );
        assert!((telemetry.sample_ratio - 0.25).abs() < f64::EPSILON);
        assert_eq!(telemetry.metrics_addr.unwrap().port(), 9464);
        assert!(telemetry.exports_metrics());
        assert!(config.check().is_ok());
        assert!(!format!("{config:?}").contains("hunter2"));
    }

    #[test]
    fn telemetry_check_names_the_bad_setting() {
        let cases = [
            (
                TelemetryConfig {
                    otlp_endpoint: Some("collector:4318".into()),
                    ..TelemetryConfig::default()
                },
                "CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT",
            ),
            (
                TelemetryConfig {
                    sample_ratio: 2.0,
                    ..TelemetryConfig::default()
                },
                "CAMPBOOKS_TELEMETRY__SAMPLE_RATIO",
            ),
            (
                TelemetryConfig {
                    otlp_headers: Some(SecretString::from("hunter2")),
                    ..TelemetryConfig::default()
                },
                "CAMPBOOKS_TELEMETRY__OTLP_HEADERS",
            ),
            (
                TelemetryConfig {
                    client_collector_url: Some("alloy:12347".into()),
                    ..TelemetryConfig::default()
                },
                "CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL",
            ),
            (
                TelemetryConfig {
                    environment: Some("prod uction".into()),
                    ..TelemetryConfig::default()
                },
                "CAMPBOOKS_TELEMETRY__ENVIRONMENT",
            ),
        ];
        for (config, setting) in cases {
            let err = config.check().unwrap_err();
            assert!(err.contains(setting), "{setting}: {err}");
            assert!(!err.contains("hunter2"), "{err}");
        }
        assert!(TelemetryConfig::default().check().is_ok());
        let blank = TelemetryConfig {
            error_dsn: Some(SecretString::from("")),
            environment: Some(" ".into()),
            client_collector_url: Some(String::new()),
            ..TelemetryConfig::default()
        };
        assert!(blank.check().is_ok());
        assert_eq!(blank.error_dsn(), None);
        assert_eq!(blank.client_collector_url(), None);
        assert_eq!(blank.environment(), "production");
    }

    #[test]
    fn analytics_is_off_unless_all_three_are_set() {
        let key = "k".repeat(MIN_VISITOR_KEY_CHARS);
        let full = AnalyticsConfig {
            matomo_url: Some("https://analytics.example.com/".into()),
            matomo_site_id: Some("2".into()),
            matomo_visitor_key: Some(SecretString::from(key.clone())),
        };
        let matomo = full.matomo().unwrap();
        assert_eq!(matomo.url, "https://analytics.example.com");
        assert_eq!(matomo.site_id, "2");
        assert_eq!(matomo.visitor_key(), key.as_bytes());
        assert!(full.check().is_ok());
        assert!(AnalyticsConfig::default().matomo().is_none());
        assert!(AnalyticsConfig::default().check().is_ok());
        for partial in [
            AnalyticsConfig {
                matomo_url: None,
                ..full.clone()
            },
            AnalyticsConfig {
                matomo_site_id: Some(" ".into()),
                ..full.clone()
            },
            AnalyticsConfig {
                matomo_visitor_key: Some(SecretString::from("")),
                ..full.clone()
            },
        ] {
            assert!(partial.matomo().is_none(), "{partial:?}");
            assert!(partial.check().is_ok(), "{partial:?}");
        }
    }

    #[test]
    fn analytics_site_id_is_read_as_text_or_number() {
        for (value, expected) in [
            (serde_json::json!(2), Some("2")),
            (serde_json::json!("2"), Some("2")),
            (serde_json::json!(""), None),
        ] {
            let figment = figment(&[("database_url", "postgres://localhost/x")]).merge(
                Serialized::default("analytics", serde_json::json!({ "matomo_site_id": value })),
            );
            let config = Config::from_figment(&figment).unwrap();
            assert_eq!(
                non_blank(config.analytics.matomo_site_id.as_deref()),
                expected
            );
        }
    }

    #[test]
    fn analytics_check_names_the_bad_setting_never_the_key() {
        let cases = [
            (
                AnalyticsConfig {
                    matomo_url: Some("analytics.example.com".into()),
                    ..AnalyticsConfig::default()
                },
                "CAMPBOOKS_ANALYTICS__MATOMO_URL",
            ),
            (
                AnalyticsConfig {
                    matomo_site_id: Some("0".into()),
                    ..AnalyticsConfig::default()
                },
                "CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID",
            ),
            (
                AnalyticsConfig {
                    matomo_site_id: Some("two".into()),
                    ..AnalyticsConfig::default()
                },
                "CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID",
            ),
            (
                AnalyticsConfig {
                    matomo_visitor_key: Some(SecretString::from("hunter2")),
                    ..AnalyticsConfig::default()
                },
                "CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY",
            ),
        ];
        for (config, setting) in cases {
            let err = config.check().unwrap_err();
            assert!(err.contains(setting), "{setting}: {err}");
            assert!(!err.contains("hunter2"), "{err}");
        }
    }

    #[test]
    fn debug_redacts_the_visitor_key() {
        let key = format!("hunter2{}", "k".repeat(MIN_VISITOR_KEY_CHARS));
        let figment =
            figment(&[("database_url", "postgres://localhost/x")]).merge(Serialized::default(
                "analytics",
                serde_json::json!({
                    "matomo_url": "https://analytics.example.com",
                    "matomo_site_id": "2",
                    "matomo_visitor_key": key,
                }),
            ));
        let config = Config::from_figment(&figment).unwrap();
        assert!(config.check().is_ok());
        assert!(!format!("{config:?}").contains("hunter2"));
        let matomo = config.analytics.matomo().unwrap();
        assert!(!format!("{matomo:?}").contains("hunter2"));
    }

    #[test]
    fn debug_redacts_database_url() {
        let config =
            Config::from_figment(&figment(&[("database_url", "postgres://u:hunter2@h/x")]))
                .unwrap();
        assert!(!format!("{config:?}").contains("hunter2"));
    }
}
