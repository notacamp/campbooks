//! What a query may ask: filters, sorts and summaries, checked against a
//! table's properties before any SQL is made.
//!
//! A filter is a tree of AND/OR groups (at most 3 levels, 50 conditions):
//!
//! ```json
//! {"and": [
//!   {"property": "<id>", "operator": "is_not", "value": "<option id>"},
//!   {"or": [ {"property": "<id>", "operator": "gt", "value": "100"} ]}
//! ]}
//! ```
//!
//! Each type has its operators; `is_not` and `does_not_contain` are the
//! complement of `is` and `contains`, so an empty cell passes them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    PropertyError, PropertyType, Result, Spec,
    config::shows_who,
    value::{When, parse_decimal, parse_when},
};

/// The deepest a filter's groups nest.
pub const MAX_DEPTH: usize = 3;
/// The most conditions a filter holds.
pub const MAX_CONDITIONS: usize = 50;
/// The most sorts a query has.
pub const MAX_SORTS: usize = 5;
/// The longest an operand may be.
const MAX_OPERAND: usize = 2_000;

/// How a condition compares a cell with its operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = FilterOperator)]
pub enum Operator {
    /// Equal (text ignoring case, a number, an option, a day, a checkbox).
    Is,
    /// Not equal; an empty cell passes.
    IsNot,
    /// One of several options.
    IsAnyOf,
    /// Holds the text, the option, the person, the row or the link.
    Contains,
    /// Does not; an empty cell passes.
    DoesNotContain,
    /// Text starts with.
    StartsWith,
    /// Text ends with.
    EndsWith,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Before the instant.
    Before,
    /// After the instant.
    After,
    /// On or before the instant.
    OnOrBefore,
    /// On or after the instant.
    OnOrAfter,
    /// Nothing there.
    IsEmpty,
    /// Something there.
    IsNotEmpty,
}

/// What a condition compares with, in the form its type needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// None (`is_empty`, `is_not_empty`).
    None,
    /// Text.
    Text(String),
    /// A canonical decimal.
    Number(String),
    /// A boolean.
    Bool(bool),
    /// An option id, a member id or a row id.
    Id(String),
    /// Several option ids.
    Ids(Vec<String>),
    /// A day or an instant.
    When(When),
    /// A Campbooks record.
    Link {
        /// Its record type.
        record: String,
        /// Its id.
        id: String,
    },
}

/// One comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// The property compared.
    pub property: Uuid,
    /// How.
    pub operator: Operator,
    /// With what.
    pub operand: Operand,
}

/// A filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// All (or any, for an OR group) of the items.
    Group {
        /// An OR group, not an AND group.
        any: bool,
        /// The items.
        items: Vec<Filter>,
    },
    /// One comparison.
    Condition(Condition),
}

impl Filter {
    /// Every property the filter names.
    pub fn properties(&self, out: &mut Vec<Uuid>) {
        match self {
            Self::Group { items, .. } => items.iter().for_each(|item| item.properties(out)),
            Self::Condition(condition) => out.push(condition.property),
        }
    }
}

/// The operators a property of this type and config accepts.
pub fn operators(spec: &Spec) -> &'static [Operator] {
    use Operator::{
        After, Before, Contains, DoesNotContain, EndsWith, Gt, Gte, Is, IsAnyOf, IsEmpty, IsNot,
        IsNotEmpty, Lt, Lte, OnOrAfter, OnOrBefore, StartsWith,
    };
    match spec.ty {
        PropertyType::Title | PropertyType::Text | PropertyType::Url | PropertyType::Email
        | PropertyType::Phone => &[
            Is,
            IsNot,
            Contains,
            DoesNotContain,
            StartsWith,
            EndsWith,
            IsEmpty,
            IsNotEmpty,
        ],
        PropertyType::Number | PropertyType::UniqueId => {
            &[Is, IsNot, Gt, Gte, Lt, Lte, IsEmpty, IsNotEmpty]
        }
        PropertyType::Select | PropertyType::Status => {
            &[Is, IsNot, IsAnyOf, IsEmpty, IsNotEmpty]
        }
        PropertyType::MultiSelect | PropertyType::Person | PropertyType::Relation => {
            &[Contains, DoesNotContain, IsEmpty, IsNotEmpty]
        }
        PropertyType::Date => &[
            Is, Before, After, OnOrBefore, OnOrAfter, IsEmpty, IsNotEmpty,
        ],
        PropertyType::Created | PropertyType::Edited if shows_who(&spec.config) => &[Is, IsNot],
        PropertyType::Created | PropertyType::Edited => {
            &[Is, Before, After, OnOrBefore, OnOrAfter]
        }
        PropertyType::Checkbox => &[Is],
        PropertyType::Files => &[IsEmpty, IsNotEmpty],
        PropertyType::CampbooksLink => &[Contains, DoesNotContain, IsEmpty, IsNotEmpty],
        _ => &[],
    }
}

