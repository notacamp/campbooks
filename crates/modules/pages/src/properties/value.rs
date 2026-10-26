//! A cell's value, per type: what may be stored and its canonical form.
//!
//! [`validate`] is the one gate: a value that does not fit its property's
//! type and config is refused (`invalid_value`) and nothing is stored; one
//! that does comes back in canonical form, so equal values are stored equal
//! and compared as text. Nothing here reads a value to decide anything but
//! whether it fits.
//!
//! | Type | Stored as |
//! | --- | --- |
//! | `text` | runs `[{text, bold?, italic?, strike?, code?, href?}]` |
//! | `number` | a decimal string (`-12.5`) |
//! | `select`, `status` | an option id |
//! | `multi_select` | option ids |
//! | `date` | `{start, end?, time_zone?}`: `YYYY-MM-DD` or RFC 3339 |
//! | `checkbox` | a boolean |
//! | `person`, `relation` | ids |
//! | `files` | `[{hash, name, size, type}]` |
//! | `url`, `email`, `phone` | a string |
//! | `campbooks_link` | `[{type, id}]` |
//!
//! `null` clears a cell.

use std::collections::HashSet;

use chrono::{DateTime, FixedOffset, NaiveDate, SecondsFormat};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{
    MAX_LIST, MAX_TEXT, MAX_VALUE_BYTES, PropertyError, PropertyType, Result,
    config::{date_has_time, options_of},
};

/// The most runs a text value holds.
const MAX_RUNS: usize = 200;
/// The longest a link may be, and an email address.
const MAX_URL: usize = 2_000;
const MAX_EMAIL: usize = 254;
/// The most files one cell holds, and the largest a file may be (the upload
/// limit of a page).
const MAX_FILES: usize = 20;
const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
/// The longest an id of a Campbooks record may be (`pages.links.ref_id`).
const MAX_REF_ID: usize = 320;

/// A date value read back: a day, or an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum When {
    /// A day (`YYYY-MM-DD`), compared as midnight UTC.
    Day(NaiveDate),
    /// An instant.
    At(DateTime<FixedOffset>),
}

impl When {
    /// The instant it stands for, in UTC.
    pub fn instant(self) -> DateTime<chrono::Utc> {
        match self {
            Self::Day(day) => day
                .and_hms_opt(0, 0, 0)
                .map_or(DateTime::UNIX_EPOCH, |naive| naive.and_utc()),
            Self::At(at) => at.with_timezone(&chrono::Utc),
        }
    }

    /// The day in UTC.
    pub fn day(self) -> NaiveDate {
        self.instant().date_naive()
    }

    /// Its canonical text.
    pub fn text(self) -> String {
        match self {
            Self::Day(day) => day.format("%Y-%m-%d").to_string(),
            Self::At(at) => at.to_rfc3339_opts(SecondsFormat::Secs, false),
        }
    }
}

/// A date or a datetime as text, or none if it is neither.
pub fn parse_when(text: &str) -> Option<When> {
    let bytes = text.as_bytes();
    if bytes.len() == 10 {
        let ok = bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
        if !ok {
            return None;
        }
        return NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .map(When::Day);
    }
    if text.len() > 40 || !text.contains('T') {
        return None;
    }
    DateTime::parse_from_rfc3339(text).ok().map(When::At)
}

/// `text` as a canonical decimal (`-12.5`): at most 15 digits before the
/// point and 9 after, no sign but `-`, no exponent.
pub fn parse_decimal(text: &str) -> Option<String> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits, None),
    };
    let all_digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) || fraction.is_some_and(|f| !all_digits(f)) {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.unwrap_or("").trim_end_matches('0');
    if whole.len() > 15 || fraction.len() > 9 {
        return None;
    }
    let zero = whole == "0" && fraction.is_empty();
    let mut out = String::new();
    if negative && !zero {
        out.push('-');
    }
    out.push_str(whole);
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(fraction);
    }
    Some(out)
}

fn clean_text(raw: &str) -> String {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    spaced
        .chars()
        .filter(|c| campbooks_kernel::text::is_displayable(*c))
        .collect()
}

