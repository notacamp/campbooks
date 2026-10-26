//! SQL for a query over a table, cast by type.
//!
//! The functions return fragments for the module to put into one statement.
//! A fragment reads the row's page as `p` (`pages.pages`), its table row as
//! `tr` (`pages.table_rows`) and a stored property's cell as the alias the
//! caller joined ([`join`]). **Every operand is bound** ([`Binds`]), never
//! spliced into the text: the only literals are uuids (typed) and the
//! library's own constants.
//!
//! The casts are safe because a cell is checked against its type before it
//! is stored ([`super::value::validate`]); each cast is still guarded by the
//! JSON type of the cell, so a value written by an older version never
//! raises an error in a query.

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use sqlx::{Postgres, postgres::PgArguments, query::Query};
use uuid::Uuid;

use super::{
    PropertyError, PropertyType, Result, Spec,
    config::{options_of, shows_who},
    filter::{Condition, Direction, Filter, Operand, Operator, SummaryKind},
    value::When,
};

/// A value to bind to a placeholder.
#[derive(Debug, Clone, PartialEq)]
pub enum Bind {
    /// Text.
    Text(String),
    /// A list of texts.
    Texts(Vec<String>),
    /// An id.
    Uuid(Uuid),
    /// A boolean.
    Bool(bool),
    /// An instant.
    Instant(DateTime<Utc>),
    /// A day.
    Day(NaiveDate),
    /// A number.
    Int(i64),
    /// JSON.
    Json(Value),
}

/// The values bound to a statement, numbered from where the caller's own
/// placeholders end.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Binds {
    next: usize,
    items: Vec<Bind>,
}

impl Binds {
    /// Placeholders from `$first` on (the caller owns `$1` to `$first - 1`).
    pub const fn starting_at(first: usize) -> Self {
        Self {
            next: first,
            items: Vec::new(),
        }
    }

    /// Add a value; the placeholder to put in the SQL (`$3`).
    pub fn push(&mut self, bind: Bind) -> String {
        let placeholder = format!("${}", self.next + self.items.len());
        self.items.push(bind);
        placeholder
    }

    /// The values, in placeholder order.
    pub fn items(&self) -> &[Bind] {
        &self.items
    }

    /// Bind them all to `query`.
    pub fn apply<'q>(
        &'q self,
        mut query: Query<'q, Postgres, PgArguments>,
    ) -> Query<'q, Postgres, PgArguments> {
        for bind in &self.items {
            query = match bind {
                Bind::Text(text) => query.bind(text),
                Bind::Texts(texts) => query.bind(texts),
                Bind::Uuid(id) => query.bind(*id),
                Bind::Bool(on) => query.bind(*on),
                Bind::Instant(at) => query.bind(*at),
                Bind::Day(day) => query.bind(*day),
                Bind::Int(number) => query.bind(*number),
                Bind::Json(json) => query.bind(json),
            };
        }
        query
    }
}

/// What a property's value is in SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Number,
    Instant,
    Bool,
    Id,
    Json,
    Who,
}

/// The join that brings a stored property's cell in as `alias`, or none for
/// a property that comes from the page.
pub fn join(spec: &Spec, alias: &str) -> Option<String> {
    spec.ty.stored().then(|| {
        format!(
            "LEFT JOIN pages.row_values {alias} ON {alias}.row_id = p.id \
             AND {alias}.property_id = '{}'::uuid",
            spec.id
        )
    })
}

