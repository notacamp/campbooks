//! The catalogue of every module's declarations: validated at startup and
//! printed by `campbooks-server declarations` for clients and tools.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use utoipa::openapi::{RefOr, schema::Schema};

use super::{Access, ActionDecl, SchemaDecl};
use crate::{
    Module,
    names::{check_owned_name, is_dotted},
};

/// Every record type, action and event the running system declares, sorted
/// by name, with the JSON Schemas they refer to under `components.schemas`
/// (so `$ref`s resolve inside the document, as in the OpenAPI document).
///
/// This is the contract generated surfaces read: the client's shortcut
/// registry and command menu (label key, shortcut, scope, mutates), and —
/// when they land — MCP tools (name, description, input/output schema,
/// `mutates` as the read-only/destructive hint), assistant tools (the same,
/// plus undo to make its changes proposals) and automation (events as
/// triggers with their payload schema, actions as steps, record types as the
/// targets of `RecordRef`s).
#[derive(Debug, Clone, Serialize)]
pub struct Catalogue {
    /// Declared record types.
    pub records: Vec<RecordEntry>,
    /// Declared actions.
    pub actions: Vec<ActionEntry>,
    /// Declared events.
    pub events: Vec<EventEntry>,
    /// The schemas referred to above.
    pub components: Components,
}

/// A record type in the catalogue.
#[derive(Debug, Clone, Serialize)]
pub struct RecordEntry {
    /// `<module>.<record>`.
    pub name: &'static str,
    /// The declaring module.
    pub module: String,
    /// i18n key of its human name.
    pub label: String,
    /// What decides who may see one.
    pub access: Access,
    /// Its public shape.
    pub schema: RefOr<Schema>,
}

/// An action in the catalogue.
#[derive(Debug, Clone, Serialize)]
pub struct ActionEntry {
    /// `<module>.<verb>`.
    pub name: &'static str,
    /// The declaring module.
    pub module: &'static str,
    /// What it does (English, for people and assistants).
    pub description: &'static str,
    /// i18n key of its label.
    pub label: String,
    /// The scope a token needs to call it.
    pub scope: &'static str,
    /// Whether it changes data.
    pub mutates: bool,
    /// Whether its response carries an undo token.
    pub undoable: bool,
    /// How long its undo tokens stay redeemable.
    pub undo_window_seconds: Option<u64>,
    /// Its default keyboard shortcut.
    pub shortcut: Option<&'static str>,
    /// HTTP method of its endpoint (always `POST`).
    pub method: &'static str,
    /// Its endpoint.
    pub path: String,
    /// Its OpenAPI operation id.
    pub operation_id: String,
    /// Its input (the request body).
    pub input: RefOr<Schema>,
    /// Its output (`result` in the response).
    pub output: RefOr<Schema>,
}

/// An event in the catalogue.
#[derive(Debug, Clone, Serialize)]
pub struct EventEntry {
    /// `<module>.<event>`.
    pub name: &'static str,
    /// The declaring module.
    pub module: String,
    /// Payload schema version.
    pub version: u16,
    /// Its payload.
    pub payload: RefOr<Schema>,
}

/// Shared schemas, as in an OpenAPI document.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Components {
    /// Schemas by component name.
    pub schemas: BTreeMap<String, RefOr<Schema>>,
}

/// The declarations break a rule; each message names the offender.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid module declarations:\n  - {}", .0.join("\n  - "))]
pub struct DeclarationError(pub Vec<String>);

/// Longest key sequence a shortcut may be (`"g i"` is two).
const MAX_SHORTCUT_STEPS: usize = 3;
/// Shortcut modifiers, in the order they must be written.
const MODIFIERS: [&str; 4] = ["mod", "ctrl", "alt", "shift"];
/// Named keys a shortcut may use besides single characters and `f1`–`f12`.
const NAMED_KEYS: [&str; 14] = [
    "enter",
    "escape",
    "tab",
    "space",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
];

