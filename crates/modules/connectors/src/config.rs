//! The module's settings: `CAMPBOOKS_CONNECTORS__*` (the sync engine and
//! the IMAP adapter), `CAMPBOOKS_GOOGLE__*` and `CAMPBOOKS_ZOHO__*` (the
//! OAuth clients, shared with sign-in).

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
    /// Any mailbox over IMAP and SMTP (`CAMPBOOKS_CONNECTORS__IMAP__*`).
    pub imap: ImapConfig,
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

/// `CAMPBOOKS_CONNECTORS__IMAP__*`: connecting any mailbox with its
/// password (usually an app password).
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one independent setting an operator turns on"
)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImapConfig {
    /// Offer IMAP connections and read them (default false, until the
    /// connect screen lands).
    pub enabled: bool,
    /// Also reach mail servers on private, loopback and CGNAT addresses: a
    /// self-hosted instance's own network (default false). Never link-local
    /// or cloud metadata addresses.
    pub allow_private_hosts: bool,
    /// Reach mail servers on any port, not only 993, 143, 465 and 587
    /// (default false).
    pub allow_any_port: bool,
    /// How long opening a connection to a mail server may take, in seconds
    /// (default 10).
    pub connect_timeout_seconds: u32,
    /// How long talking to one mail server may take in all, in seconds
    /// (default 30); while reading a mailbox, how long one command may wait
    /// for the server.
    pub timeout_seconds: u32,
    /// How far back a new mailbox reads, in days (default 90).
    pub backfill_days: u32,
    /// Messages larger than this (bytes, as the server states or sends
    /// them) are recorded as oversized and not downloaded (default 50 MiB).
    pub max_message_bytes: u64,
    /// One run starts no download once it has received this many bytes
    /// (default 64 MiB), so a run's memory stays bounded.
    pub run_budget_bytes: u64,
    /// Minutes between sweeps that re-read the flags of a mailbox's window
    /// and find what was deleted (default 360).
    pub sweep_minutes: u32,
    /// Sessions open at once to one IMAP host, across every process
    /// (default 4).
    pub sessions_per_host: u32,
    /// Of those, the most one workspace may hold (default 2), so one
    /// workspace never takes a provider's every session.
    pub workspace_sessions_per_host: u32,
    /// Most folders of one mailbox read (default 200).
    pub max_folders: u32,
    /// Sessions open at once to the IMAP host of a big provider (one of
    /// [`PROVIDER_HOSTS`](crate::imap::PROVIDER_HOSTS)), across every
    /// process, runs and IDLE together (default 200): the small hosts' cap
    /// ([`sessions_per_host`](Self::sessions_per_host)) would starve a big
    /// provider's customers.
    pub provider_sessions_per_host: u32,
    /// Hold mailboxes' Inboxes in IDLE (RFC 2177), so new mail is read as
    /// it arrives (default false). Needs [`enabled`](Self::enabled).
    pub idle_enabled: bool,
    /// IDLE sessions one process holds at most (default 50).
    pub idle_sessions: u32,
    /// Of those, the most one workspace's mailboxes hold (default 5), so
    /// one workspace never fills a process's pool.
    pub idle_sessions_per_workspace: u32,
    /// A mailbox's IDLE asks for at most one run per this many seconds
    /// (default 30), however often its server reports a change.
    pub idle_debounce_seconds: u32,
    /// Minutes between two renewals of IDLE (`DONE`, `NOOP`, `IDLE`;
    /// default 25, at most 29 as RFC 2177 asks).
    pub idle_refresh_minutes: u32,
    /// A mailbox with no change for this many days drops IDLE and is polled
    /// (default 14).
    pub idle_quiet_days: u32,
    /// While a mailbox is held in IDLE, its next poll is at least this many
    /// seconds away (default 900): a safety poll, which also reads the
    /// folders IDLE does not watch.
    pub idle_poll_seconds: u32,
    /// IDLE sessions one workspace holds at most on one IMAP host (default
    /// 10); on a host that is not a known provider, at most half its
    /// [`workspace_sessions_per_host`](Self::workspace_sessions_per_host)
    /// too.
    pub workspace_idle_sessions_per_host: u32,
    /// Folders Campbooks creates in one mailbox at most (stream folders,
    /// `Campbooks/<Stream>`; default 10, at most 100).
    pub max_created_folders: u32,
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
            imap: ImapConfig::default(),
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

impl Default for ImapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_private_hosts: false,
            allow_any_port: false,
            connect_timeout_seconds: 10,
            timeout_seconds: 30,
            backfill_days: 90,
            max_message_bytes: 50 * 1024 * 1024,
            run_budget_bytes: 64 * 1024 * 1024,
            sweep_minutes: 360,
            sessions_per_host: 4,
            workspace_sessions_per_host: 2,
            max_folders: 200,
            provider_sessions_per_host: 200,
            idle_enabled: false,
            idle_sessions: 50,
            idle_sessions_per_workspace: 5,
            idle_debounce_seconds: 30,
            idle_refresh_minutes: 25,
            idle_quiet_days: 14,
            idle_poll_seconds: 900,
            workspace_idle_sessions_per_host: 10,
            max_created_folders: 10,
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
        let imap = &self.imap;
        if imap.connect_timeout_seconds == 0
            || imap.timeout_seconds == 0
            || imap.connect_timeout_seconds > imap.timeout_seconds
            || imap.timeout_seconds > 300
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__CONNECT_TIMEOUT_SECONDS must be at least 1 and at \
                 most CAMPBOOKS_CONNECTORS__IMAP__TIMEOUT_SECONDS, itself at most 300"
                    .to_owned(),
            );
        }
        if imap.backfill_days == 0 || imap.backfill_days > 3650 {
            return Err("CAMPBOOKS_CONNECTORS__IMAP__BACKFILL_DAYS must be 1 to 3650".to_owned());
        }
        if imap.max_message_bytes == 0
            || imap.run_budget_bytes == 0
            || imap.sweep_minutes == 0
            || imap.sessions_per_host == 0
            || imap.max_folders == 0
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES, …__RUN_BUDGET_BYTES, \
                 …__SWEEP_MINUTES, …__SESSIONS_PER_HOST and …__MAX_FOLDERS must be positive"
                    .to_owned(),
            );
        }
        if imap.workspace_sessions_per_host == 0
            || imap.workspace_sessions_per_host > imap.sessions_per_host
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_SESSIONS_PER_HOST must be at least 1 and \
                 at most …__SESSIONS_PER_HOST"
                    .to_owned(),
            );
        }
        if imap.max_message_bytes > 1024 * 1024 * 1024 || imap.max_folders > 10_000 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES must be at most 1 GiB and \
                 …__MAX_FOLDERS at most 10000"
                    .to_owned(),
            );
        }
        self.check_idle()?;
        if imap.max_created_folders == 0 || imap.max_created_folders > 100 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS must be 1 to 100".to_owned(),
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

