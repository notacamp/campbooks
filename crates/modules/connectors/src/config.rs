//! The module's settings: `CAMPBOOKS_CONNECTORS__*` (the sync engine, the
//! IMAP adapter and Microsoft mail), `CAMPBOOKS_GOOGLE__*`,
//! `CAMPBOOKS_MICROSOFT__*` and `CAMPBOOKS_ZOHO__*` (the OAuth clients,
//! shared with sign-in).

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
    /// Calendar sync (`CAMPBOOKS_CONNECTORS__CALENDAR__*`).
    pub calendar: CalendarConfig,
    /// Microsoft mail (`CAMPBOOKS_CONNECTORS__MICROSOFT__*`), off by
    /// default.
    pub microsoft: MicrosoftMailConfig,
    /// Notion pages kept in step with Pages
    /// (`CAMPBOOKS_CONNECTORS__NOTION__*`), off by default.
    pub notion: NotionConfig,
    /// Whether Zoho Calendar is offered (`CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR`,
    /// default off): its adapter is unverified against a live Zoho grant.
    pub zoho_calendar: bool,
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

/// `CAMPBOOKS_CONNECTORS__CALENDAR__*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarConfig {
    /// Requests per minute each connection's calendar may make, shared by
    /// every process (default 300). Google allows a user far more; this
    /// leaves room for a member's other apps.
    pub requests_per_minute: u32,
}

/// `CAMPBOOKS_CONNECTORS__MICROSOFT__*`: Outlook.com, Hotmail, Live, MSN
/// and Microsoft 365 mail through Microsoft Graph (campbooks-cloud#309).
/// Nothing here is checked while it is off: a server that does not offer
/// Microsoft mail starts whatever these say (an unknown key or a tenant of
/// any shape included).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct MicrosoftMailConfig {
    /// Offer Microsoft mailboxes (default false). Also needs the Microsoft
    /// OAuth client (`CAMPBOOKS_MICROSOFT__CLIENT_ID` and a secret) and the
    /// instance secret key; without them it is not offered, with a warning.
    pub enabled: bool,
    /// The tenant a mail consent and its token refreshes go to: `common`
    /// (personal, work and school accounts), `organizations`, `consumers` or
    /// a tenant id. Unset, sign-in's (`CAMPBOOKS_MICROSOFT__TENANT`, itself
    /// `common` by default), so an app registered for one tenant never
    /// widens.
    #[serde(deserialize_with = "any_text")]
    pub tenant: Option<String>,
    /// How far back a new mailbox reads, in days (default 90).
    #[serde(deserialize_with = "any_text")]
    pub backfill_days: Option<String>,
    /// Messages larger than this (bytes, as Graph sends their MIME) are
    /// recorded as oversized and not stored (default 50 MiB, at most 64 MiB:
    /// up to four are downloaded at once).
    #[serde(deserialize_with = "any_text")]
    pub max_message_bytes: Option<String>,
    /// One run starts no download once it has fetched this many bytes
    /// (default 64 MiB). A run holds at most about this much plus the four
    /// downloads in flight (each at most `max_message_bytes`).
    #[serde(deserialize_with = "any_text")]
    pub run_budget_bytes: Option<String>,
    /// Requests per minute each Microsoft connection may make, shared by
    /// every process (default 600; Graph allows a mailbox 10000 in ten
    /// minutes).
    #[serde(deserialize_with = "any_text")]
    pub requests_per_minute: Option<String>,
    /// Most folders of one mailbox read (default 100).
    #[serde(deserialize_with = "any_text")]
    pub max_folders: Option<String>,
    /// Subscribe to Graph's change notifications (default false), so new
    /// mail is read as it arrives. Needs an `https` `CAMPBOOKS_PUBLIC_URL`
    /// that Microsoft can reach, forwarding `/hooks/microsoft/mail`; without
    /// one the mailboxes are polled.
    #[serde(deserialize_with = "any_text")]
    pub notifications: Option<String>,
}

/// The largest `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_MESSAGE_BYTES`: four
/// downloads in flight hold at most four such messages besides the run's
/// budget.
pub const MAX_MICROSOFT_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;

/// The Microsoft adapter's settings, read (defaults for what is unset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrosoftAdapter {
    pub backfill_days: u32,
    pub max_message_bytes: u64,
    pub run_budget_bytes: u64,
    pub requests_per_minute: u32,
    pub max_folders: u32,
    pub notifications: bool,
}

