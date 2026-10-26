//! What a column becomes when its property changes type.
//!
//! [`plan`] converts the values it can (through their text form, or option by
//! option) and answers `None` for those it cannot; the module clears those
//! cells and keeps what they held, so undoing the change restores every
//! cell as it was. A conversion never invents a value, and a value it makes
//! always passes [`value::validate`] for the new type.

use serde_json::{Value, json};

use super::{
    MAX_OPTIONS, PropertyError, PropertyType, Result, Spec,
    config::{self, SelectOption, StatusGroup, default_config, new_option_id, options_of},
    value::{self, When, parse_decimal, parse_when, plain_text},
};

/// A column converted: its new config, and for each cell its new value
/// (`Some(Value::Null)` for a cell that was empty) or `None` when it cannot
/// be converted.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan<K> {
    /// The config of the new type (options made from the values when the
    /// column became a select and had none).
    pub config: Value,
    /// Each cell and what it becomes.
    pub values: Vec<(K, Option<Value>)>,
}

/// Convert the `cells` of property `from` to type `to`, with `to_config` or
/// the type's default. Fails when either type cannot hold stored cells.
pub fn plan<K: Clone>(
    from: &Spec,
    to: PropertyType,
    to_config: Option<&Value>,
    cells: &[(K, Value)],
) -> Result<Plan<K>> {
    if !from.ty.editable() || !to.editable() || !to.creatable() {
        return Err(PropertyError::CONFIG);
    }
    let mut config = match to_config {
        Some(raw) => config::normalise(to, raw)?,
        None => default_config(to),
    };
    if is_select(to) && from.ty != to && options_of(&config).is_empty() {
        config = options_from_values(from, to, &config, cells)?;
    }
    let values = cells
        .iter()
        .map(|(key, cell)| {
            let converted = if cell.is_null() {
                Some(Value::Null)
            } else {
                convert_value(from, to, &config, cell)
            };
            (key.clone(), converted)
        })
        .collect();
    Ok(Plan { config, values })
}

const fn is_select(ty: PropertyType) -> bool {
    matches!(
        ty,
        PropertyType::Select | PropertyType::MultiSelect | PropertyType::Status
    )
}

/// A column converted to a select with no options gets one per distinct
/// text (ignoring case), in the order they first appear.
fn options_from_values<K>(
    from: &Spec,
    to: PropertyType,
    config: &Value,
    cells: &[(K, Value)],
) -> Result<Value> {
    let mut options: Vec<SelectOption> = Vec::new();
    for (_, cell) in cells {
        for name in names_of(from, cell) {
            let Ok(name) = super::clean_name(&name) else {
                continue;
            };
            if options.len() < MAX_OPTIONS
                && !options
                    .iter()
                    .any(|option| option.name.to_lowercase() == name.to_lowercase())
            {
                options.push(SelectOption {
                    id: new_option_id(),
                    name,
                    color: None,
                    group: (to == PropertyType::Status).then_some(StatusGroup::Todo),
                });
            }
        }
    }
    if options.is_empty() {
        return Ok(config.clone());
    }
    config::with_options(to, &options)
}

/// The names a value stands for when it becomes options.
fn names_of(from: &Spec, cell: &Value) -> Vec<String> {
    match from.ty {
        PropertyType::MultiSelect => options_of(&from.config)
            .into_iter()
            .filter(|option| value::id_list(cell).contains(&option.id))
            .map(|option| option.name)
            .collect(),
        PropertyType::Select | PropertyType::Status => {
            plain_text(from.ty, &from.config, cell).into_iter().collect()
        }
        PropertyType::Text | PropertyType::Url | PropertyType::Email | PropertyType::Phone => {
            plain_text(from.ty, &from.config, cell)
                .map(|text| text.split(',').map(|part| part.trim().to_owned()).collect())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn convert_value(from: &Spec, to: PropertyType, config: &Value, cell: &Value) -> Option<Value> {
    let checked = |candidate: Value| {
        value::validate(to, config, &candidate)
            .ok()
            .filter(|checked| !checked.is_null())
    };
    if from.ty == to {
        return match to {
            PropertyType::Date => date(config, cell),
            _ => checked(cell.clone()).or_else(|| select_by_name(from, to, config, cell)),
        };
    }
    let text = plain_text(from.ty, &from.config, cell);
    match to {
        PropertyType::Text => checked(Value::String(text?)),
        PropertyType::Number => checked(Value::String(parse_decimal(&text?)?)),
        PropertyType::Url | PropertyType::Email | PropertyType::Phone => {
            checked(Value::String(text?))
        }
        PropertyType::Select | PropertyType::Status => select_by_name(from, to, config, cell),
        PropertyType::MultiSelect => {
            let names = names_of(from, cell);
            let ids: Vec<Value> = names
                .iter()
                .filter_map(|name| option_named(config, name))
                .map(Value::String)
                .collect();
            checked(Value::Array(ids))
        }
        PropertyType::Date => match from.ty {
            PropertyType::Text | PropertyType::Url | PropertyType::Email | PropertyType::Phone => {
                date(config, &json!({"start": text?.trim()}))
            }
            _ => None,
        },
        PropertyType::Checkbox => match text?.as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

fn option_named(config: &Value, name: &str) -> Option<String> {
    options_of(config)
        .into_iter()
        .find(|option| option.name.to_lowercase() == name.to_lowercase())
        .map(|option| option.id)
}

/// One option, by id when the new property has it, else by name.
fn select_by_name(from: &Spec, to: PropertyType, config: &Value, cell: &Value) -> Option<Value> {
    let by_id = match from.ty {
        PropertyType::Select | PropertyType::Status => cell.as_str().map(ToOwned::to_owned),
        PropertyType::MultiSelect => value::id_list(cell).into_iter().next(),
        _ => None,
    }
    .filter(|id| options_of(config).iter().any(|option| &option.id == id));
    let id = by_id.or_else(|| {
        names_of(from, cell)
            .first()
            .and_then(|name| option_named(config, name))
    })?;
    value::validate(
        to,
        config,
        &if to == PropertyType::MultiSelect {
            json!([id])
        } else {
            Value::String(id)
        },
    )
    .ok()
}

/// A date for a column that does or does not hold times: a time is cut to
/// its day when the new config has none.
fn date(config: &Value, cell: &Value) -> Option<Value> {
    let start = cell.get("start").and_then(Value::as_str)?;
    let cut = |text: &str| -> Option<String> {
        let when = parse_when(text)?;
        Some(if config::date_has_time(config) {
            when.text()
        } else {
            When::Day(when.day()).text()
        })
    };
    let mut candidate = serde_json::Map::new();
    candidate.insert("start".into(), Value::String(cut(start)?));
    if let Some(end) = cell.get("end").and_then(Value::as_str) {
        candidate.insert("end".into(), Value::String(cut(end)?));
    }
    if config::date_has_time(config) {
        if let Some(zone) = cell.get("time_zone") {
            candidate.insert("time_zone".into(), zone.clone());
        }
    }
    value::validate(PropertyType::Date, config, &Value::Object(candidate)).ok()
}