fn scalar(spec: &Spec, alias: &str) -> Result<(String, Kind)> {
    let a = alias;
    Ok(match spec.ty {
        PropertyType::Title => ("p.title".into(), Kind::Text),
        PropertyType::Text => (
            format!(
                "(CASE WHEN jsonb_typeof({a}.value) = 'array' THEN \
                 (SELECT string_agg(run ->> 'text', '') FROM jsonb_array_elements({a}.value) run) END)"
            ),
            Kind::Text,
        ),
        PropertyType::Url | PropertyType::Email | PropertyType::Phone => (
            format!("(CASE WHEN jsonb_typeof({a}.value) = 'string' THEN {a}.value #>> '{{}}' END)"),
            Kind::Text,
        ),
        PropertyType::Number => (
            format!(
                "(CASE WHEN jsonb_typeof({a}.value) = 'string' THEN ({a}.value #>> '{{}}')::numeric END)"
            ),
            Kind::Number,
        ),
        PropertyType::UniqueId => ("tr.number::numeric".into(), Kind::Number),
        PropertyType::Select | PropertyType::Status => (
            format!("(CASE WHEN jsonb_typeof({a}.value) = 'string' THEN {a}.value #>> '{{}}' END)"),
            Kind::Id,
        ),
        PropertyType::Date => (
            format!(
                "(CASE WHEN jsonb_typeof({a}.value) <> 'object' THEN NULL \
                 WHEN length({a}.value ->> 'start') = 10 THEN (({a}.value ->> 'start') || 'T00:00:00Z')::timestamptz \
                 ELSE ({a}.value ->> 'start')::timestamptz END)"
            ),
            Kind::Instant,
        ),
        PropertyType::Created if shows_who(&spec.config) => ("p.created_by".into(), Kind::Who),
        PropertyType::Edited if shows_who(&spec.config) => ("p.edited_by".into(), Kind::Who),
        PropertyType::Created => ("p.created_at".into(), Kind::Instant),
        PropertyType::Edited => ("p.edited_at".into(), Kind::Instant),
        PropertyType::Checkbox => (
            format!("(CASE WHEN jsonb_typeof({a}.value) = 'boolean' THEN ({a}.value #>> '{{}}')::boolean END)"),
            Kind::Bool,
        ),
        PropertyType::MultiSelect
        | PropertyType::Person
        | PropertyType::Relation
        | PropertyType::Files
        | PropertyType::CampbooksLink => (format!("{a}.value"), Kind::Json),
        _ => return Err(PropertyError::FILTER),
    })
}

fn empty(spec: &Spec, alias: &str) -> String {
    match spec.ty {
        PropertyType::Title => "(p.title = '')".into(),
        PropertyType::UniqueId | PropertyType::Created | PropertyType::Edited => "false".into(),
        _ => format!(
            "({alias}.value IS NULL OR {alias}.value = 'null'::jsonb \
             OR {alias}.value = '[]'::jsonb OR {alias}.value = '\"\"'::jsonb)"
        ),
    }
}

/// The SQL for one condition (a boolean expression).
pub fn condition(
    spec: &Spec,
    alias: &str,
    condition: &Condition,
    binds: &mut Binds,
) -> Result<String> {
    use Operator::{
        After, Before, Contains, DoesNotContain, EndsWith, Gt, Gte, Is, IsAnyOf, IsEmpty, IsNot,
        IsNotEmpty, Lt, Lte, OnOrAfter, OnOrBefore, StartsWith,
    };
    match condition.operator {
        IsEmpty => return Ok(empty(spec, alias)),
        IsNotEmpty => return Ok(format!("(NOT {})", empty(spec, alias))),
        _ => {}
    }
    let (e, kind) = scalar(spec, alias)?;
    let negated = matches!(condition.operator, IsNot | DoesNotContain);
    let positive = match (kind, condition.operator, &condition.operand) {
        (Kind::Text, Is | IsNot, Operand::Text(text)) => {
            format!("lower({e}) = lower({})", binds.push(Bind::Text(text.clone())))
        }
        (Kind::Text, Contains | DoesNotContain, Operand::Text(text)) => format!(
            "strpos(lower({e}), lower({})) > 0",
            binds.push(Bind::Text(text.clone()))
        ),
        (Kind::Text, StartsWith, Operand::Text(text)) => format!(
            "starts_with(lower({e}), lower({}))",
            binds.push(Bind::Text(text.clone()))
        ),
        (Kind::Text, EndsWith, Operand::Text(text)) => {
            let p = binds.push(Bind::Text(text.clone()));
            format!("right(lower({e}), char_length({p})) = lower({p})")
        }
        (Kind::Number, op @ (Is | IsNot | Gt | Gte | Lt | Lte), Operand::Number(number)) => {
            let symbol = match op {
                Is | IsNot => "=",
                Gt => ">",
                Gte => ">=",
                Lt => "<",
                _ => "<=",
            };
            let p = binds.push(Bind::Text(number.clone()));
            format!("{e} {symbol} {p}::numeric")
        }
        (Kind::Id, Is | IsNot, Operand::Id(id)) => {
            format!("{e} = {}", binds.push(Bind::Text(id.clone())))
        }
        (Kind::Id, IsAnyOf, Operand::Ids(ids)) => {
            format!("{e} = ANY({}::text[])", binds.push(Bind::Texts(ids.clone())))
        }
        (Kind::Json, Contains | DoesNotContain, Operand::Id(id)) => format!(
            "(jsonb_typeof({e}) = 'array' AND {e} @> jsonb_build_array({}::text))",
            binds.push(Bind::Text(id.clone()))
        ),
        (Kind::Json, Contains | DoesNotContain, Operand::Link { record, id }) => {
            let (r, i) = (
                binds.push(Bind::Text(record.clone())),
                binds.push(Bind::Text(id.clone())),
            );
            format!(
                "(jsonb_typeof({e}) = 'array' AND {e} @> \
                 jsonb_build_array(jsonb_build_object('type', {r}::text, 'id', {i}::text)))"
            )
        }
        (Kind::Instant, Is, Operand::When(when)) => format!(
            "({e} AT TIME ZONE 'UTC')::date = {}::date",
            binds.push(Bind::Day(when.day()))
        ),
        (Kind::Instant, op @ (Before | After | OnOrBefore | OnOrAfter), Operand::When(when)) => {
            let symbol = match op {
                Before => "<",
                After => ">",
                OnOrBefore => "<=",
                _ => ">=",
            };
            format!(
                "{e} {symbol} {}::timestamptz",
                binds.push(Bind::Instant(When::instant(*when)))
            )
        }
        (Kind::Bool, Is, Operand::Bool(on)) => {
            format!("COALESCE({e}, false) = {}", binds.push(Bind::Bool(*on)))
        }
        (Kind::Who, Is | IsNot, Operand::Id(id)) => {
            let id: Uuid = id.parse().map_err(|_| PropertyError::FILTER)?;
            format!("{e} = {}::uuid", binds.push(Bind::Uuid(id)))
        }
        _ => return Err(PropertyError::FILTER),
    };
    Ok(if negated {
        format!("(NOT COALESCE({positive}, false))")
    } else {
        format!("COALESCE({positive}, false)")
    })
}

