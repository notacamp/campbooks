//! The module's settings, `CAMPBOOKS_CONVERSATIONS__*`.

use campbooks_kernel::config::env_source;
use figment::Figment;
use serde::Deserialize;

/// Messages sent longer ago than this get no model pass (a first sync's
/// backfill), unless configured otherwise.
pub const DEFAULT_DECIDE_BACKFILL_DAYS: u32 = 14;

/// Only inbound messages sent within this many days raise a need (attention),
/// unless configured otherwise.
pub const DEFAULT_NEEDS_WINDOW_DAYS: u32 = 14;

/// Seconds a send waits before it leaves, so it can be taken back, unless
/// configured otherwise.
pub const DEFAULT_UNDO_SEND_SECONDS: u32 = 10;

/// The longest undo window that may be configured.
pub const MAX_UNDO_SEND_SECONDS: u32 = 60;

/// Settings of the `conversations` module.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationsConfig {
    /// The model pass skips inbound messages sent more than this many days
    /// ago, so importing an old mailbox costs no AI
    /// (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`, default 14). Rules
    /// still sort every message.
    #[serde(default = "default_backfill_days")]
    pub decide_backfill_days: u32,
    /// How long a send waits before it leaves, so the member can take it
    /// back (`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS`, default 10, at
    /// most 60; 0 sends at once).
    #[serde(default = "default_undo_send_seconds")]
    pub undo_send_seconds: u32,
    /// Only inbound messages sent within this many days raise a need on the
    /// members' Home, so a first sync of old mail fills nothing
    /// (`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS`, default 14).
    #[serde(default = "default_needs_window_days")]
    pub needs_window_days: u32,
    /// When Scout notices mail the member gets a lot of and offers a smart
    /// rule for it (`CAMPBOOKS_CONVERSATIONS__NOTICES__*`).
    #[serde(default)]
    pub notices: NoticeSettings,
}

/// The thresholds of "Scout noticed" (campbooks-cloud#169). Priors, to be
/// tuned on the owner's mail: nothing here was measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoticeSettings {
    /// How far back a sender's mail is looked at, in days (`WINDOW_DAYS`,
    /// default 60).
    pub window_days: u32,
    /// Conversations from one sender in that window before anything is
    /// noticed (`MIN_MESSAGES`, default 8).
    pub min_messages: u32,
    /// Distinct days they came on, at least: mail that keeps coming, not a
    /// burst (`MIN_DAYS`, default 4).
    pub min_days: u32,
    /// "Keep them off Home" when the member opened at most this share of
    /// them, in percent (`OPENED_PERCENT_MAX`, default 25).
    pub opened_percent_max: u32,
    /// "Archive them as they arrive" when the member archived or deleted at
    /// least this share of them unread, in percent
    /// (`ARCHIVED_UNREAD_PERCENT_MIN`, default 70).
    pub archived_unread_percent_min: u32,
    /// An inbox is looked at again at most this often, in hours
    /// (`EVERY_HOURS`, default 24).
    pub every_hours: u32,
    /// "Not now" hides a suggestion for this many days (`NOT_NOW_DAYS`,
    /// default 14).
    pub not_now_days: u32,
    /// Most suggestions kept per inbox (`MAX_PER_INBOX`, default 5).
    pub max_per_inbox: u32,
}

impl Default for NoticeSettings {
    fn default() -> Self {
        Self {
            window_days: 60,
            min_messages: 8,
            min_days: 4,
            opened_percent_max: 25,
            archived_unread_percent_min: 70,
            every_hours: 24,
            not_now_days: 14,
            max_per_inbox: 5,
        }
    }
}

const fn default_needs_window_days() -> u32 {
    DEFAULT_NEEDS_WINDOW_DAYS
}

const fn default_undo_send_seconds() -> u32 {
    DEFAULT_UNDO_SEND_SECONDS
}

const fn default_backfill_days() -> u32 {
    DEFAULT_DECIDE_BACKFILL_DAYS
}

impl Default for ConversationsConfig {
    fn default() -> Self {
        Self {
            decide_backfill_days: DEFAULT_DECIDE_BACKFILL_DAYS,
            undo_send_seconds: DEFAULT_UNDO_SEND_SECONDS,
            needs_window_days: DEFAULT_NEEDS_WINDOW_DAYS,
            notices: NoticeSettings::default(),
        }
    }
}

impl ConversationsConfig {
    /// Load from `CAMPBOOKS_CONVERSATIONS__*`.
    pub fn from_env() -> Result<Self, String> {
        Self::from_figment(&env_source())
    }

    /// Load the `conversations` section of any figment source.
    pub fn from_figment(figment: &Figment) -> Result<Self, String> {
        if !figment.contains("conversations") {
            return Ok(Self::default());
        }
        let config: Self = figment
            .extract_inner("conversations")
            .map_err(|e| format!("CAMPBOOKS_CONVERSATIONS__*: {e}"))?;
        if config.undo_send_seconds > MAX_UNDO_SEND_SECONDS {
            return Err(format!(
                "CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS={}: at most {MAX_UNDO_SEND_SECONDS}",
                config.undo_send_seconds
            ));
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;

    use super::*;

    #[test]
    fn defaults_to_fourteen_days() {
        let config = ConversationsConfig::from_figment(&Figment::new()).unwrap();
        assert_eq!(config.decide_backfill_days, 14);
        assert_eq!(config.undo_send_seconds, 10);
        assert_eq!(config.needs_window_days, 14);
    }

    #[test]
    fn reads_and_bounds_the_undo_window() {
        let at = |seconds: u32| {
            ConversationsConfig::from_figment(&Figment::from(Serialized::defaults(
                serde_json::json!({"conversations": {"undo_send_seconds": seconds}}),
            )))
        };
        assert_eq!(at(0).unwrap().undo_send_seconds, 0);
        assert_eq!(at(30).unwrap().undo_send_seconds, 30);
        assert!(at(61).unwrap_err().contains("UNDO_SEND_SECONDS"));
    }

    #[test]
    fn reads_the_backfill_window() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "conversations": {"decide_backfill_days": 3}
        })));
        let config = ConversationsConfig::from_figment(&figment).unwrap();
        assert_eq!(config.decide_backfill_days, 3);
    }

    #[test]
    fn rejects_unknown_settings() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "conversations": {"decide_backfil_days": 3}
        })));
        assert!(ConversationsConfig::from_figment(&figment).is_err());
    }
}