impl Default for MicrosoftAdapter {
    fn default() -> Self {
        Self {
            backfill_days: 90,
            max_message_bytes: 50 * 1024 * 1024,
            run_budget_bytes: 64 * 1024 * 1024,
            requests_per_minute: 600,
            max_folders: 100,
            notifications: false,
        }
    }
}

impl MicrosoftMailConfig {
    /// The adapter's settings, each checked: the first one that is not
    /// usable is an error naming it. Only called while Microsoft mail is
    /// on ([`ConnectorsConfig::check`]); [`adapter`](Self::adapter) reads
    /// them where they are used.
    pub fn read_adapter(&self) -> Result<MicrosoftAdapter, String> {
        let defaults = MicrosoftAdapter::default();
        let number = |value: Option<&str>, name: &str, low: u64, high: u64, default: u64| {
            let Some(text) = value.map(str::trim) else {
                return Ok(default);
            };
            text.parse::<u64>()
                .ok()
                .filter(|n| (low..=high).contains(n))
                .ok_or_else(|| {
                    format!("CAMPBOOKS_CONNECTORS__MICROSOFT__{name} must be {low} to {high}")
                })
        };
        let small = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        let notifications = match self.notifications.as_deref().map(str::trim) {
            None => defaults.notifications,
            Some(text) if text.eq_ignore_ascii_case("true") || text == "1" => true,
            Some(text) if text.eq_ignore_ascii_case("false") || text == "0" => false,
            Some(_) => {
                return Err(
                    "CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS must be true or false"
                        .to_owned(),
                );
            }
        };
        Ok(MicrosoftAdapter {
            backfill_days: small(number(
                self.backfill_days.as_deref(),
                "BACKFILL_DAYS",
                1,
                3650,
                u64::from(defaults.backfill_days),
            )?),
            max_message_bytes: number(
                self.max_message_bytes.as_deref(),
                "MAX_MESSAGE_BYTES",
                1,
                MAX_MICROSOFT_MESSAGE_BYTES,
                defaults.max_message_bytes,
            )?,
            run_budget_bytes: number(
                self.run_budget_bytes.as_deref(),
                "RUN_BUDGET_BYTES",
                1,
                u64::MAX,
                defaults.run_budget_bytes,
            )?,
            requests_per_minute: small(number(
                self.requests_per_minute.as_deref(),
                "REQUESTS_PER_MINUTE",
                1,
                100_000,
                u64::from(defaults.requests_per_minute),
            )?),
            max_folders: small(number(
                self.max_folders.as_deref(),
                "MAX_FOLDERS",
                1,
                1000,
                u64::from(defaults.max_folders),
            )?),
            notifications,
        })
    }

    /// The adapter's settings as used: what [`read_adapter`](Self::read_adapter)
    /// reads, or the defaults where a value is unusable (never the case
    /// while Microsoft mail is on: the server would not have started).
    pub fn adapter(&self) -> MicrosoftAdapter {
        self.read_adapter().unwrap_or_default()
    }
}

/// `CAMPBOOKS_CONNECTORS__NOTION__*`: pages imported from Notion and kept in
/// step both ways (campbooks-cloud#376). Nothing here is checked while it is
/// off: a server that does not offer Notion starts whatever these say (an
/// unknown key or a value of any shape included).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct NotionConfig {
    /// Offer Notion (default false: `true`, `1`, `yes` or `on` turn it on).
    /// Also needs the Notion integration (`CAMPBOOKS_NOTION__CLIENT_ID` and a
    /// secret) and the instance secret key; without them it is not offered,
    /// with a warning.
    #[serde(deserialize_with = "any_text")]
    pub enabled: Option<String>,
    /// Requests per second each Notion connection may make, shared by every
    /// process (default 3, Notion's average limit).
    #[serde(deserialize_with = "any_text")]
    pub requests_per_second: Option<String>,
    /// Requests per minute each Notion connection may make, shared by every
    /// process (default 120).
    #[serde(deserialize_with = "any_text")]
    pub requests_per_minute: Option<String>,
    /// Most blocks read from one page (default 2000).
    #[serde(deserialize_with = "any_text")]
    pub max_blocks: Option<String>,
    /// Deepest blocks nest when a page is read (default 12).
    #[serde(deserialize_with = "any_text")]
    pub max_depth: Option<String>,
    /// How long one run waits for its connection's request budget before it
    /// gives the page up until later, in seconds (default 300).
    #[serde(deserialize_with = "any_text")]
    pub patience_seconds: Option<String>,
}

