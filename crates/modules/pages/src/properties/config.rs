//! A property's config, per type: what it may say, and its canonical form.
//!
//! A config is a JSON object the module stores as given back by
//! [`normalise`]: unknown keys are refused, defaults are written out, option
//! ids are made when missing. Select, multi-select and status properties keep
//! their options by id, so renaming one changes every row that holds it and
//! no value is rewritten.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{MAX_OPTIONS, PropertyError, PropertyType, Result, clean_name};

/// The colours an option may have.
pub const COLORS: &[&str] = &[
    "default", "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

/// The longest a currency prefix, an ID prefix or an option id may be.
const MAX_PREFIX: usize = 10;
const MAX_OPTION_ID: usize = 40;
/// The most record types a Campbooks link may allow.
const MAX_LINK_TYPES: usize = 8;

/// Where a status option sits in the flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = StatusGroup)]
pub enum StatusGroup {
    /// Not started.
    Todo,
    /// In progress.
    Doing,
    /// Finished.
    Done,
}

/// One option of a select, multi-select or status property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = SelectOption)]
pub struct SelectOption {
    /// Its id: stable, what values hold. Made when missing.
    #[serde(default)]
    pub id: String,
    /// Its name, 1 to 100 characters, unique among the property's options.
    pub name: String,
    /// One of `default`, `gray`, `brown`, `orange`, `yellow`, `green`,
    /// `blue`, `purple`, `pink`, `red`.
    #[serde(default)]
    pub color: Option<String>,
    /// For a status: `todo`, `doing` or `done`. None for a select.
    #[serde(default)]
    pub group: Option<StatusGroup>,
}

/// A new option id: short, and never shown.
pub fn new_option_id() -> String {
    let simple = Uuid::now_v7().simple().to_string();
    format!("o{}", &simple[20..])
}

/// The config a new property of type `ty` starts with.
pub fn default_config(ty: PropertyType) -> Value {
    match ty {
        PropertyType::Number => json!({"format": "number"}),
        PropertyType::Select | PropertyType::MultiSelect | PropertyType::Status => {
            json!({"options": []})
        }
        PropertyType::Date => json!({"time": false}),
        PropertyType::CampbooksLink => json!({"types": []}),
        PropertyType::Created | PropertyType::Edited => json!({"of": "time"}),
        PropertyType::UniqueId => json!({"prefix": ""}),
        _ => json!({}),
    }
}

fn object_with<'a>(raw: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>> {
    static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
    let object = match raw {
        Value::Null => &*EMPTY,
        Value::Object(object) => object,
        _ => return Err(PropertyError::CONFIG),
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(PropertyError::CONFIG);
    }
    Ok(object)
}

