//! The same questions as [`super::sql`], answered in memory.
//!
//! The module never needs this to serve a query (rows are filtered and
//! sorted in the database); it is the reference the tests hold the SQL to:
//! random rows are filtered, ordered and summed both ways and must agree. It
//! also pins what each operator means.

use std::{cmp::Ordering, collections::HashMap};

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{
    PropertyType, Spec,
    config::{options_of, shows_who},
    filter::{Condition, Direction, Filter, Operand, Operator, SummaryKind},
    value::{id_list, parse_decimal, parse_when, runs_text},
};

/// A row as a query sees it: the page's columns and its stored cells.
#[derive(Debug, Clone)]
pub struct RowFacts {
    /// The row's title.
    pub title: String,
    /// Its number in the table.
    pub number: i64,
    /// When it was made.
    pub created_at: DateTime<Utc>,
    /// Who made it.
    pub created_by: Uuid,
    /// When it was last edited.
    pub edited_at: DateTime<Utc>,
    /// Who edited it last.
    pub edited_by: Uuid,
    /// Its stored cells, by property.
    pub cells: HashMap<Uuid, Value>,
}

#[derive(Debug, Clone, PartialEq)]
enum Datum {
    Empty,
    Text(String),
    Number(i128),
    Instant(DateTime<Utc>),
    Bool(bool),
    Id(String),
    Ids(Vec<String>),
    Links(Vec<(String, String)>),
    Who(Uuid),
}

/// A decimal in units of 10^-9.
pub fn scaled(text: &str) -> Option<i128> {
    let canonical = parse_decimal(text)?;
    let (negative, digits) = match canonical.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, canonical.as_str()),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let fraction = format!("{fraction:0<9}");
    let magnitude: i128 = format!("{whole}{fraction}").parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

