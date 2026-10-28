//! Notion's property kinds and values in this module's, and back
//! (campbooks-cloud#377). The mapping is structural, kind to type; nothing
//! here reads what a value says to decide anything.
//!
//! - Both ways: title, rich text, number, select, multi-select, status,
//!   date, checkbox, URL, email and phone number.
//! - Notion's alone, shown and never written: formula and rollup (their last
//!   value, as text), people, created-by and last-edited-by (names only),
//!   and every other kind (`external`: the kind and, for the ones with a
//!   short value, that text).
//! - This module's alone, never sent: a Campbooks link, people of this
//!   workspace, files, relations, created, edited and ID.

use serde_json::{Value, json};

use super::model::{NOption, NProperty, NewOption};
use crate::properties::{
    PropertyType, config as property_config,
    config::SelectOption,
    value::{self, runs_text},
};

/// The most characters a text cell keeps.
const MAX_TEXT: usize = 2000;

/// A Notion property as a table property.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapped {
    /// Its type here.
    pub ty: PropertyType,
    /// Its config.
    pub config: Value,
    /// Whether values and the column go both ways.
    pub two_way: bool,
}

/// The id an option has here: Notion's own when it is short and plain,
/// otherwise derived from it (stable, never shown).
pub fn option_id(notion_id: &str) -> String {
    let plain = !notion_id.is_empty()
        && notion_id.len() <= 40
        && notion_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if plain {
        return notion_id.to_owned();
    }
    let hash = campbooks_kernel::blobs::BlobHash::of(notion_id.as_bytes()).to_string();
    format!("o{}", &hash[..20])
}

const CURRENCIES: [(&str, &str); 6] = [
    ("dollar", "USD"),
    ("euro", "EUR"),
    ("pound", "GBP"),
    ("yen", "JPY"),
    ("canadian_dollar", "CAD"),
    ("australian_dollar", "AUD"),
];

fn number_config(format: Option<&str>) -> Value {
    match format {
        Some("percent") => json!({"format": "percent"}),
        Some(other) => CURRENCIES
            .iter()
            .find(|(notion, _)| *notion == other)
            .map_or_else(
                || json!({"format": "number"}),
                |(_, code)| json!({"format": "currency", "currency": code}),
            ),
        None => json!({"format": "number"}),
    }
}

fn number_format(config: &Value) -> String {
    match config.get("format").and_then(Value::as_str) {
        Some("percent") => "percent".to_owned(),
        Some("currency") => {
            let code = config
                .get("currency")
                .and_then(Value::as_str)
                .unwrap_or_default();
            CURRENCIES
                .iter()
                .find(|(_, iso)| *iso == code)
                .map_or("number", |(notion, _)| *notion)
                .to_owned()
        }
        _ => "number".to_owned(),
    }
}

fn options_config(ty: PropertyType, options: &[NOption]) -> Option<Value> {
    let listed: Vec<Value> = options
        .iter()
        .map(|o| {
            let mut out = json!({"id": option_id(&o.id), "name": o.name, "color": o.color});
            if ty == PropertyType::Status {
                out["group"] = json!(o.group.clone().unwrap_or_else(|| "todo".into()));
            }
            out
        })
        .collect();
    property_config::normalise(ty, &json!({"options": listed})).ok()
}

/// The type, config and direction of Notion property `p`.
pub fn mapped(p: &NProperty) -> Mapped {
    let two = |ty: PropertyType, config: Value| Mapped {
        ty,
        config,
        two_way: true,
    };
    let reserved = |ty: PropertyType| Mapped {
        ty,
        config: json!({"kind": p.kind}),
        two_way: false,
    };
    match p.kind.as_str() {
        "title" => two(PropertyType::Title, json!({})),
        "rich_text" => two(PropertyType::Text, json!({})),
        "number" => two(PropertyType::Number, number_config(p.format.as_deref())),
        "select" | "multi_select" | "status" => {
            let ty = match p.kind.as_str() {
                "select" => PropertyType::Select,
                "multi_select" => PropertyType::MultiSelect,
                _ => PropertyType::Status,
            };
            match options_config(ty, &p.options) {
                Some(config) => two(ty, config),
                None => reserved(PropertyType::External),
            }
        }
        "date" => two(PropertyType::Date, json!({"time": true})),
        "checkbox" => two(PropertyType::Checkbox, json!({})),
        "url" => two(PropertyType::Url, json!({})),
        "email" => two(PropertyType::Email, json!({})),
        "phone_number" => two(PropertyType::Phone, json!({})),
        "formula" => reserved(PropertyType::NotionFormula),
        "rollup" => reserved(PropertyType::NotionRollup),
        "people" | "created_by" | "last_edited_by" => reserved(PropertyType::NotionPeople),
        _ => reserved(PropertyType::External),
    }
}

/// The Notion kind (and number format) a column of this type and config is
/// sent as, when it is sent at all.
pub fn notion_kind(ty: PropertyType, config: &Value) -> Option<(&'static str, Option<String>)> {
    Some(match ty {
        PropertyType::Title => ("title", None),
        PropertyType::Text => ("rich_text", None),
        PropertyType::Number => ("number", Some(number_format(config))),
        PropertyType::Select => ("select", None),
        PropertyType::MultiSelect => ("multi_select", None),
        PropertyType::Status => ("status", None),
        PropertyType::Date => ("date", None),
        PropertyType::Checkbox => ("checkbox", None),
        PropertyType::Url => ("url", None),
        PropertyType::Email => ("email", None),
        PropertyType::Phone => ("phone_number", None),
        _ => return None,
    })
}