fn ascii_token(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.len() <= max
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `raw` as the canonical config of a property of type `ty`, or why not.
pub fn normalise(ty: PropertyType, raw: &Value) -> Result<Value> {
    match ty {
        PropertyType::Number => {
            let object = object_with(raw, &["format", "currency"])?;
            let format = object
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or("number");
            let currency = object.get("currency").filter(|c| !c.is_null());
            match (format, currency) {
                ("number" | "percent", None) => Ok(json!({"format": format})),
                ("currency", Some(Value::String(code)))
                    if code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase()) =>
                {
                    Ok(json!({"format": "currency", "currency": code}))
                }
                _ => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::Select | PropertyType::MultiSelect | PropertyType::Status => {
            let object = object_with(raw, &["options"])?;
            let options = match object.get("options") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(items)) => items.clone(),
                Some(_) => return Err(PropertyError::CONFIG),
            };
            let options = normalise_options(ty, &options)?;
            Ok(json!({"options": options}))
        }
        PropertyType::Date => {
            let object = object_with(raw, &["time"])?;
            match object.get("time") {
                None | Some(Value::Null) => Ok(json!({"time": false})),
                Some(Value::Bool(time)) => Ok(json!({"time": time})),
                Some(_) => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::Relation => {
            let object = object_with(raw, &["table", "pair"])?;
            let table = object
                .get("table")
                .and_then(Value::as_str)
                .and_then(|text| text.parse::<Uuid>().ok())
                .ok_or(PropertyError::CONFIG)?;
            match object.get("pair") {
                None | Some(Value::Null) => Ok(json!({"table": table.to_string()})),
                Some(Value::String(pair)) => {
                    let pair = pair.parse::<Uuid>().map_err(|_| PropertyError::CONFIG)?;
                    Ok(json!({"table": table.to_string(), "pair": pair.to_string()}))
                }
                Some(_) => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::CampbooksLink => {
            let object = object_with(raw, &["types"])?;
            let types = match object.get("types") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(items)) => items.clone(),
                Some(_) => return Err(PropertyError::CONFIG),
            };
            if types.len() > MAX_LINK_TYPES {
                return Err(PropertyError::CONFIG);
            }
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for item in &types {
                let name = item.as_str().ok_or(PropertyError::CONFIG)?;
                if !is_record_type(name) || !seen.insert(name.to_owned()) {
                    return Err(PropertyError::CONFIG);
                }
                out.push(name.to_owned());
            }
            Ok(json!({"types": out}))
        }
        PropertyType::Created | PropertyType::Edited => {
            let object = object_with(raw, &["of"])?;
            match object.get("of").and_then(Value::as_str) {
                None | Some("time") => Ok(json!({"of": "time"})),
                Some("who") => Ok(json!({"of": "who"})),
                Some(_) => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::UniqueId => {
            let object = object_with(raw, &["prefix"])?;
            match object.get("prefix") {
                None | Some(Value::Null) => Ok(json!({"prefix": ""})),
                Some(Value::String(prefix))
                    if prefix.is_empty() || ascii_token(prefix, MAX_PREFIX) =>
                {
                    Ok(json!({"prefix": prefix}))
                }
                Some(_) => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::NotionFormula
        | PropertyType::NotionRollup
        | PropertyType::NotionPeople
        | PropertyType::External => {
            let object = object_with(raw, &["kind"])?;
            match object.get("kind") {
                None | Some(Value::Null) => Ok(json!({})),
                Some(Value::String(kind)) if ascii_token(kind, 64) => Ok(json!({"kind": kind})),
                Some(_) => Err(PropertyError::CONFIG),
            }
        }
        PropertyType::Title
        | PropertyType::Text
        | PropertyType::Checkbox
        | PropertyType::Person
        | PropertyType::Files
        | PropertyType::Url
        | PropertyType::Email
        | PropertyType::Phone => {
            object_with(raw, &[])?;
            Ok(json!({}))
        }
    }
}

/// `module.record`, lower case with underscores: a record type of the
/// access catalogue.
pub(super) fn is_record_type(text: &str) -> bool {
    let Some((module, record)) = text.split_once('.') else {
        return false;
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 32
            && p.starts_with(|c: char| c.is_ascii_lowercase())
            && p.chars().all(|c| c.is_ascii_lowercase() || c == '_')
    };
    part(module) && part(record)
}

fn normalise_options(ty: PropertyType, items: &[Value]) -> Result<Vec<Value>> {
    if items.len() > MAX_OPTIONS {
        return Err(PropertyError::CONFIG);
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let object = item.as_object().ok_or(PropertyError::CONFIG)?;
        if object
            .keys()
            .any(|key| !["id", "name", "color", "group"].contains(&key.as_str()))
        {
            return Err(PropertyError::CONFIG);
        }
        let id = match object.get("id") {
            None | Some(Value::Null) => new_option_id(),
            Some(Value::String(id)) if id.is_empty() => new_option_id(),
            Some(Value::String(id)) if ascii_token(id, MAX_OPTION_ID) => id.clone(),
            Some(_) => return Err(PropertyError::CONFIG),
        };
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or(PropertyError::CONFIG)
            .and_then(|name| clean_name(name).map_err(|_| PropertyError::CONFIG))?;
        let color = match object.get("color") {
            None | Some(Value::Null) => "default",
            Some(Value::String(color)) if COLORS.contains(&color.as_str()) => color.as_str(),
            Some(_) => return Err(PropertyError::CONFIG),
        };
        if !ids.insert(id.clone()) || !names.insert(name.to_lowercase()) {
            return Err(PropertyError::CONFIG);
        }
        let mut option = json!({"id": id, "name": name, "color": color});
        match (ty, object.get("group")) {
            (PropertyType::Status, None | Some(Value::Null)) => {
                option["group"] = json!("todo");
            }
            (PropertyType::Status, Some(Value::String(group)))
                if ["todo", "doing", "done"].contains(&group.as_str()) =>
            {
                option["group"] = json!(group);
            }
            (_, None | Some(Value::Null)) => {}
            _ => return Err(PropertyError::CONFIG),
        }
        out.push(option);
    }
    Ok(out)
}

/// The options of a select, multi-select or status config, in order.
pub fn options_of(config: &Value) -> Vec<SelectOption> {
    config
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The config of type `ty` with `options` in place of its own, checked like
/// any other.
pub fn with_options(ty: PropertyType, options: &[SelectOption]) -> Result<Value> {
    if !matches!(
        ty,
        PropertyType::Select | PropertyType::MultiSelect | PropertyType::Status
    ) {
        return Err(PropertyError::CONFIG);
    }
    let items = options
        .iter()
        .map(|option| serde_json::to_value(option).map_err(|_| PropertyError::CONFIG))
        .collect::<Result<Vec<_>>>()?;
    normalise(ty, &json!({"options": items}))
}

/// Whether a date property of this config holds times.
pub fn date_has_time(config: &Value) -> bool {
    config.get("time").and_then(Value::as_bool).unwrap_or(false)
}

/// The prefix of an ID property.
pub fn id_prefix(config: &Value) -> &str {
    config.get("prefix").and_then(Value::as_str).unwrap_or("")
}

/// Whether a created or edited property shows who, not when.
pub fn shows_who(config: &Value) -> bool {
    config.get("of").and_then(Value::as_str) == Some("who")
}