fn safe_link(href: &str, schemes: &[&str]) -> bool {
    let lower = href.to_ascii_lowercase();
    href.len() <= MAX_URL
        && !href.chars().any(|c| c.is_whitespace() || c.is_control())
        && schemes.iter().any(|scheme| {
            lower.strip_prefix(scheme).is_some_and(|rest| {
                if *scheme == "mailto:" {
                    !rest.is_empty()
                } else {
                    rest.strip_prefix("//")
                        .is_some_and(|host| !host.is_empty() && !host.starts_with('/'))
                }
            })
        })
}

/// `raw` as the stored value of a cell of type `ty` with `config`, or why
/// not. `null` (or an empty list, an empty string) clears the cell.
pub fn validate(ty: PropertyType, config: &Value, raw: &Value) -> Result<Value> {
    if !ty.editable() {
        return Err(PropertyError::NOT_EDITABLE);
    }
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let value = match ty {
        PropertyType::Text => text(raw)?,
        PropertyType::Number => number(raw)?,
        PropertyType::Select | PropertyType::Status => select(config, raw)?,
        PropertyType::MultiSelect => multi_select(config, raw)?,
        PropertyType::Date => date(config, raw)?,
        PropertyType::Checkbox => match raw {
            Value::Bool(_) => raw.clone(),
            _ => return Err(PropertyError::VALUE),
        },
        PropertyType::Person | PropertyType::Relation => ids(raw)?,
        PropertyType::Files => files(raw)?,
        PropertyType::Url => string_value(raw, |text| {
            safe_link(text, &["http:", "https:"]).then(|| text.to_owned())
        })?,
        PropertyType::Email => string_value(raw, email)?,
        PropertyType::Phone => string_value(raw, phone)?,
        PropertyType::CampbooksLink => links(config, raw)?,
        _ => return Err(PropertyError::NOT_EDITABLE),
    };
    if serde_json::to_vec(&value).map_or(true, |bytes| bytes.len() > MAX_VALUE_BYTES) {
        return Err(PropertyError::VALUE);
    }
    Ok(value)
}

fn string_value(raw: &Value, check: impl Fn(&str) -> Option<String>) -> Result<Value> {
    let text = raw.as_str().ok_or(PropertyError::VALUE)?.trim();
    if text.is_empty() {
        return Ok(Value::Null);
    }
    check(text).map(Value::String).ok_or(PropertyError::VALUE)
}

fn email(text: &str) -> Option<String> {
    let (local, domain) = text.split_once('@')?;
    let plain = |part: &str| {
        !part
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '@' || c == ',' || c == '<')
    };
    let ok = text.len() <= MAX_EMAIL
        && (1..=64).contains(&local.len())
        && plain(local)
        && plain(domain)
        && domain.contains('.')
        && !domain.starts_with(['.', '-'])
        && !domain.ends_with(['.', '-'])
        && !domain.contains("..");
    ok.then(|| text.to_owned())
}

fn phone(text: &str) -> Option<String> {
    let digits = text.chars().filter(char::is_ascii_digit).count();
    let ok = (3..=32).contains(&text.len())
        && digits >= 3
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '(' | ')' | '-' | '.' | ' '));
    ok.then(|| text.to_owned())
}

fn text(raw: &Value) -> Result<Value> {
    let runs: Vec<Value> = match raw {
        Value::String(text) => vec![json!({"text": text})],
        Value::Array(runs) => runs.clone(),
        _ => return Err(PropertyError::VALUE),
    };
    if runs.len() > MAX_RUNS {
        return Err(PropertyError::VALUE);
    }
    let mut out = Vec::new();
    let mut length = 0;
    for run in &runs {
        let object = run.as_object().ok_or(PropertyError::VALUE)?;
        if object
            .keys()
            .any(|key| !["text", "bold", "italic", "strike", "code", "href"].contains(&key.as_str()))
        {
            return Err(PropertyError::VALUE);
        }
        let content = clean_text(
            object
                .get("text")
                .and_then(Value::as_str)
                .ok_or(PropertyError::VALUE)?,
        );
        length += content.chars().count();
        if length > MAX_TEXT {
            return Err(PropertyError::VALUE);
        }
        if content.is_empty() {
            continue;
        }
        let mut canonical = Map::new();
        canonical.insert("text".into(), Value::String(content));
        for mark in ["bold", "italic", "strike", "code"] {
            match object.get(mark) {
                None | Some(Value::Null | Value::Bool(false)) => {}
                Some(Value::Bool(true)) => {
                    canonical.insert(mark.into(), Value::Bool(true));
                }
                Some(_) => return Err(PropertyError::VALUE),
            }
        }
        match object.get("href") {
            None | Some(Value::Null) => {}
            Some(Value::String(href)) if safe_link(href, &["https:", "http:", "mailto:"]) => {
                canonical.insert("href".into(), Value::String(href.clone()));
            }
            Some(_) => return Err(PropertyError::VALUE),
        }
        out.push(Value::Object(canonical));
    }
    Ok(if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    })
}