fn operand_of(spec: &Spec, operator: Operator, raw: Option<&Value>) -> Result<Operand> {
    use Operator::{IsAnyOf, IsEmpty, IsNotEmpty};
    if matches!(operator, IsEmpty | IsNotEmpty) {
        return Ok(Operand::None);
    }
    let raw = raw.filter(|raw| !raw.is_null()).ok_or(PropertyError::FILTER)?;
    let text = || {
        raw.as_str()
            .filter(|text| !text.is_empty() && text.len() <= MAX_OPERAND)
            .map(ToOwned::to_owned)
            .ok_or(PropertyError::FILTER)
    };
    let id_token = |text: &str| {
        !text.is_empty()
            && text.len() <= 40
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    match spec.ty {
        PropertyType::Title | PropertyType::Text | PropertyType::Url | PropertyType::Email
        | PropertyType::Phone => Ok(Operand::Text(text()?)),
        PropertyType::Number | PropertyType::UniqueId => {
            let text = match raw {
                Value::Number(number) => number.to_string(),
                Value::String(text) => text.clone(),
                _ => return Err(PropertyError::FILTER),
            };
            parse_decimal(&text)
                .map(Operand::Number)
                .ok_or(PropertyError::FILTER)
        }
        PropertyType::Select | PropertyType::Status | PropertyType::MultiSelect => {
            if operator == IsAnyOf {
                let items = raw.as_array().ok_or(PropertyError::FILTER)?;
                let ids = items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .filter(|text| id_token(text))
                            .map(ToOwned::to_owned)
                            .ok_or(PropertyError::FILTER)
                    })
                    .collect::<Result<Vec<_>>>()?;
                if ids.is_empty() || ids.len() > super::MAX_OPTIONS {
                    return Err(PropertyError::FILTER);
                }
                Ok(Operand::Ids(ids))
            } else {
                let id = text()?;
                id_token(&id)
                    .then_some(Operand::Id(id))
                    .ok_or(PropertyError::FILTER)
            }
        }
        PropertyType::Person | PropertyType::Relation => {
            let id: Uuid = text()?.parse().map_err(|_| PropertyError::FILTER)?;
            Ok(Operand::Id(id.to_string()))
        }
        PropertyType::Created | PropertyType::Edited if shows_who(&spec.config) => {
            let id: Uuid = text()?.parse().map_err(|_| PropertyError::FILTER)?;
            Ok(Operand::Id(id.to_string()))
        }
        PropertyType::Date | PropertyType::Created | PropertyType::Edited => parse_when(&text()?)
            .map(Operand::When)
            .ok_or(PropertyError::FILTER),
        PropertyType::Checkbox => raw
            .as_bool()
            .map(Operand::Bool)
            .ok_or(PropertyError::FILTER),
        PropertyType::CampbooksLink => {
            let object = raw.as_object().ok_or(PropertyError::FILTER)?;
            let record = object
                .get("type")
                .and_then(Value::as_str)
                .filter(|record| super::config::is_record_type(record))
                .ok_or(PropertyError::FILTER)?;
            let id = object
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 320)
                .ok_or(PropertyError::FILTER)?;
            Ok(Operand::Link {
                record: record.to_owned(),
                id: id.to_owned(),
            })
        }
        _ => Err(PropertyError::FILTER),
    }
}