/// The Notion adapter's settings, read (defaults for what is unset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotionAdapter {
    pub requests_per_second: u32,
    pub requests_per_minute: u32,
    pub max_blocks: u32,
    pub max_depth: u32,
    pub patience_seconds: u32,
}

impl Default for NotionAdapter {
    fn default() -> Self {
        Self {
            requests_per_second: 3,
            requests_per_minute: 120,
            max_blocks: 2000,
            max_depth: 12,
            patience_seconds: 300,
        }
    }
}

impl NotionConfig {
    /// Whether Notion is asked for: `true`, `1`, `yes` or `on`, in any case.
    /// Anything else, a value of any shape included, is off.
    pub fn is_enabled(&self) -> bool {
        self.enabled.as_deref().is_some_and(|text| {
            matches!(
                text.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            )
        })
    }

    /// The adapter's settings, each checked: the first one that is not
    /// usable is an error naming it. Only called while Notion is on
    /// ([`ConnectorsConfig::check`]); [`adapter`](Self::adapter) reads them
    /// where they are used.
    pub fn read_adapter(&self) -> Result<NotionAdapter, String> {
        let defaults = NotionAdapter::default();
        let number = |value: Option<&str>, name: &str, low: u32, high: u32, default: u32| {
            let Some(text) = value.map(str::trim) else {
                return Ok(default);
            };
            text.parse::<u32>()
                .ok()
                .filter(|n| (low..=high).contains(n))
                .ok_or_else(|| {
                    format!("CAMPBOOKS_CONNECTORS__NOTION__{name} must be {low} to {high}")
                })
        };
        Ok(NotionAdapter {
            requests_per_second: number(
                self.requests_per_second.as_deref(),
                "REQUESTS_PER_SECOND",
                1,
                1000,
                defaults.requests_per_second,
            )?,
            requests_per_minute: number(
                self.requests_per_minute.as_deref(),
                "REQUESTS_PER_MINUTE",
                1,
                100_000,
                defaults.requests_per_minute,
            )?,
            max_blocks: number(
                self.max_blocks.as_deref(),
                "MAX_BLOCKS",
                1,
                20_000,
                defaults.max_blocks,
            )?,
            max_depth: number(
                self.max_depth.as_deref(),
                "MAX_DEPTH",
                1,
                32,
                defaults.max_depth,
            )?,
            patience_seconds: number(
                self.patience_seconds.as_deref(),
                "PATIENCE_SECONDS",
                1,
                3600,
                defaults.patience_seconds,
            )?,
        })
    }

    /// The adapter's settings as used: what [`read_adapter`](Self::read_adapter)
    /// reads, or the defaults where a value is unusable (never the case
    /// while Notion is on: the server would not have started).
    pub fn adapter(&self) -> NotionAdapter {
        self.read_adapter().unwrap_or_default()
    }
}

/// `CAMPBOOKS_NOTION__*`: the Notion integration this instance is registered
/// as. Read only while Notion is offered, and tolerantly: every value is
/// text, whatever its shape.
#[derive(Debug, Clone, Default)]
pub struct NotionClientConfig {
    /// `…__CLIENT_ID`.
    pub client_id: Option<String>,
    /// `…__CLIENT_SECRET`, inline; for development.
    pub client_secret: Option<SecretString>,
    /// `…__CLIENT_SECRET_FILE`.
    pub client_secret_file: Option<PathBuf>,
    /// `…__BASE_URL`: serve every endpoint from this base instead (a local
    /// fake). Development only.
    pub base_url: Option<String>,
    /// `…__REDIRECT_PATH`: where Notion sends members back, registered with
    /// the integration (default `/auth/callback/notion`).
    pub redirect_path: Option<String>,
    /// `…__WEBHOOK_SECRET`, inline; for development: the verification token
    /// of the Notion webhook subscription.
    pub webhook_secret: Option<SecretString>,
    /// `…__WEBHOOK_SECRET_FILE`.
    pub webhook_secret_file: Option<PathBuf>,
}

/// The raw shape of [`NotionClientConfig`]: every value as text.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawNotionClient {
    #[serde(deserialize_with = "any_text")]
    client_id: Option<String>,
    #[serde(deserialize_with = "any_text")]
    client_secret: Option<String>,
    #[serde(deserialize_with = "any_text")]
    client_secret_file: Option<String>,
    #[serde(deserialize_with = "any_text")]
    base_url: Option<String>,
    #[serde(deserialize_with = "any_text")]
    redirect_path: Option<String>,
    #[serde(deserialize_with = "any_text")]
    webhook_secret: Option<String>,
    #[serde(deserialize_with = "any_text")]
    webhook_secret_file: Option<String>,
}