/// The plain text of a text value's runs.
pub fn runs_text(value: &Value) -> String {
    value
        .as_array()
        .map(|runs| {
            runs.iter()
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default()
}

fn number(raw: &Value) -> Result<Value> {
    let text = match raw {
        Value::Number(number) => {
            if let Some(whole) = number.as_i64() {
                whole.to_string()
            } else if let Some(whole) = number.as_u64() {
                whole.to_string()
            } else {
                format!("{:.9}", number.as_f64().ok_or(PropertyError::VALUE)?)
            }
        }
        Value::String(text) if text.trim().is_empty() => return Ok(Value::Null),
        Value::String(text) => text.clone(),
        _ => return Err(PropertyError::VALUE),
    };
    parse_decimal(&text)
        .map(Value::String)
        .ok_or(PropertyError::VALUE)
}

fn select(config: &Value, raw: &Value) -> Result<Value> {
    let id = raw.as_str().ok_or(PropertyError::VALUE)?;
    if options_of(config).iter().any(|option| option.id == id) {
        Ok(Value::String(id.to_owned()))
    } else {
        Err(PropertyError::VALUE)
    }
}

fn multi_select(config: &Value, raw: &Value) -> Result<Value> {
    let items = raw.as_array().ok_or(PropertyError::VALUE)?;
    let options = options_of(config);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let id = item.as_str().ok_or(PropertyError::VALUE)?;
        if !options.iter().any(|option| option.id == id) || !seen.insert(id) {
            return Err(PropertyError::VALUE);
        }
        out.push(Value::String(id.to_owned()));
    }
    Ok(if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    })
}

fn date(config: &Value, raw: &Value) -> Result<Value> {
    let object = raw.as_object().ok_or(PropertyError::VALUE)?;
    if object
        .keys()
        .any(|key| !["start", "end", "time_zone"].contains(&key.as_str()))
    {
        return Err(PropertyError::VALUE);
    }
    let start = object
        .get("start")
        .and_then(Value::as_str)
        .and_then(parse_when)
        .ok_or(PropertyError::VALUE)?;
    let end = match object.get("end") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(parse_when(text).ok_or(PropertyError::VALUE)?),
        Some(_) => return Err(PropertyError::VALUE),
    };
    let has_time = matches!(start, When::At(_));
    if has_time && !date_has_time(config) {
        return Err(PropertyError::VALUE);
    }
    if let Some(end) = end {
        if matches!(end, When::At(_)) != has_time || end.instant() < start.instant() {
            return Err(PropertyError::VALUE);
        }
    }
    let mut out = Map::new();
    out.insert("start".into(), Value::String(start.text()));
    if let Some(end) = end {
        out.insert("end".into(), Value::String(end.text()));
    }
    match object.get("time_zone") {
        None | Some(Value::Null) => {}
        Some(Value::String(zone))
            if has_time
                && !zone.is_empty()
                && zone.len() <= 64
                && zone
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '+' | '-')) =>
        {
            out.insert("time_zone".into(), Value::String(zone.clone()));
        }
        Some(_) => return Err(PropertyError::VALUE),
    }
    Ok(Value::Object(out))
}

fn ids(raw: &Value) -> Result<Value> {
    let items = raw.as_array().ok_or(PropertyError::VALUE)?;
    if items.len() > MAX_LIST {
        return Err(PropertyError::VALUE);
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let id: Uuid = item
            .as_str()
            .and_then(|text| text.parse().ok())
            .ok_or(PropertyError::VALUE)?;
        if seen.insert(id) {
            out.push(Value::String(id.to_string()));
        }
    }
    Ok(if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    })
}