fn datum(spec: &Spec, row: &RowFacts) -> Datum {
    let cell = row.cells.get(&spec.id).filter(|value| !value.is_null());
    match spec.ty {
        PropertyType::Title => Datum::Text(row.title.clone()),
        PropertyType::UniqueId => Datum::Number(i128::from(row.number) * 1_000_000_000),
        PropertyType::Created if shows_who(&spec.config) => Datum::Who(row.created_by),
        PropertyType::Edited if shows_who(&spec.config) => Datum::Who(row.edited_by),
        PropertyType::Created => Datum::Instant(row.created_at),
        PropertyType::Edited => Datum::Instant(row.edited_at),
        PropertyType::Checkbox => Datum::Bool(cell.and_then(Value::as_bool).unwrap_or(false)),
        _ => {
            let Some(value) = cell else {
                return Datum::Empty;
            };
            match spec.ty {
                PropertyType::Text => Datum::Text(runs_text(value)),
                PropertyType::Url | PropertyType::Email | PropertyType::Phone => {
                    value.as_str().map_or(Datum::Empty, |t| Datum::Text(t.into()))
                }
                PropertyType::Number => value
                    .as_str()
                    .and_then(scaled)
                    .map_or(Datum::Empty, Datum::Number),
                PropertyType::Select | PropertyType::Status => value
                    .as_str()
                    .map_or(Datum::Empty, |id| Datum::Id(id.to_owned())),
                PropertyType::Date => value
                    .get("start")
                    .and_then(Value::as_str)
                    .and_then(parse_when)
                    .map_or(Datum::Empty, |when| Datum::Instant(when.instant())),
                PropertyType::CampbooksLink => Datum::Links(
                    value
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    Some((
                                        item.get("type")?.as_str()?.to_owned(),
                                        item.get("id")?.as_str()?.to_owned(),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                ),
                _ => Datum::Ids(id_list(value)),
            }
        }
    }
}

fn empty(datum: &Datum) -> bool {
    match datum {
        Datum::Empty => true,
        Datum::Text(text) => text.is_empty(),
        Datum::Ids(ids) => ids.is_empty(),
        Datum::Links(links) => links.is_empty(),
        _ => false,
    }
}

/// Whether `row` passes `filter`.
pub fn matches(filter: &Filter, specs: &HashMap<Uuid, Spec>, row: &RowFacts) -> bool {
    match filter {
        Filter::Group { any, items } => {
            if *any {
                items.iter().any(|item| matches(item, specs, row))
            } else {
                items.iter().all(|item| matches(item, specs, row))
            }
        }
        Filter::Condition(condition) => specs
            .get(&condition.property)
            .is_some_and(|spec| holds(spec, condition, row)),
    }
}

fn holds(spec: &Spec, condition: &Condition, row: &RowFacts) -> bool {
    let datum = datum(spec, row);
    let is_empty = empty(&datum);
    let positive = match (&datum, condition.operator, &condition.operand) {
        (_, Operator::IsEmpty, _) => return is_empty,
        (_, Operator::IsNotEmpty, _) => return !is_empty,
        (Datum::Text(cell), op, Operand::Text(operand)) => {
            let (cell, operand) = (cell.to_lowercase(), operand.to_lowercase());
            match op {
                Operator::Is | Operator::IsNot => cell == operand,
                Operator::Contains | Operator::DoesNotContain => cell.contains(&operand),
                Operator::StartsWith => cell.starts_with(&operand),
                Operator::EndsWith => cell.ends_with(&operand),
                _ => false,
            }
        }
        (Datum::Number(cell), op, Operand::Number(operand)) => {
            let Some(operand) = scaled(operand) else {
                return false;
            };
            match op {
                Operator::Is | Operator::IsNot => *cell == operand,
                Operator::Gt => *cell > operand,
                Operator::Gte => *cell >= operand,
                Operator::Lt => *cell < operand,
                Operator::Lte => *cell <= operand,
                _ => false,
            }
        }
        (Datum::Id(cell), Operator::Is | Operator::IsNot, Operand::Id(operand)) => cell == operand,
        (Datum::Id(cell), Operator::IsAnyOf, Operand::Ids(operands)) => operands.contains(cell),
        (Datum::Ids(cell), Operator::Contains | Operator::DoesNotContain, Operand::Id(id)) => {
            cell.contains(id)
        }
        (
            Datum::Links(cell),
            Operator::Contains | Operator::DoesNotContain,
            Operand::Link { record, id },
        ) => cell.iter().any(|(r, i)| r == record && i == id),
        (Datum::Instant(cell), op, Operand::When(when)) => match op {
            Operator::Is => cell.date_naive() == when.day(),
            Operator::Before => *cell < when.instant(),
            Operator::After => *cell > when.instant(),
            Operator::OnOrBefore => *cell <= when.instant(),
            Operator::OnOrAfter => *cell >= when.instant(),
            _ => false,
        },
        (Datum::Bool(cell), Operator::Is, Operand::Bool(operand)) => cell == operand,
        (Datum::Who(cell), Operator::Is | Operator::IsNot, Operand::Id(id)) => {
            cell.to_string() == *id
        }
        _ => false,
    };
    match condition.operator {
        Operator::IsNot | Operator::DoesNotContain => !positive,
        _ => positive,
    }
}

/// How two rows order under one sort (`None` cells last, either way).
pub fn compare(spec: &Spec, direction: Direction, a: &RowFacts, b: &RowFacts) -> Ordering {
    let (x, y) = (datum(spec, a), datum(spec, b));
    let options: Vec<String> = options_of(&spec.config)
        .into_iter()
        .map(|option| option.id)
        .collect();
    let key = |datum: &Datum| -> Option<SortKey> {
        match datum {
            Datum::Text(text) if !text.is_empty() || spec.ty == PropertyType::Title => {
                Some(SortKey::Text(text.to_lowercase()))
            }
            Datum::Number(number) => Some(SortKey::Number(*number)),
            Datum::Instant(at) => Some(SortKey::Instant(*at)),
            Datum::Bool(on) => Some(SortKey::Bool(*on)),
            Datum::Id(id) => options
                .iter()
                .position(|option| option == id)
                .map(SortKey::Position),
            _ => None,
        }
    };
    match (key(&x), key(&y)) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => match direction {
            Direction::Asc => x.cmp(&y),
            Direction::Desc => y.cmp(&x),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SortKey {
    Text(String),
    Number(i128),
    Instant(DateTime<Utc>),
    Bool(bool),
    Position(usize),
}

/// One summary over `rows`, as the SQL shows it (`None` when there is
/// nothing to summarise). Numbers are canonical decimals.
pub fn summarise(spec: &Spec, kind: SummaryKind, rows: &[&RowFacts]) -> Option<String> {
    let render = |scaled: i128| {
        let sign = if scaled < 0 { "-" } else { "" };
        let magnitude = scaled.unsigned_abs();
        let text = format!("{sign}{}.{:09}", magnitude / 1_000_000_000, magnitude % 1_000_000_000);
        parse_decimal(&text)
    };
    let numbers: Vec<i128> = rows
        .iter()
        .filter_map(|row| match datum(spec, row) {
            Datum::Number(number) => Some(number),
            _ => None,
        })
        .collect();
    match kind {
        SummaryKind::Count => Some(rows.len().to_string()),
        SummaryKind::Sum if numbers.is_empty() => None,
        SummaryKind::Sum => render(numbers.iter().sum()),
        SummaryKind::Min => numbers.iter().min().copied().and_then(render),
        SummaryKind::Max => numbers.iter().max().copied().and_then(render),
        SummaryKind::PercentChecked => {
            if rows.is_empty() {
                return None;
            }
            let checked = rows
                .iter()
                .filter(|row| datum(spec, row) == Datum::Bool(true))
                .count();
            let hundredths = i128::try_from(checked).ok()? * 10_000 / i128::try_from(rows.len()).ok()?;
            // `round(…, 2)` rounds half up; the test data keeps to counts
            // whose share is exact to two decimals.
            let text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
            parse_decimal(&text)
        }
        SummaryKind::Average => None,
    }
}