impl NotionClientConfig {
    /// Load the `notion` section of `figment`: an empty setting is no
    /// setting.
    pub fn from_figment(figment: &Figment) -> Result<Self, String> {
        if !figment.contains("notion") {
            return Ok(Self::default());
        }
        let raw: RawNotionClient = figment
            .extract_inner("notion")
            .map_err(|e| format!("CAMPBOOKS_NOTION__*: {e}"))?;
        let text = |value: Option<String>| {
            value
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        Ok(Self {
            client_id: text(raw.client_id),
            client_secret: text(raw.client_secret).map(SecretString::from),
            client_secret_file: text(raw.client_secret_file).map(PathBuf::from),
            base_url: text(raw.base_url),
            redirect_path: text(raw.redirect_path),
            webhook_secret: text(raw.webhook_secret).map(SecretString::from),
            webhook_secret_file: text(raw.webhook_secret_file).map(PathBuf::from),
        })
    }
}

/// Any value as text, so its shape never stops a server that does not use
/// it: a scalar as written, anything else as nothing (checked where used).
fn any_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Text(String),
        Integer(i64),
        Float(f64),
        Bool(bool),
        Other(serde::de::IgnoredAny),
    }
    Ok(Some(match Raw::deserialize(deserializer)? {
        Raw::Text(text) => text,
        Raw::Integer(number) => number.to_string(),
        Raw::Float(number) => number.to_string(),
        Raw::Bool(value) => value.to_string(),
        Raw::Other(_) => String::new(),
    }))
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
    /// Offer IMAP connections and read them (default true). Needs the
    /// instance secret key: without one IMAP is not offered, and setting
    /// this to true explicitly stops the server (`Deps::from_figment`).
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
    /// Messages one mailbox login sends an hour at most, across every
    /// workspace and connection it is in (default 50).
    pub send_per_hour: u32,
    /// Recipients one message may have at most (default 50).
    pub send_recipients_per_message: u32,
    /// Recipients one mailbox login sends to a day at most (default 500).
    pub send_recipients_per_day: u32,
    /// Messages one workspace sends an hour at most through IMAP mailboxes
    /// (default 200).
    pub send_workspace_per_hour: u32,
    /// Messages sent an hour at most through one SMTP host, every workspace
    /// together (default 1000).
    pub send_host_per_hour: u32,
    /// The largest message sent, in bytes as it goes out (default 25 MiB, at
    /// most 35 MiB).
    pub send_max_message_bytes: u64,
    /// How long one submission may take in all, connecting to the server's
    /// last answer, in seconds (default 300, at most 600: within the login's
    /// 15-minute SMTP session slot).
    pub send_timeout_seconds: u32,
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
            calendar: CalendarConfig::default(),
            microsoft: MicrosoftMailConfig::default(),
            notion: NotionConfig::default(),
            zoho_calendar: false,
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

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 300,
        }
    }
}