fn files(raw: &Value) -> Result<Value> {
    let items = raw.as_array().ok_or(PropertyError::VALUE)?;
    if items.len() > MAX_FILES {
        return Err(PropertyError::VALUE);
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let object = item.as_object().ok_or(PropertyError::VALUE)?;
        if object
            .keys()
            .any(|key| !["hash", "name", "size", "type"].contains(&key.as_str()))
        {
            return Err(PropertyError::VALUE);
        }
        let hash = object
            .get("hash")
            .and_then(Value::as_str)
            .filter(|hash| {
                hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            })
            .ok_or(PropertyError::VALUE)?;
        let name = clean_text(
            object
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PropertyError::VALUE)?,
        );
        let name = name.trim();
        let size = object
            .get("size")
            .and_then(Value::as_u64)
            .filter(|size| *size <= MAX_FILE_BYTES)
            .ok_or(PropertyError::VALUE)?;
        let media = object
            .get("type")
            .and_then(Value::as_str)
            .filter(|media| {
                (3..=100).contains(&media.len())
                    && media.contains('/')
                    && media
                        .chars()
                        .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\'))
            })
            .ok_or(PropertyError::VALUE)?;
        if name.is_empty() || name.chars().count() > 200 {
            return Err(PropertyError::VALUE);
        }
        if seen.insert(hash.to_owned()) {
            out.push(json!({"hash": hash, "name": name, "size": size, "type": media}));
        }
    }
    Ok(if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    })
}

fn links(config: &Value, raw: &Value) -> Result<Value> {
    let items = raw.as_array().ok_or(PropertyError::VALUE)?;
    if items.len() > MAX_LIST {
        return Err(PropertyError::VALUE);
    }
    let allowed: Vec<&str> = config
        .get("types")
        .and_then(Value::as_array)
        .map(|types| types.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let object = item.as_object().ok_or(PropertyError::VALUE)?;
        if object
            .keys()
            .any(|key| !["type", "id"].contains(&key.as_str()))
        {
            return Err(PropertyError::VALUE);
        }
        let record = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(PropertyError::VALUE)?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= MAX_REF_ID
                    && !id
                        .chars()
                        .any(|c| c.is_whitespace() || !campbooks_kernel::text::is_displayable(c))
            })
            .ok_or(PropertyError::VALUE)?;
        let known = super::config::is_record_type(record);
        if !known || (!allowed.is_empty() && !allowed.contains(&record)) {
            return Err(PropertyError::VALUE);
        }
        if seen.insert((record.to_owned(), id.to_owned())) {
            out.push(json!({"type": record, "id": id}));
        }
    }
    Ok(if out.is_empty() {
        Value::Null
    } else {
        Value::Array(out)
    })
}

/// Whether a stored value is empty: nothing, `null`, or an empty list.
pub fn is_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => items.is_empty(),
        Some(Value::String(text)) => text.is_empty(),
        Some(_) => false,
    }
}

/// The ids a person, relation or select-like value holds.
pub fn id_list(value: &Value) -> Vec<String> {
    match value {
        Value::String(id) => vec![id.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(ToOwned::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// A value as plain text, for conversion between types and for search: the
/// text of runs, the name of an option, a number, a date, an address.
/// `None` for the types that have no text form (people, files, relations).
pub fn plain_text(ty: PropertyType, config: &Value, value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let name_of = |id: &str| {
        options_of(config)
            .into_iter()
            .find(|option| option.id == id)
            .map(|option| option.name)
    };
    match ty {
        PropertyType::Text => Some(runs_text(value)).filter(|text| !text.is_empty()),
        PropertyType::Number | PropertyType::Url | PropertyType::Email | PropertyType::Phone => {
            value.as_str().map(ToOwned::to_owned)
        }
        PropertyType::Select | PropertyType::Status => value.as_str().and_then(name_of),
        PropertyType::MultiSelect => {
            let names: Vec<String> = id_list(value)
                .iter()
                .filter_map(|id| name_of(id))
                .collect();
            Some(names.join(", ")).filter(|text| !text.is_empty())
        }
        PropertyType::Date => value
            .get("start")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        PropertyType::Checkbox => value.as_bool().map(|on| on.to_string()),
        _ => None,
    }
}