/// The SQL for a whole filter. `alias_of` names the join of a property.
pub fn filter(
    tree: &Filter,
    specs: &std::collections::HashMap<Uuid, Spec>,
    alias_of: &dyn Fn(Uuid) -> String,
    binds: &mut Binds,
) -> Result<String> {
    match tree {
        Filter::Group { any, items } => {
            if items.is_empty() {
                return Ok("true".into());
            }
            let parts = items
                .iter()
                .map(|item| filter(item, specs, alias_of, binds))
                .collect::<Result<Vec<_>>>()?;
            Ok(format!(
                "({})",
                parts.join(if *any { " OR " } else { " AND " })
            ))
        }
        Filter::Condition(cond) => {
            let spec = specs.get(&cond.property).ok_or(PropertyError::FILTER)?;
            condition(spec, &alias_of(cond.property), cond, binds)
        }
    }
}

/// The expression one sort orders by, with its direction and `NULLS LAST`.
pub fn order_by(
    spec: &Spec,
    alias: &str,
    direction: Direction,
    binds: &mut Binds,
) -> Result<String> {
    let (e, kind) = scalar(spec, alias)?;
    let key = match (spec.ty, kind) {
        (PropertyType::Select | PropertyType::Status, _) => {
            let ids: Vec<String> = options_of(&spec.config)
                .into_iter()
                .map(|option| option.id)
                .collect();
            format!("array_position({}::text[], {e})", binds.push(Bind::Texts(ids)))
        }
        (_, Kind::Text) => format!("lower({e})"),
        (_, Kind::Bool) => format!("COALESCE({e}, false)"),
        (_, Kind::Number | Kind::Instant) => e,
        _ => return Err(PropertyError::SORT),
    };
    let way = match direction {
        Direction::Asc => "ASC",
        Direction::Desc => "DESC",
    };
    Ok(format!("{key} {way} NULLS LAST"))
}

/// The aggregate for one summary, as text (`NULL` when there is nothing to
/// summarise). Numbers come back with up to 9 decimals.
pub fn aggregate(spec: &Spec, alias: &str, kind: SummaryKind) -> Result<String> {
    if !super::filter::summary_allowed(spec.ty, kind) {
        return Err(PropertyError::SUMMARY);
    }
    if kind == SummaryKind::Count {
        return Ok("count(*)::text".into());
    }
    let (e, value_kind) = scalar(spec, alias).map_err(|_| PropertyError::SUMMARY)?;
    Ok(match (kind, value_kind) {
        (SummaryKind::Sum, _) => format!("sum({e})::text"),
        (SummaryKind::Average, _) => format!("round(avg({e}), 9)::text"),
        (SummaryKind::Min | SummaryKind::Max, Kind::Instant) => {
            let function = if kind == SummaryKind::Min { "min" } else { "max" };
            format!(
                "to_char({function}({e}) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"+00:00\"')"
            )
        }
        (SummaryKind::Min, _) => format!("min({e})::text"),
        (SummaryKind::Max, _) => format!("max({e})::text"),
        (SummaryKind::PercentChecked, _) => format!(
            "(CASE WHEN count(*) = 0 THEN NULL ELSE \
             round(100.0 * count(*) FILTER (WHERE COALESCE({e}, false)) / count(*), 2)::text END)"
        ),
        _ => return Err(PropertyError::SUMMARY),
    })
}