/// The kinds a column can be made as at Notion by its API.
pub fn creatable(kind: &str) -> bool {
    matches!(
        kind,
        "rich_text"
            | "number"
            | "select"
            | "multi_select"
            | "date"
            | "checkbox"
            | "url"
            | "email"
            | "phone_number"
    )
}

/// The options of a select-like config, to add at Notion.
pub fn options_to_add(config: &Value) -> Vec<NewOption> {
    property_config::options_of(config)
        .into_iter()
        .map(|o: SelectOption| NewOption {
            name: o.name,
            color: o.color.unwrap_or_else(|| "default".into()),
            group: None,
        })
        .collect()
}

fn text_of(runs: &Value) -> String {
    runs.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn cut_runs(runs: &Value) -> Value {
    let Some(items) = runs.as_array() else {
        return Value::Null;
    };
    let mut left = MAX_TEXT;
    let mut out = Vec::new();
    for run in items {
        if left == 0 {
            break;
        }
        let mut run = run.clone();
        if let Some(text) = run.get("text").and_then(Value::as_str) {
            let kept: String = text.chars().take(left).collect();
            left -= kept.chars().count();
            run["text"] = Value::String(kept);
        }
        out.push(run);
    }
    Value::Array(out)
}

/// A title as the page's title: plain text, at most 200 characters.
pub fn title_of(neutral: &Value) -> String {
    crate::domain::clean_title(&text_of(neutral).chars().take(200).collect::<String>())
        .unwrap_or_default()
}

fn option_by(config: &Value, neutral: &Value) -> Option<String> {
    let options = property_config::options_of(config);
    let id = neutral.get("id").and_then(Value::as_str).map(option_id);
    let name = neutral.get("name").and_then(Value::as_str);
    id.and_then(|id| options.iter().find(|o| o.id == id).map(|o| o.id.clone()))
        .or_else(|| {
            name.and_then(|name| {
                options
                    .iter()
                    .find(|o| o.name.trim().eq_ignore_ascii_case(name.trim()))
                    .map(|o| o.id.clone())
            })
        })
}

/// The canonical value of a cell of type `ty` and `config` that Notion
/// holds as `neutral`: what is stored in `row_values` (the title is the
/// page's: a string). `null` when it holds nothing, or nothing that fits.
pub fn from_neutral(ty: PropertyType, config: &Value, neutral: &Value) -> Value {
    if neutral.is_null() {
        return if ty == PropertyType::Title {
            Value::String(String::new())
        } else {
            Value::Null
        };
    }
    match ty {
        PropertyType::Title => Value::String(title_of(neutral)),
        PropertyType::Text => value::validate(ty, config, &cut_runs(neutral))
            .or_else(|_| value::validate(ty, config, &Value::String(text_of(neutral))))
            .unwrap_or(Value::Null),
        PropertyType::Number
        | PropertyType::Checkbox
        | PropertyType::Url
        | PropertyType::Email
        | PropertyType::Phone => value::validate(ty, config, neutral).unwrap_or(Value::Null),
        PropertyType::Select | PropertyType::Status => {
            option_by(config, neutral).map_or(Value::Null, Value::String)
        }
        PropertyType::MultiSelect => {
            let mut ids: Vec<String> = Vec::new();
            for item in neutral.as_array().into_iter().flatten() {
                if let Some(id) = option_by(config, item)
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
            if ids.is_empty() {
                Value::Null
            } else {
                json!(ids)
            }
        }
        PropertyType::Date => value::validate(ty, config, neutral)
            .or_else(|_| {
                let mut plain = neutral.clone();
                if let Some(map) = plain.as_object_mut() {
                    map.remove("time_zone");
                }
                value::validate(ty, config, &plain)
            })
            .unwrap_or(Value::Null),
        // Read only: Notion's text or names as they are.
        PropertyType::NotionFormula
        | PropertyType::NotionRollup
        | PropertyType::NotionPeople
        | PropertyType::External => neutral.clone(),
        _ => Value::Null,
    }
}

/// The neutral value of a cell of type `ty` and `config` holding `stored`
/// (the page's title for a title): what the connector writes to Notion.
pub fn to_neutral(ty: PropertyType, config: &Value, stored: &Value) -> Value {
    match ty {
        PropertyType::Title => match stored.as_str().filter(|t| !t.is_empty()) {
            Some(title) => json!([{"text": title}]),
            None => Value::Null,
        },
        PropertyType::Text
        | PropertyType::Date
        | PropertyType::Url
        | PropertyType::Email
        | PropertyType::Phone => stored.clone(),
        PropertyType::Number => stored
            .as_str()
            .and_then(|text| {
                text.parse::<i64>().map(Value::from).ok().or_else(|| {
                    text.parse::<f64>()
                        .ok()
                        .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
                })
            })
            .unwrap_or(Value::Null),
        PropertyType::Select | PropertyType::Status => {
            let options = property_config::options_of(config);
            stored
                .as_str()
                .and_then(|id| options.iter().find(|o| o.id == id))
                .map_or(Value::Null, |o| json!({"id": o.id, "name": o.name}))
        }
        PropertyType::MultiSelect => {
            let options = property_config::options_of(config);
            let items: Vec<Value> = value::id_list(stored)
                .iter()
                .filter_map(|id| options.iter().find(|o| &o.id == id))
                .map(|o| json!({"id": o.id, "name": o.name}))
                .collect();
            if items.is_empty() {
                Value::Null
            } else {
                Value::Array(items)
            }
        }
        PropertyType::Checkbox => Value::Bool(stored.as_bool().unwrap_or(false)),
        _ => Value::Null,
    }
}

/// The plain text of a text value's runs.
pub fn plain(runs: &Value) -> String {
    runs_text(runs)
}
