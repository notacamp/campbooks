//! The module's settings: `CAMPBOOKS_CONNECTORS__*` (the sync engine),
//! `CAMPBOOKS_GOOGLE__*` and `CAMPBOOKS_ZOHO__*` (the OAuth clients, shared
//! with sign-in).

use std::path::PathBuf;

use figment::Figment;
use secrecy::SecretString;
use serde::Deserialize;

/// Settings of the sync engine and adapters, `CAMPBOOKS_CONNECTORS__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConnectorsConfig {
    /// The Gmail adapter (`CAMPBOOKS_CONNECTORS__GMAIL__*`).
    pub gmail: GmailConfig,
    /// Adaptive polling (`CAMPBOOKS_CONNECTORS__POLL__*`).
    pub poll: PollConfig,
    /// Google's request budget (`CAMPBOOKS_CONNECTORS__GOOGLE__*`).
    pub google: BudgetConfig,
    /// The Zoho Mail adapter (`CAMPBOOKS_CONNECTORS__ZOHO__*`).
    pub zoho: ZohoMailConfig,
    /// Consecutive failed runs before a connection's health is `failing`
    /// (`CAMPBOOKS_CONNECTORS__FAILING_AFTER`, default 5).
    pub failing_after: u32,
    /// Days a produced batch is kept for replay
    /// (`CAMPBOOKS_CONNECTORS__BATCH_RETENTION_DAYS`, default 7).
    pub batch_retention_days: u32,
}

/// `CAMPBOOKS_CONNECTORS__GMAIL__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GmailConfig {
    /// How far back a new connection reads, in days (default 90).
    pub backfill_days: u32,
    /// Messages larger than this (bytes, as Gmail sends them) are skipped
    /// and recorded as oversized instead of stored (default 50 MiB).
    pub max_message_bytes: u64,
    /// One run stops starting downloads once it has fetched this many
    /// bytes and continues in the next run (default 64 MiB), so a run's
    /// memory stays bounded.
    pub run_budget_bytes: u64,
}

/// `CAMPBOOKS_CONNECTORS__ZOHO__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZohoMailConfig {
    /// How far back a new mailbox reads, in days (default 90).
    pub backfill_days: u32,
    /// Messages larger than this (bytes) are skipped and recorded as
    /// oversized (default 50 MiB).
    pub max_message_bytes: u64,
    /// One run starts no download once it has fetched this many bytes
    /// (default 64 MiB).
    pub run_budget_bytes: u64,
    /// Requests per minute each Zoho login may make, shared by every
    /// process (default 60).
    pub requests_per_minute: u32,
    /// Minutes between sweeps that re-read a mailbox's whole window
    /// (default 360).
    pub sweep_minutes: u32,
}

/// `CAMPBOOKS_CONNECTORS__POLL__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PollConfig {
    /// Interval after a run that found changes, in seconds (default 60).
    pub min_seconds: u32,
    /// Longest interval of an idle stream, in seconds (default 4 hours).
    pub max_seconds: u32,
}

/// `CAMPBOOKS_CONNECTORS__GOOGLE__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetConfig {
    /// Google API quota units per second shared by every process (default
    /// 5000, a quarter of Gmail's default per-project quota). The budget
    /// holds two seconds' worth: a run planned to cost more waits for a full
    /// budget and owes the rest once it is done, so a low rate slows big
    /// pushes down but never lets them spend uncounted.
    pub units_per_second: u32,
}

impl Default for ConnectorsConfig {
    fn default() -> Self {
        Self {
            gmail: GmailConfig::default(),
            poll: PollConfig::default(),
            google: BudgetConfig::default(),
            zoho: ZohoMailConfig::default(),
            failing_after: 5,
            batch_retention_days: 7,
        }
    }
}

impl Default for GmailConfig {
    fn default() -> Self {
        Self {
            backfill_days: 90,
            max_message_bytes: 50 * 1024 * 1024,
            run_budget_bytes: 64 * 1024 * 1024,
        }
    }
}

impl Default for ZohoMailConfig {
    fn default() -> Self {
        Self {
            backfill_days: 90,
            max_message_bytes: 50 * 1024 * 1024,
            run_budget_bytes: 64 * 1024 * 1024,
            requests_per_minute: 60,
            sweep_minutes: 360,
        }
    }
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            min_seconds: 60,
            max_seconds: 4 * 60 * 60,
        }
    }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            units_per_second: 5_000,
        }
    }
}

impl ConnectorsConfig {
    /// Load the `connectors` section of `figment` and check it.
    pub fn from_figment(figment: &Figment) -> Result<Self, String> {
        let config: Self = if figment.contains("connectors") {
            figment
                .extract_inner("connectors")
                .map_err(|e| format!("CAMPBOOKS_CONNECTORS__*: {e}"))?
        } else {
            Self::default()
        };
        config.check()?;
        Ok(config)
    }