impl ConnectorsConfig {
    /// The IDLE settings, and the known providers' session cap.
    fn check_idle(&self) -> Result<(), String> {
        let imap = &self.imap;
        if imap.provider_sessions_per_host < imap.sessions_per_host
            || imap.provider_sessions_per_host > 100_000
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__PROVIDER_SESSIONS_PER_HOST must be at least \
                 …__SESSIONS_PER_HOST and at most 100000"
                    .to_owned(),
            );
        }
        if imap.idle_sessions == 0 || imap.idle_sessions > 10_000 {
            return Err("CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS must be 1 to 10000".to_owned());
        }
        if imap.idle_sessions_per_workspace == 0
            || imap.idle_sessions_per_workspace > imap.idle_sessions
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS_PER_WORKSPACE must be at least 1 and \
                 at most …__IDLE_SESSIONS"
                    .to_owned(),
            );
        }
        if imap.idle_debounce_seconds == 0 || imap.idle_debounce_seconds > 3_600 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__IDLE_DEBOUNCE_SECONDS must be 1 to 3600".to_owned(),
            );
        }
        if imap.idle_refresh_minutes == 0 || imap.idle_refresh_minutes > 29 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__IDLE_REFRESH_MINUTES must be 1 to 29 (RFC 2177)"
                    .to_owned(),
            );
        }
        if imap.idle_quiet_days == 0 || imap.idle_quiet_days > 365 {
            return Err("CAMPBOOKS_CONNECTORS__IMAP__IDLE_QUIET_DAYS must be 1 to 365".to_owned());
        }
        // Tied to the poll bounds only while IDLE runs: with it off the
        // setting is unused, and an instance that narrowed its poll bounds
        // must still start.
        if imap.idle_poll_seconds == 0
            || (imap.enabled
                && imap.idle_enabled
                && (imap.idle_poll_seconds < self.poll.min_seconds
                    || imap.idle_poll_seconds > self.poll.max_seconds))
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS must be at least \
                 CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS and at most …__POLL__MAX_SECONDS"
                    .to_owned(),
            );
        }
        if imap.workspace_idle_sessions_per_host == 0 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_IDLE_SESSIONS_PER_HOST must be positive"
                    .to_owned(),
            );
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
    fn imap_is_off_by_default_and_its_settings_are_checked() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert!(!config.imap.enabled);
        assert!(!config.imap.allow_private_hosts && !config.imap.allow_any_port);
        assert_eq!(
            (
                config.imap.connect_timeout_seconds,
                config.imap.timeout_seconds
            ),
            (10, 30)
        );
        let on = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"enabled": true, "allow_private_hosts": true}}
        })))
        .unwrap();
        assert!(on.imap.enabled && on.imap.allow_private_hosts);
        let bad = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"connect_timeout_seconds": 60, "timeout_seconds": 30}}
        })));
        assert!(bad.unwrap_err().contains("CONNECT_TIMEOUT_SECONDS"));
        assert_eq!(
            (
                config.imap.backfill_days,
                config.imap.max_message_bytes,
                config.imap.run_budget_bytes,
                config.imap.sweep_minutes,
                config.imap.sessions_per_host,
                config.imap.max_folders,
            ),
            (90, 50 * 1024 * 1024, 64 * 1024 * 1024, 360, 4, 200)
        );
        let read = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"sessions_per_host": 2, "sweep_minutes": 60}}
        })))
        .unwrap();
        assert_eq!(
            (read.imap.sessions_per_host, read.imap.sweep_minutes),
            (2, 60)
        );
        for bad in [
            serde_json::json!({"sessions_per_host": 0}),
            serde_json::json!({"max_folders": 0}),
            serde_json::json!({"backfill_days": 0}),
            serde_json::json!({"max_message_bytes": 2_u64 << 30}),
            serde_json::json!({"workspace_sessions_per_host": 5}),
            serde_json::json!({"workspace_sessions_per_host": 0}),
        ] {
            let refused = ConnectorsConfig::from_figment(&figment(
                serde_json::json!({"connectors": {"imap": bad}}),
            ));
            assert!(
                refused
                    .unwrap_err()
                    .contains("CAMPBOOKS_CONNECTORS__IMAP__")
            );
        }
    }

    #[test]
    fn idle_is_off_by_default_and_its_settings_are_checked() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        let imap = &config.imap;
        assert!(!imap.idle_enabled);
        assert_eq!(
            (
                imap.idle_sessions,
                imap.idle_sessions_per_workspace,
                imap.idle_debounce_seconds,
                imap.idle_refresh_minutes,
                imap.idle_quiet_days,
                imap.idle_poll_seconds,
                imap.provider_sessions_per_host,
                imap.workspace_idle_sessions_per_host,
            ),
            (50, 5, 30, 25, 14, 900, 200, 10)
        );
        let on = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"enabled": true, "idle_enabled": true, "idle_sessions": 5}}
        })))
        .unwrap();
        assert!(on.imap.idle_enabled);
        assert_eq!(on.imap.idle_sessions, 5);
        assert_eq!(on.imap.idle_sessions_per_workspace, 5);
        for (bad, setting) in [
            (serde_json::json!({"idle_sessions": 0}), "IDLE_SESSIONS"),
            (
                serde_json::json!({"idle_sessions": 4}),
                "IDLE_SESSIONS_PER_WORKSPACE",
            ),
            (
                serde_json::json!({"idle_sessions_per_workspace": 0}),
                "IDLE_SESSIONS_PER_WORKSPACE",
            ),
            (
                serde_json::json!({"idle_debounce_seconds": 0}),
                "IDLE_DEBOUNCE_SECONDS",
            ),
            (
                serde_json::json!({"idle_refresh_minutes": 30}),
                "IDLE_REFRESH_MINUTES",
            ),
            (serde_json::json!({"idle_quiet_days": 0}), "IDLE_QUIET_DAYS"),
            (
                serde_json::json!({"enabled": true, "idle_enabled": true, "idle_poll_seconds": 10}),
                "IDLE_POLL_SECONDS",
            ),
            (
                serde_json::json!({"idle_poll_seconds": 0}),
                "IDLE_POLL_SECONDS",
            ),
            (
                serde_json::json!({"provider_sessions_per_host": 3}),
                "PROVIDER_SESSIONS_PER_HOST",
            ),
            (
                serde_json::json!({"workspace_idle_sessions_per_host": 0}),
                "WORKSPACE_IDLE_SESSIONS_PER_HOST",
            ),
        ] {
            let refused = ConnectorsConfig::from_figment(&figment(
                serde_json::json!({"connectors": {"imap": bad}}),
            ))
            .unwrap_err();
            assert!(refused.contains(setting), "{refused}");
        }
    }

    #[test]
    fn imap_write_back_settings_are_checked() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert_eq!(config.imap.max_created_folders, 10);
        let read = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"max_created_folders": 3}}
        })))
        .unwrap();
        assert_eq!(read.imap.max_created_folders, 3);
        for bad in [0, 101] {
            let refused = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"imap": {"max_created_folders": bad}}
            })))
            .unwrap_err();
            assert!(refused.contains("MAX_CREATED_FOLDERS"), "{refused}");
        }
    }

    #[test]
    fn narrowed_poll_bounds_start_while_idle_is_off() {
        // An instance whose poll bounds leave out the IDLE safety poll's
        // default still starts with IDLE off (the 2026-10-23 outage), and is
        // told so only once it turns IDLE on.
        let narrowed = serde_json::json!({"poll": {"min_seconds": 60, "max_seconds": 600}});
        let config = ConnectorsConfig::from_figment(&figment(
            serde_json::json!({"connectors": narrowed.clone()}),
        ))
        .unwrap();
        assert_eq!(config.poll.max_seconds, 600);
        let mut on = narrowed;
        on["imap"] = serde_json::json!({"enabled": true, "idle_enabled": true});
        let refused =
            ConnectorsConfig::from_figment(&figment(serde_json::json!({"connectors": on})))
                .unwrap_err();
        assert!(refused.contains("IDLE_POLL_SECONDS"), "{refused}");
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