impl Default for ImapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
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
            send_per_hour: 50,
            send_recipients_per_message: 50,
            send_recipients_per_day: 500,
            send_workspace_per_hour: 200,
            send_host_per_hour: 1_000,
            send_max_message_bytes: 25 * 1024 * 1024,
            send_timeout_seconds: 300,
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
        if self.calendar.requests_per_minute == 0 {
            return Err(
                "CAMPBOOKS_CONNECTORS__CALENDAR__REQUESTS_PER_MINUTE must be positive".to_owned(),
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
        // Checked only where IMAP runs: a stray value never stops an instance
        // that does not read IMAP.
        if imap.enabled && (imap.max_created_folders == 0 || imap.max_created_folders > 100) {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS must be 1 to 100".to_owned(),
            );
        }
        if imap.enabled {
            self.check_send()?;
        }
        self.check_microsoft()?;
        self.check_notion()?;
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
    /// The Microsoft mail settings: checked only while Microsoft mail is on,
    /// so a stray value never stops an instance that does not offer it.
    fn check_microsoft(&self) -> Result<(), String> {
        if let Some(tenant) = &self.microsoft.tenant
            && self.microsoft.enabled
            && !crate::microsoft::is_tenant(tenant.trim())
        {
            return Err(format!(
                "CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT={tenant}: use common, organizations, \
                 consumers or a tenant id (or turn CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED off)"
            ));
        }
        if self.microsoft.enabled {
            // Tied to no poll bound: the poll bounds never stop it.
            self.microsoft.read_adapter().map_err(|e| {
                format!("{e} (or turn CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED off)")
            })?;
        }
        Ok(())
    }

    /// The Notion settings: checked only while Notion is on, so a stray value
    /// never stops an instance that does not offer it.
    fn check_notion(&self) -> Result<(), String> {
        if self.notion.is_enabled() {
            // Tied to no poll bound: the poll bounds never stop it.
            self.notion
                .read_adapter()
                .map_err(|e| format!("{e} (or turn CAMPBOOKS_CONNECTORS__NOTION__ENABLED off)"))?;
        }
        Ok(())
    }

    /// The IMAP send settings: checked only where IMAP runs, and tied to no
    /// poll bound.
    fn check_send(&self) -> Result<(), String> {
        let imap = &self.imap;
        if imap.send_per_hour == 0 || imap.send_per_hour > 10_000 {
            return Err("CAMPBOOKS_CONNECTORS__IMAP__SEND_PER_HOUR must be 1 to 10000".to_owned());
        }
        if imap.send_recipients_per_message == 0 || imap.send_recipients_per_message > 1_000 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_RECIPIENTS_PER_MESSAGE must be 1 to 1000"
                    .to_owned(),
            );
        }
        if imap.send_recipients_per_day < imap.send_recipients_per_message
            || imap.send_recipients_per_day > 100_000
        {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_RECIPIENTS_PER_DAY must be at least \
                 …__SEND_RECIPIENTS_PER_MESSAGE and at most 100000"
                    .to_owned(),
            );
        }
        if imap.send_workspace_per_hour == 0 || imap.send_workspace_per_hour > 100_000 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_WORKSPACE_PER_HOUR must be 1 to 100000"
                    .to_owned(),
            );
        }
        if imap.send_host_per_hour == 0 || imap.send_host_per_hour > 1_000_000 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_HOST_PER_HOUR must be 1 to 1000000".to_owned(),
            );
        }
        let max =
            u64::try_from(crate::features::send_message::MAX_MESSAGE_BYTES).unwrap_or(u64::MAX);
        if imap.send_max_message_bytes < 64 * 1024 || imap.send_max_message_bytes > max {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_MAX_MESSAGE_BYTES must be 65536 to 36700160 \
                 (35 MiB)"
                    .to_owned(),
            );
        }
        // A submission ends well within the 15 minutes its login's SMTP
        // session slot takes to come back, so it never outlives the slot.
        if imap.send_timeout_seconds < imap.timeout_seconds || imap.send_timeout_seconds > 600 {
            return Err(
                "CAMPBOOKS_CONNECTORS__IMAP__SEND_TIMEOUT_SECONDS must be at least \
                 …__TIMEOUT_SECONDS and at most 600"
                    .to_owned(),
            );
        }
        Ok(())
    }

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

