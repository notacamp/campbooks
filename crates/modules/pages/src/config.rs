//! The module's settings, `CAMPBOOKS_PAGES__*`.
//!
//! Pages are built but not yet switched on for everyone, so they ship off:
//! `CAMPBOOKS_PAGES__ENABLED=true` turns them on. While off, every pages
//! endpoint answers 404 and no pages schedule runs. What the setting says is
//! read tolerantly: a missing, misspelled or oddly shaped value means off and
//! never stops the server from starting.

use campbooks_kernel::config::env_source;
use figment::Figment;
use serde_json::Value;

/// What the module is composed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deps {
    /// Whether pages are on (`CAMPBOOKS_PAGES__ENABLED`, default false).
    pub enabled: bool,
}

/// The catalogues (OpenAPI, events, scopes) and tests compose the module
/// switched on; a server reads the setting with [`Deps::from_env`].
impl Default for Deps {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Deps {
    /// Read `CAMPBOOKS_PAGES__ENABLED` from the environment.
    pub fn from_env() -> Self {
        Self::from_figment(&env_source())
    }

    /// Read the `pages` section of any figment source. Never fails.
    pub fn from_figment(figment: &Figment) -> Self {
        let value: Option<Value> = figment.extract_inner("pages.enabled").ok();
        let enabled = match value {
            Some(Value::Bool(on)) => on,
            Some(Value::String(text)) => matches!(
                text.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            ),
            Some(Value::Number(number)) => number.as_i64() == Some(1),
            _ => false,
        };
        Self { enabled }
    }
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;
    use serde_json::json;

    use super::*;

    fn figment(value: Value) -> Figment {
        Figment::from(Serialized::defaults(value))
    }

    #[test]
    fn pages_are_off_unless_asked_for() {
        assert!(!Deps::from_figment(&Figment::new()).enabled);
        assert!(!Deps::from_figment(&figment(json!({}))).enabled);
        assert!(Deps::from_figment(&figment(json!({"pages": {"enabled": true}}))).enabled);
        assert!(Deps::from_figment(&figment(json!({"pages": {"enabled": "True"}}))).enabled);
        assert!(!Deps::from_figment(&figment(json!({"pages": {"enabled": false}}))).enabled);
    }

    #[test]
    fn production_settings_never_stop_a_server_that_does_not_offer_pages() {
        // Production's other settings, and whatever shape a stray pages
        // setting has: the server starts, with pages off.
        for settings in [
            json!({"connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}}}),
            json!({"pages": "nope"}),
            json!({"pages": 7}),
            json!({"pages": ["enabled"]}),
            json!({"pages": {"enabled": ["yes"]}}),
            json!({"pages": {"enabled": {"a": 1}}}),
            json!({"pages": {"enabled": "maybe", "colour": "blue"}}),
            json!({"pages": {"enabled": null}}),
        ] {
            assert!(!Deps::from_figment(&figment(settings)).enabled);
        }
    }
}
