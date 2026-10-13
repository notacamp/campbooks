//! The module's settings, `CAMPBOOKS_CONVERSATIONS__*`.

use campbooks_kernel::config::env_source;
use figment::Figment;
use serde::Deserialize;

/// Messages sent longer ago than this get no model pass (a first sync's
/// backfill), unless configured otherwise.
pub const DEFAULT_DECIDE_BACKFILL_DAYS: u32 = 14;

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
