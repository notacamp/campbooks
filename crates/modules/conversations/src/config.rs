//! The module's settings, `CAMPBOOKS_CONVERSATIONS__*`.

use campbooks_kernel::config::env_source;
use figment::Figment;
use serde::Deserialize;

/// Messages sent longer ago than this get no model pass (a first sync's
/// backfill), unless configured otherwise.
pub const DEFAULT_DECIDE_BACKFILL_DAYS: u32 = 14;

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
}

const fn default_backfill_days() -> u32 {
    DEFAULT_DECIDE_BACKFILL_DAYS
}

impl Default for ConversationsConfig {
    fn default() -> Self {
        Self {
            decide_backfill_days: DEFAULT_DECIDE_BACKFILL_DAYS,
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
        figment
            .extract_inner("conversations")
            .map_err(|e| format!("CAMPBOOKS_CONVERSATIONS__*: {e}"))
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
