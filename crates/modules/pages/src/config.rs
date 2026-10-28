//! The module's settings, `CAMPBOOKS_PAGES__*`.
//!
//! Pages are built but not yet switched on for everyone, so they ship off:
//! `CAMPBOOKS_PAGES__ENABLED=true` turns them on. While off, every pages
//! endpoint answers 404 and no pages schedule runs. Tables in pages are a
//! second, separate switch, `CAMPBOOKS_PAGES__TABLES__ENABLED=true`, also off
//! by default: while off, there are no table routes and no table is listed
//! with the pages. What a setting says is read tolerantly: a missing,
//! misspelled or oddly shaped value means off and never stops the server
//! from starting.

use campbooks_kernel::config::env_source;
use figment::Figment;
use serde_json::Value;

/// What the module is composed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deps {
    /// Whether pages are on (`CAMPBOOKS_PAGES__ENABLED`, default false).
    pub enabled: bool,
    /// Whether tables are on (`CAMPBOOKS_PAGES__TABLES__ENABLED`, default
    /// false). They need pages on too.
    pub tables: bool,
    /// Whether Notion databases are brought in as tables
    /// (`CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED`, default false).
    /// Effective only with Notion (`CAMPBOOKS_CONNECTORS__NOTION__ENABLED`),
    /// pages and tables on; the connectors module reads the same switches.
    pub databases: bool,
}

/// Whether tables are on, for a handler to read from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablesOn(pub bool);

/// Whether Notion databases are on (a switch of its own, effective only with
/// pages and tables on), for a handler to read from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabasesOn(pub bool);

/// The catalogues (OpenAPI, events, scopes) and tests compose the module
/// switched on; a server reads the setting with [`Deps::from_env`].
impl Default for Deps {
    fn default() -> Self {
        Self {
            enabled: true,
            tables: true,
            databases: true,
        }
    }
}

impl Deps {
    /// Read `CAMPBOOKS_PAGES__ENABLED` and `CAMPBOOKS_PAGES__TABLES__ENABLED`
    /// from the environment.
    pub fn from_env() -> Self {
        Self::from_figment(&env_source())
    }

    /// Read the `pages` section of any figment source. Never fails.
    pub fn from_figment(figment: &Figment) -> Self {
        let enabled = switch(figment, "pages.enabled");
        let tables = switch(figment, "pages.tables.enabled");
        Self {
            enabled,
            tables,
            databases: enabled
                && tables
                && switch(figment, "connectors.notion.enabled")
                && switch(figment, "connectors.notion.databases.enabled"),
        }
    }
}

/// One switch: on only when it says so plainly.
fn switch(figment: &Figment, key: &str) -> bool {
    let value: Option<Value> = figment.extract_inner(key).ok();
    match value {
        Some(Value::Bool(on)) => on,
        Some(Value::String(text)) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
        Some(Value::Number(number)) => number.as_i64() == Some(1),
        _ => false,
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
    fn tables_are_off_unless_asked_for_and_are_a_switch_of_their_own() {
        assert!(!Deps::from_figment(&Figment::new()).tables);
        assert!(!Deps::from_figment(&figment(json!({"pages": {"enabled": true}}))).tables);
        let on = Deps::from_figment(&figment(
            json!({"pages": {"enabled": true, "tables": {"enabled": "true"}}}),
        ));
        assert!(on.enabled && on.tables);
        let only_tables =
            Deps::from_figment(&figment(json!({"pages": {"tables": {"enabled": 1}}})));
        assert!(!only_tables.enabled && only_tables.tables);
    }

    #[test]
    fn notion_databases_need_notion_pages_and_tables_and_their_own_switch() {
        let on = |connectors: Value, pages: Value| {
            Deps::from_figment(&figment(json!({"connectors": connectors, "pages": pages})))
        };
        let all = json!({"enabled": true, "tables": {"enabled": true}});
        let asked = json!({"notion": {"enabled": true, "databases": {"enabled": "true"}}});
        assert!(on(asked.clone(), all.clone()).databases);
        // Each of the four is needed.
        assert!(!on(json!({"notion": {"enabled": true}}), all.clone()).databases);
        assert!(
            !on(
                json!({"notion": {"databases": {"enabled": true}}}),
                all.clone()
            )
            .databases
        );
        assert!(!on(asked.clone(), json!({"enabled": true})).databases);
        assert!(!on(asked, json!({"tables": {"enabled": true}})).databases);
        assert!(!Deps::from_figment(&Figment::new()).databases);
        // Stray shapes never stop the server.
        for stray in [
            json!({"notion": "nope"}),
            json!({"notion": {"databases": 7}}),
            json!({"notion": {"enabled": true, "databases": {"enabled": ["yes"]}}}),
            json!({"notion": {"enabled": {"a": 1}, "databases": {"enabled": {"b": 2}}}}),
        ] {
            assert!(!on(stray, all.clone()).databases);
        }
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
            json!({"pages": {"tables": "nope"}}),
            json!({"pages": {"tables": 7}}),
            json!({"pages": {"tables": ["enabled"]}}),
            json!({"pages": {"tables": {"enabled": ["yes"]}}}),
            json!({"pages": {"tables": {"enabled": {"a": 1}}}}),
            json!({"pages": {"tables": {"enabled": "maybe", "colour": "blue"}}}),
            json!({"pages": {"tables": {"enabled": null}}}),
        ] {
            let deps = Deps::from_figment(&figment(settings));
            assert!(!deps.enabled && !deps.tables);
        }
    }
}