impl Catalogue {
    /// Collect and check `modules`' declarations: names are `<module>.<name>`
    /// of the declaring module and unique per kind; actions are described,
    /// require one of their module's own scopes, have a valid shortcut
    /// (unique within the module) and an undo window of at least a second;
    /// events have a version of at least 1; access containers are declared
    /// record types; and no two schemas share a component name.
    pub fn build(modules: &[Module]) -> Result<Self, DeclarationError> {
        let mut errors = Vec::new();
        let mut schemas = SchemaSet::default();
        let mut catalogue = Self {
            records: Vec::new(),
            actions: Vec::new(),
            events: Vec::new(),
            components: Components::default(),
        };

        for module in modules {
            let declarations = &module.declarations;
            for record in declarations.records() {
                let name = record.name();
                if let Err(e) = check_owned_name(module.name, name) {
                    errors.push(format!("record type `{name}`: {e}"));
                }
                schemas.add(
                    &record.schema,
                    &format!("record type `{name}`"),
                    &mut errors,
                );
                catalogue.records.push(RecordEntry {
                    name,
                    module: module.name.to_owned(),
                    label: record.label(),
                    access: record.access(),
                    schema: record.schema.reference(),
                });
            }
            let scopes: BTreeSet<&str> = module.scopes.iter().map(|s| s.name).collect();
            let mut shortcuts = BTreeMap::new();
            for action in declarations.actions() {
                errors.extend(check_action(module.name, action, &scopes, &mut shortcuts));
                let what = format!("action `{}`", action.name());
                schemas.add(&action.input, &what, &mut errors);
                schemas.add(&action.output, &what, &mut errors);
                catalogue.actions.push(ActionEntry::of(action));
            }
            for event in declarations.events() {
                let name = event.name();
                if let Err(e) = check_owned_name(module.name, name) {
                    errors.push(format!("event `{name}`: {e}"));
                }
                if event.version() == 0 {
                    errors.push(format!("event `{name}`: VERSION must be at least 1"));
                }
                schemas.add(&event.payload, &format!("event `{name}`"), &mut errors);
                catalogue.events.push(EventEntry {
                    name,
                    module: module.name.to_owned(),
                    version: event.version(),
                    payload: event.payload.reference(),
                });
            }
        }

        errors.extend(duplicates(
            "record type",
            catalogue.records.iter().map(|r| r.name),
        ));
        errors.extend(duplicates(
            "action",
            catalogue.actions.iter().map(|a| a.name),
        ));
        errors.extend(duplicates("event", catalogue.events.iter().map(|e| e.name)));
        let record_types: BTreeSet<&str> = catalogue.records.iter().map(|r| r.name).collect();
        for record in &catalogue.records {
            if let Access::Container(container) = record.access
                && (!is_dotted(container) || !record_types.contains(container))
            {
                errors.push(format!(
                    "record type `{}`: access container `{container}` is not a declared \
                     record type",
                    record.name
                ));
            }
        }

        if !errors.is_empty() {
            return Err(DeclarationError(errors));
        }
        catalogue.records.sort_by_key(|r| r.name);
        catalogue.actions.sort_by_key(|a| a.name);
        catalogue.events.sort_by_key(|e| e.name);
        catalogue.components.schemas = schemas.0.into_iter().map(|(k, (v, _))| (k, v)).collect();
        Ok(catalogue)
    }
}

impl ActionEntry {
    fn of(action: &ActionDecl) -> Self {
        Self {
            name: action.name(),
            module: action.module(),
            description: action.description(),
            label: action.label(),
            scope: action.scope(),
            mutates: action.mutates(),
            undoable: action.undo_window().is_some(),
            undo_window_seconds: action.undo_window().map(|w| w.as_secs()),
            shortcut: action.shortcut(),
            method: "POST",
            path: action.path(),
            operation_id: action.operation_id(),
            input: action.input.reference(),
            output: action.output.reference(),
        }
    }
}

fn check_action<'a>(
    module: &str,
    action: &'a ActionDecl,
    scopes: &BTreeSet<&str>,
    shortcuts: &mut BTreeMap<&'a str, &'a str>,
) -> Vec<String> {
    let name = action.name();
    let mut errors = Vec::new();
    if let Err(e) = check_owned_name(module, name) {
        errors.push(format!("action `{name}`: {e}"));
    }
    if action.description().trim().is_empty() {
        errors.push(format!(
            "action `{name}`: needs a description (`.describe(…)`)"
        ));
    }
    if !scopes.contains(action.scope()) {
        errors.push(format!(
            "action `{name}`: requires scope `{}`, which module `{module}` does not list in \
             `Module.scopes`",
            action.scope()
        ));
    }
    if let Some(shortcut) = action.shortcut() {
        if let Err(e) = check_shortcut(shortcut) {
            errors.push(format!("action `{name}`: shortcut `{shortcut}`: {e}"));
        }
        if let Some(other) = shortcuts.insert(shortcut, name) {
            errors.push(format!(
                "action `{name}`: shortcut `{shortcut}` is already bound to `{other}`"
            ));
        }
    }
    if action
        .undo_window()
        .is_some_and(|window| window.as_secs() < 1)
    {
        errors.push(format!(
            "action `{name}`: the undo window must be at least one second"
        ));
    }
    errors
}