    fn check(&self) -> Result<(), String> {
        if self.gmail.backfill_days == 0 || self.gmail.backfill_days > 3650 {
            return Err("CAMPBOOKS_CONNECTORS__GMAIL__BACKFILL_DAYS must be 1 to 3650".to_owned());
        }
        if self.gmail.max_message_bytes == 0 || self.gmail.run_budget_bytes == 0 {
            return Err("CAMPBOOKS_CONNECTORS__GMAIL__MAX_MESSAGE_BYTES and \
                 CAMPBOOKS_CONNECTORS__GMAIL__RUN_BUDGET_BYTES must be positive"
                .to_owned());
        }
        if self.poll.min_seconds == 0 || self.poll.min_seconds > self.poll.max_seconds {
            return Err(
                "CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS must be at least 1 and at most \
                 CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS"
                    .to_owned(),
            );
        }
        if self.google.units_per_second == 0 {
            return Err(
                "CAMPBOOKS_CONNECTORS__GOOGLE__UNITS_PER_SECOND must be positive".to_owned(),
            );
        }
        let zoho = &self.zoho;
        if zoho.backfill_days == 0 || zoho.backfill_days > 3650 {
            return Err("CAMPBOOKS_CONNECTORS__ZOHO__BACKFILL_DAYS must be 1 to 3650".to_owned());
        }
        if zoho.max_message_bytes == 0
            || zoho.run_budget_bytes == 0
            || zoho.requests_per_minute == 0
            || zoho.sweep_minutes == 0
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__ZOHO__MAX_MESSAGE_BYTES, …__RUN_BUDGET_BYTES, \
                 …__REQUESTS_PER_MINUTE and …__SWEEP_MINUTES must be positive"
                    .to_owned(),
            );
        }
        if self.failing_after == 0 {
            return Err("CAMPBOOKS_CONNECTORS__FAILING_AFTER must be positive".to_owned());
        }
        if self.batch_retention_days == 0 {
            return Err("CAMPBOOKS_CONNECTORS__BATCH_RETENTION_DAYS must be positive".to_owned());
        }
        Ok(())
    }
}

/// A provider's OAuth client, `CAMPBOOKS_GOOGLE__*` or `CAMPBOOKS_ZOHO__*`.
/// The provider is offered only when the client id and a secret are set.
/// The identity module reads the same settings (one OAuth app serves
/// sign-in and mail); keys it alone uses are ignored here.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OAuthClientConfig {
    /// `…__CLIENT_ID`.
    pub client_id: Option<String>,
    /// `…__CLIENT_SECRET`, inline; for development.
    pub client_secret: Option<SecretString>,
    /// `…__CLIENT_SECRET_FILE`: read when a token is requested, so a
    /// rotated secret is picked up without a restart.
    pub client_secret_file: Option<PathBuf>,
    /// `…__BASE_URL`: serve every endpoint of the provider from this base
    /// instead (a local fake). Development only (`CAMPBOOKS_ENV=development`);
    /// refused otherwise.
    pub base_url: Option<String>,
}

impl OAuthClientConfig {
    /// Load the `section` (`google`, `zoho`) of `figment`.
    pub fn from_figment(figment: &Figment, section: &str) -> Result<Self, String> {
        if !figment.contains(section) {
            return Ok(Self::default());
        }
        figment
            .extract_inner(section)
            .map_err(|e| format!("CAMPBOOKS_{}__*: {e}", section.to_ascii_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;

    use super::*;

    fn figment(value: serde_json::Value) -> Figment {
        Figment::from(Serialized::defaults(value))
    }

    #[test]
    fn defaults() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert_eq!(config, ConnectorsConfig::default());
        assert_eq!(config.gmail.backfill_days, 90);
        assert_eq!(
            (config.poll.min_seconds, config.poll.max_seconds),
            (60, 14_400)
        );
    }

    #[test]
    fn reads_and_checks_settings() {
        let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"gmail": {"backfill_days": 30}, "poll": {"min_seconds": 30}}
        })))
        .unwrap();
        assert_eq!(config.gmail.backfill_days, 30);
        assert_eq!(config.poll.min_seconds, 30);
        let bad = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 99_999}}
        })));
        assert!(bad.unwrap_err().contains("MIN_SECONDS"));
        let unknown = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"gmail": {"backfil_days": 3}}
        })));
        assert!(unknown.is_err());
    }

    #[test]
    fn providers_are_unset_by_default() {
        for section in ["google", "zoho"] {
            let client = OAuthClientConfig::from_figment(&Figment::new(), section).unwrap();
            assert!(client.client_id.is_none() && client.client_secret_file.is_none());
        }
    }

    #[test]
    fn zoho_settings_are_read_and_checked() {
        let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"zoho": {"requests_per_minute": 30, "sweep_minutes": 60}}
        })))
        .unwrap();
        assert_eq!(config.zoho.requests_per_minute, 30);
        assert_eq!(config.zoho.sweep_minutes, 60);
        let bad = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"zoho": {"requests_per_minute": 0}}
        })));
        assert!(bad.unwrap_err().contains("REQUESTS_PER_MINUTE"));
        // Sign-in's own keys sit in the same section and are ignored here.
        let client = OAuthClientConfig::from_figment(
            &figment(serde_json::json!({"zoho": {"client_id": "z", "redirect_path": "/oauth/zoho/callback",
                                                 "accounts_server": "https://accounts.zoho.eu"}})),
            "zoho",
        )
        .unwrap();
        assert_eq!(client.client_id.as_deref(), Some("z"));
    }
}
