//! The module's settings: `CAMPBOOKS_CONNECTORS__*` (the sync engine) and
//! `CAMPBOOKS_GOOGLE__*` (the Google OAuth client).

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
    /// 5000, a quarter of Gmail's default per-project quota).
    pub units_per_second: u32,
}

impl Default for ConnectorsConfig {
    fn default() -> Self {
        Self {
            gmail: GmailConfig::default(),
            poll: PollConfig::default(),
            google: BudgetConfig::default(),
            failing_after: 5,
            batch_retention_days: 7,
        }
    }
}

impl Default for GmailConfig {
    fn default() -> Self {
        Self { backfill_days: 90 }
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
        if self.failing_after == 0 {
            return Err("CAMPBOOKS_CONNECTORS__FAILING_AFTER must be positive".to_owned());
        }
        if self.batch_retention_days == 0 {
            return Err("CAMPBOOKS_CONNECTORS__BATCH_RETENTION_DAYS must be positive".to_owned());
        }
        Ok(())
    }
}

/// The Google OAuth client, `CAMPBOOKS_GOOGLE__*`. Google is offered only
/// when the client id and a secret are set.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GoogleConfig {
    /// `CAMPBOOKS_GOOGLE__CLIENT_ID`.
    pub client_id: Option<String>,
    /// `CAMPBOOKS_GOOGLE__CLIENT_SECRET`, inline; for development.
    pub client_secret: Option<SecretString>,
    /// `CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`: read when a token is
    /// requested, so a rotated secret is picked up without a restart.
    pub client_secret_file: Option<PathBuf>,
    /// `CAMPBOOKS_GOOGLE__BASE_URL`: serve every Google endpoint from this
    /// base instead (a local fake Google). Development only
    /// (`CAMPBOOKS_ENV=development`); refused otherwise.
    pub base_url: Option<String>,
}

impl GoogleConfig {
    /// Load the `google` section of `figment`.
    pub fn from_figment(figment: &Figment) -> Result<Self, String> {
        if !figment.contains("google") {
            return Ok(Self::default());
        }
        figment
            .extract_inner("google")
            .map_err(|e| format!("CAMPBOOKS_GOOGLE__*: {e}"))
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
    fn google_is_unset_by_default() {
        let google = GoogleConfig::from_figment(&Figment::new()).unwrap();
        assert!(google.client_id.is_none() && google.client_secret_file.is_none());
    }
}