/// A shortcut is 1–3 chords separated by single spaces; a chord is
/// `[mod+][ctrl+][alt+][shift+]<key>` in that order.
pub(crate) fn check_shortcut(shortcut: &str) -> Result<(), String> {
    let steps: Vec<&str> = shortcut.split(' ').collect();
    if steps.len() > MAX_SHORTCUT_STEPS {
        return Err(format!(
            "at most {MAX_SHORTCUT_STEPS} chords separated by single spaces"
        ));
    }
    for step in steps {
        let mut parts: Vec<&str> = step.split('+').collect();
        let key = parts.pop().unwrap_or_default();
        if !is_key(key) {
            return Err(format!(
                "`{step}`: `{key}` is not a key (a lower-case letter, digit or punctuation \
                 character, `f1`–`f12`, or one of {})",
                NAMED_KEYS.join(", ")
            ));
        }
        let mut last = None;
        for modifier in parts {
            let Some(position) = MODIFIERS.iter().position(|m| *m == modifier) else {
                return Err(format!(
                    "`{step}`: `{modifier}` is not a modifier ({})",
                    MODIFIERS.join(", ")
                ));
            };
            if last.is_some_and(|last| position <= last) {
                return Err(format!(
                    "`{step}`: write modifiers once each, in the order {}",
                    MODIFIERS.join("+")
                ));
            }
            last = Some(position);
        }
    }
    Ok(())
}

fn is_key(key: &str) -> bool {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            c.is_ascii_lowercase() || c.is_ascii_digit() || (c.is_ascii_punctuation() && c != '+')
        }
        _ => {
            NAMED_KEYS.contains(&key)
                || key
                    .strip_prefix('f')
                    .and_then(|n| n.parse::<u8>().ok())
                    .is_some_and(|n| (1..=12).contains(&n) && !key.starts_with("f0"))
        }
    }
}

fn duplicates<'a>(what: &str, names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut reported = BTreeSet::new();
    names
        .filter(|name| !seen.insert(*name) && reported.insert(*name))
        .map(|name| format!("{what} `{name}` is declared twice"))
        .collect()
}

/// Component schemas by name, with who first declared each, refusing two
/// different schemas under one name (one would silently replace the other
/// in the OpenAPI document).
#[derive(Default)]
struct SchemaSet(BTreeMap<String, (RefOr<Schema>, String)>);

impl SchemaSet {
    fn add(&mut self, decl: &SchemaDecl, owner: &str, errors: &mut Vec<String>) {
        for (name, schema) in &decl.components {
            match self.0.get(name) {
                None => {
                    self.0
                        .insert(name.clone(), (schema.clone(), owner.to_owned()));
                }
                Some((existing, first)) => {
                    let same =
                        serde_json::to_value(existing).ok() == serde_json::to_value(schema).ok();
                    if !same {
                        errors.push(format!(
                            "{owner}: schema `{name}` differs from the `{name}` of {first}; \
                             give one of them another name (`#[schema(as = …)]`)"
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts() {
        for ok in [
            "e",
            "g i",
            "mod+enter",
            "shift+?",
            "mod+shift+k",
            "/",
            "f5",
            "g g g",
            "ctrl+alt+1",
        ] {
            assert_eq!(check_shortcut(ok), Ok(()), "{ok}");
        }
        for bad in [
            "",
            "E",
            "g  i",
            "a b c d",
            "shift+mod+k",
            "mod+mod+k",
            "cmd+k",
            "mod+",
            "+",
            "f13",
            "f0",
            "enterr",
            "é",
        ] {
            assert!(check_shortcut(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn duplicate_names_are_reported_once() {
        let v = duplicates("action", ["a.x", "a.y", "a.x", "a.x"].into_iter());
        assert_eq!(v, ["action `a.x` is declared twice"]);
    }
}