/// A provider's OAuth client, `CAMPBOOKS_GOOGLE__*`, `CAMPBOOKS_MICROSOFT__*`
/// or `CAMPBOOKS_ZOHO__*`.
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
    /// Load the `section` (`google`, `microsoft`, `zoho`) of `figment`.
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
    fn notion_is_off_by_default_and_never_stops_a_server_that_does_not_offer_it() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert!(!config.notion.is_enabled());
        assert_eq!(config.notion.adapter(), NotionAdapter::default());
        // Production's settings: its poll bounds and no Notion variable.
        let production = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}}
        })))
        .unwrap();
        assert!(!production.notion.is_enabled());
        // Off: any shape of any setting starts.
        for stray in [
            serde_json::json!({"requests_per_second": "lots", "max_blocks": -1}),
            serde_json::json!({"enabled": false, "max_depth": 0, "patience_seconds": [1]}),
            serde_json::json!({"enabled": "nope", "unknown_key": {"a": 1}}),
            serde_json::json!({"enabled": 0, "requests_per_minute": {"a": 1}}),
        ] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}, "notion": stray}
            })))
            .unwrap();
            assert!(!config.notion.is_enabled());
        }
    }

    #[test]
    fn notion_is_on_for_the_usual_spellings_and_its_settings_are_read() {
        for spelling in ["true", "TRUE", " 1 ", "yes", "On"] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"notion": {"enabled": spelling}}
            })))
            .unwrap();
            assert!(config.notion.is_enabled(), "{spelling}");
        }
        let on = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {
                "poll": {"min_seconds": 60, "max_seconds": 120},
                "notion": {"enabled": true, "requests_per_second": "2", "requests_per_minute": 90,
                           "max_blocks": 500, "max_depth": 6, "patience_seconds": 60}
            }
        })))
        .unwrap();
        assert_eq!(
            on.notion.adapter(),
            NotionAdapter {
                requests_per_second: 2,
                requests_per_minute: 90,
                max_blocks: 500,
                max_depth: 6,
                patience_seconds: 60,
            }
        );
    }

    #[test]
    fn notion_with_a_broken_value_is_refused_naming_it() {
        for (bad, name) in [
            (serde_json::json!({"requests_per_second": 0}), "REQUESTS_PER_SECOND"),
            (serde_json::json!({"requests_per_second": 5000}), "REQUESTS_PER_SECOND"),
            (serde_json::json!({"requests_per_minute": "many"}), "REQUESTS_PER_MINUTE"),
            (serde_json::json!({"max_blocks": 0}), "MAX_BLOCKS"),
            (serde_json::json!({"max_blocks": 100_000}), "MAX_BLOCKS"),
            (serde_json::json!({"max_depth": 99}), "MAX_DEPTH"),
            (serde_json::json!({"patience_seconds": 0}), "PATIENCE_SECONDS"),
        ] {
            let mut settings = bad.clone();
            settings["enabled"] = serde_json::json!(true);
            let refused = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"notion": settings}
            })))
            .unwrap_err();
            assert!(refused.contains(name), "{bad}: {refused}");
            assert!(refused.contains("NOTION__ENABLED"), "{refused}");
        }
    }

    #[test]
    fn the_notion_integration_is_read_as_text_whatever_its_shape() {
        let none = NotionClientConfig::from_figment(&Figment::new()).unwrap();
        assert!(none.client_id.is_none() && none.client_secret.is_none());
        let client = NotionClientConfig::from_figment(&figment(serde_json::json!({
            "notion": {
                "client_id": "  notion-client  ", "client_secret": "s3cret",
                "redirect_path": "/auth/callback/notion", "webhook_secret": "w",
                "client_secret_file": "", "base_url": 7
            }
        })))
        .unwrap();
        assert_eq!(client.client_id.as_deref(), Some("notion-client"));
        assert!(client.client_secret.is_some() && client.webhook_secret.is_some());
        // An empty setting is no setting.
        assert!(client.client_secret_file.is_none());
        assert_eq!(client.base_url.as_deref(), Some("7"));
        // The secret never shows in a debug print.
        assert!(!format!("{client:?}").contains("s3cret"));
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
        for section in ["google", "microsoft", "zoho"] {
            let client = OAuthClientConfig::from_figment(&Figment::new(), section).unwrap();
            assert!(client.client_id.is_none() && client.client_secret_file.is_none());
        }
    }

    #[test]
    fn imap_is_on_by_default_and_its_settings_are_checked() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert!(config.imap.enabled);
        // IDLE stays off.
        assert!(!config.imap.idle_enabled);
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
                "connectors": {"imap": {"enabled": true, "max_created_folders": bad}}
            })))
            .unwrap_err();
            assert!(refused.contains("MAX_CREATED_FOLDERS"), "{refused}");
            // IMAP off: it starts, whatever the value.
            ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"imap": {"enabled": false, "max_created_folders": bad}}
            })))
            .unwrap();
            // On by default: the value is checked.
            ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"imap": {"max_created_folders": bad}}
            })))
            .unwrap_err();
        }
        // The defaults start with prod's narrowed poll bounds, IMAP on or off.
        for enabled in [false, true] {
            ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {
                    "poll": {"min_seconds": 60, "max_seconds": 120},
                    "imap": {"enabled": enabled}
                }
            })))
            .unwrap();
        }
    }

    #[test]
    fn imap_send_settings_are_checked_only_while_imap_is_on() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        let imap = &config.imap;
        assert_eq!(
            (
                imap.send_per_hour,
                imap.send_recipients_per_message,
                imap.send_recipients_per_day,
                imap.send_workspace_per_hour,
                imap.send_host_per_hour,
                imap.send_max_message_bytes,
                imap.send_timeout_seconds,
            ),
            (50, 50, 500, 200, 1_000, 25 * 1024 * 1024, 300)
        );
        for (bad, setting) in [
            (serde_json::json!({"send_per_hour": 0}), "SEND_PER_HOUR"),
            (
                serde_json::json!({"send_recipients_per_message": 0}),
                "SEND_RECIPIENTS_PER_MESSAGE",
            ),
            (
                serde_json::json!({"send_recipients_per_day": 10}),
                "SEND_RECIPIENTS_PER_DAY",
            ),
            (
                serde_json::json!({"send_workspace_per_hour": 0}),
                "SEND_WORKSPACE_PER_HOUR",
            ),
            (
                serde_json::json!({"send_host_per_hour": 0}),
                "SEND_HOST_PER_HOUR",
            ),
            (
                serde_json::json!({"send_max_message_bytes": 1024}),
                "SEND_MAX_MESSAGE_BYTES",
            ),
            (
                serde_json::json!({"send_max_message_bytes": 64_u64 << 20}),
                "SEND_MAX_MESSAGE_BYTES",
            ),
            (
                serde_json::json!({"send_timeout_seconds": 5}),
                "SEND_TIMEOUT_SECONDS",
            ),
            (
                serde_json::json!({"send_timeout_seconds": 601}),
                "SEND_TIMEOUT_SECONDS",
            ),
        ] {
            let mut on = bad.clone();
            on["enabled"] = true.into();
            let refused = ConnectorsConfig::from_figment(&figment(
                serde_json::json!({"connectors": {"imap": on}}),
            ))
            .unwrap_err();
            assert!(refused.contains(setting), "{refused}");
            // IMAP off: it starts, whatever the value (the 2026-10-23 outage).
            let mut off = bad.clone();
            off["enabled"] = false.into();
            ConnectorsConfig::from_figment(&figment(
                serde_json::json!({"connectors": {"imap": off}}),
            ))
            .unwrap();
        }
        // The defaults start with production's poll bounds, IMAP on or off,
        // and IDLE on or off.
        for (enabled, idle) in [(false, false), (true, false), (false, true)] {
            ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {
                    "poll": {"min_seconds": 60, "max_seconds": 120},
                    "imap": {"enabled": enabled, "idle_enabled": idle}
                }
            })))
            .unwrap();
        }
        let read = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"imap": {"enabled": true, "send_per_hour": 5, "send_timeout_seconds": 30}}
        })))
        .unwrap();
        assert_eq!(read.imap.send_per_hour, 5);
        assert_eq!(read.imap.send_timeout_seconds, 30);
    }

    #[test]
    fn production_settings_start_with_imap_on_by_default() {
        // Production sets only its poll bounds (60 to 120 seconds) and no
        // IMAP variable (campbooks-cloud#314, the 2026-10-23 outage): IMAP
        // is on with every default, IDLE off, and the settings pass.
        let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}}
        })))
        .unwrap();
        assert!(config.imap.enabled);
        assert!(!config.imap.idle_enabled);
        assert_eq!(
            (config.poll.min_seconds, config.poll.max_seconds),
            (60, 120)
        );
        assert_eq!(config.imap, ImapConfig::default());
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
        assert!(config.imap.enabled && !config.imap.idle_enabled);
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

    #[test]
    fn microsoft_mail_is_off_by_default_and_checked_only_while_on() {
        let config = ConnectorsConfig::from_figment(&Figment::new()).unwrap();
        assert!(!config.microsoft.enabled);
        // Unset: sign-in's tenant, resolved where the client is built.
        assert_eq!(config.microsoft.tenant, None);
        let on = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"microsoft": {"enabled": true, "tenant": "organizations"}}
        })))
        .unwrap();
        assert!(on.microsoft.enabled);
        assert_eq!(on.microsoft.tenant.as_deref(), Some("organizations"));
        for bad in ["not a tenant", "../evil", ""] {
            // Off: it starts, whatever the value.
            let off = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"microsoft": {"enabled": false, "tenant": bad}}
            })))
            .unwrap();
            assert!(!off.microsoft.enabled);
            ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"microsoft": {"tenant": bad}}
            })))
            .unwrap();
            // On: the only setting that stops the server.
            let refused = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"microsoft": {"enabled": true, "tenant": bad}}
            })))
            .unwrap_err();
            assert!(
                refused.contains("CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT"),
                "{refused}"
            );
        }
        // Production's settings (no Microsoft mail variable, its poll
        // bounds) start, and so do they with the flag on.
        for microsoft in [
            serde_json::json!({}),
            serde_json::json!({"enabled": false}),
            serde_json::json!({"enabled": true}),
        ] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {
                    "poll": {"min_seconds": 60, "max_seconds": 120},
                    "microsoft": microsoft
                }
            })))
            .unwrap();
            assert_eq!(
                (config.poll.min_seconds, config.poll.max_seconds),
                (60, 120)
            );
        }
        // A tenant of any shape, or a key it does not know, never stops a
        // server with the flag off.
        for stray in [
            serde_json::json!({"tenant": 1}),
            serde_json::json!({"tenant": ["common"]}),
            serde_json::json!({"client_id": "m", "enable": true}),
        ] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"microsoft": stray}
            })))
            .unwrap();
            assert!(!config.microsoft.enabled);
        }
        let shaped = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"microsoft": {"enabled": true, "tenant": 1}}
        })));
        assert!(shaped.unwrap_err().contains("MICROSOFT__TENANT"));
    }

    #[test]
    fn microsoft_write_back_and_sending_add_no_setting() {
        // Production's settings (cloud#317): its poll bounds and no Microsoft
        // mail variable. Writing back and sending read nothing of their own,
        // so neither a stray key nor Microsoft mail turned on stops a server.
        for microsoft in [
            serde_json::json!({}),
            serde_json::json!({"send_max_bytes": "lots", "write_back": "maybe"}),
            serde_json::json!({"enabled": true, "send_max_bytes": -1, "write_back": [1]}),
        ] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {
                    "poll": {"min_seconds": 60, "max_seconds": 120},
                    "microsoft": microsoft
                }
            })))
            .unwrap();
            assert_eq!(config.poll.max_seconds, 120);
            assert_eq!(config.microsoft.adapter(), MicrosoftAdapter::default());
        }
    }

    #[test]
    fn microsoft_adapter_settings_never_stop_a_server_that_does_not_offer_it() {
        // Production's settings (cloud#316): its poll bounds, no Microsoft
        // mail, subscription or callback variable.
        let production = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}}
        })))
        .unwrap();
        assert!(!production.microsoft.enabled);
        assert_eq!(production.microsoft.adapter(), MicrosoftAdapter::default());
        assert!(!production.microsoft.adapter().notifications);
        // Off: any shape of any adapter setting starts.
        for stray in [
            serde_json::json!({"max_folders": "lots", "notifications": "maybe"}),
            serde_json::json!({"backfill_days": 0, "requests_per_minute": -3}),
            serde_json::json!({"max_message_bytes": ["big"], "run_budget_bytes": {"a": 1}}),
            serde_json::json!({"enabled": false, "max_folders": 0, "notifications": 7}),
        ] {
            let config = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}, "microsoft": stray}
            })))
            .unwrap();
            assert!(!config.microsoft.enabled);
            assert_eq!(config.microsoft.adapter(), MicrosoftAdapter::default());
        }
        // On: read, with production's poll bounds whatever they are.
        let on = ConnectorsConfig::from_figment(&figment(serde_json::json!({
            "connectors": {
                "poll": {"min_seconds": 60, "max_seconds": 120},
                "microsoft": {"enabled": true, "max_folders": "40", "notifications": true,
                              "backfill_days": 30, "requests_per_minute": 100}
            }
        })))
        .unwrap();
        let adapter = on.microsoft.adapter();
        assert_eq!(
            (
                adapter.max_folders,
                adapter.backfill_days,
                adapter.requests_per_minute
            ),
            (40, 30, 100)
        );
        assert!(adapter.notifications);
        // On with a broken value: refused, naming it.
        for (bad, name) in [
            (serde_json::json!({"max_folders": 0}), "MAX_FOLDERS"),
            (
                serde_json::json!({"max_message_bytes": 100 * 1024 * 1024}),
                "MAX_MESSAGE_BYTES",
            ),
            (serde_json::json!({"max_folders": "lots"}), "MAX_FOLDERS"),
            (serde_json::json!({"backfill_days": 4000}), "BACKFILL_DAYS"),
            (
                serde_json::json!({"notifications": "maybe"}),
                "NOTIFICATIONS",
            ),
            (
                serde_json::json!({"requests_per_minute": 0}),
                "REQUESTS_PER_MINUTE",
            ),
        ] {
            let mut microsoft = bad;
            microsoft["enabled"] = true.into();
            let refused = ConnectorsConfig::from_figment(&figment(serde_json::json!({
                "connectors": {"microsoft": microsoft}
            })))
            .unwrap_err();
            assert!(refused.contains(name), "{refused}");
        }
    }
}