/// A filter as JSON, checked against `specs` (the table's properties).
pub fn parse(raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Filter> {
    let mut conditions = 0;
    walk(raw, specs, 1, &mut conditions, false)?.ok_or(PropertyError::FILTER)
}

/// A stored filter, read for a query: a condition on a property that was
/// removed, or whose type changed so that it no longer fits, is left out
/// instead of failing the query (restoring the property brings it back, the
/// stored view is untouched).
pub fn parse_lenient(raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Option<Filter>> {
    let mut conditions = 0;
    walk(raw, specs, 1, &mut conditions, true)
}

fn walk(
    raw: &Value,
    specs: &HashMap<Uuid, Spec>,
    depth: usize,
    conditions: &mut usize,
    lenient: bool,
) -> Result<Option<Filter>> {
    let object = raw.as_object().ok_or(PropertyError::FILTER)?;
    for (key, any) in [("and", false), ("or", true)] {
        if let Some(items) = object.get(key) {
            if object.len() != 1 || depth > MAX_DEPTH {
                return Err(PropertyError::FILTER);
            }
            let items = items.as_array().ok_or(PropertyError::FILTER)?;
            let mut kept = Vec::new();
            for item in items {
                if let Some(item) = walk(item, specs, depth + 1, conditions, lenient)? {
                    kept.push(item);
                }
            }
            return Ok(Some(Filter::Group { any, items: kept }));
        }
    }
    if object
        .keys()
        .any(|key| !["property", "operator", "value"].contains(&key.as_str()))
    {
        return Err(PropertyError::FILTER);
    }
    *conditions += 1;
    if *conditions > MAX_CONDITIONS {
        return Err(PropertyError::FILTER);
    }
    let condition = || -> Result<Condition> {
        let property: Uuid = object
            .get("property")
            .and_then(Value::as_str)
            .and_then(|text| text.parse().ok())
            .ok_or(PropertyError::FILTER)?;
        let spec = specs.get(&property).ok_or(PropertyError::FILTER)?;
        let operator: Operator = object
            .get("operator")
            .cloned()
            .and_then(|operator| serde_json::from_value(operator).ok())
            .ok_or(PropertyError::FILTER)?;
        if !operators(spec).contains(&operator) {
            return Err(PropertyError::FILTER);
        }
        let operand = operand_of(spec, operator, object.get("value"))?;
        Ok(Condition {
            property,
            operator,
            operand,
        })
    };
    match condition() {
        Ok(condition) => Ok(Some(Filter::Condition(condition))),
        Err(_) if lenient => Ok(None),
        Err(error) => Err(error),
    }
}

/// Which way a sort goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = SortDirection)]
pub enum Direction {
    /// Smallest first; empty cells last.
    Asc,
    /// Largest first; empty cells last.
    Desc,
}

/// One sort of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = TableSort)]
pub struct Sort {
    /// The property.
    pub property: Uuid,
    /// The direction.
    pub direction: Direction,
}

/// Sorts as JSON, checked: up to 5, each a sortable property, once.
pub fn parse_sorts(raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Vec<Sort>> {
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let sorts: Vec<Sort> = serde_json::from_value(raw.clone()).map_err(|_| PropertyError::SORT)?;
    if sorts.len() > MAX_SORTS {
        return Err(PropertyError::SORT);
    }
    let mut seen = std::collections::HashSet::new();
    for sort in &sorts {
        let spec = specs.get(&sort.property).ok_or(PropertyError::SORT)?;
        if !spec.ty.sortable() || !seen.insert(sort.property) {
            return Err(PropertyError::SORT);
        }
    }
    Ok(sorts)
}

/// What a column's footer shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = SummaryKind)]
pub enum SummaryKind {
    /// How many rows.
    Count,
    /// The sum of a number.
    Sum,
    /// The average of a number.
    Average,
    /// The smallest number or earliest date.
    Min,
    /// The largest number or latest date.
    Max,
    /// The share of rows that are checked, 0 to 100.
    PercentChecked,
}

/// One summary of a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = TableSummary)]
pub struct Summary {
    /// The property.
    pub property: Uuid,
    /// What to show.
    pub kind: SummaryKind,
}

/// Whether `kind` makes sense for a property of this type.
pub fn summary_allowed(ty: PropertyType, kind: SummaryKind) -> bool {
    match kind {
        SummaryKind::Count => true,
        SummaryKind::Sum | SummaryKind::Average => ty == PropertyType::Number,
        SummaryKind::Min | SummaryKind::Max => matches!(
            ty,
            PropertyType::Number
                | PropertyType::Date
                | PropertyType::Created
                | PropertyType::Edited
                | PropertyType::UniqueId
        ),
        SummaryKind::PercentChecked => ty == PropertyType::Checkbox,
    }
}

/// Summaries as JSON, checked: each for a property that can show it.
pub fn parse_summaries(raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Vec<Summary>> {
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let summaries: Vec<Summary> =
        serde_json::from_value(raw.clone()).map_err(|_| PropertyError::SUMMARY)?;
    if summaries.len() > super::MAX_PROPERTIES {
        return Err(PropertyError::SUMMARY);
    }
    for summary in &summaries {
        let spec = specs.get(&summary.property).ok_or(PropertyError::SUMMARY)?;
        if !summary_allowed(spec.ty, summary.kind) {
            return Err(PropertyError::SUMMARY);
        }
    }
    Ok(summaries)
}
