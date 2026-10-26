//! Answering a query over a table in the database (design.md, "Operations").
//!
//! The filter tree, the sorts and the summaries become one statement over
//! `pages.table_rows` and `pages.pages`, with each stored property a join of
//! `pages.row_values`, cast by type by the properties library. Every operand
//! is bound. The rows are the table's live ones: a row in the trash is
//! excluded, and a row is as readable as its table (the caller was checked
//! against the table's page before this runs).

use std::collections::HashMap;

use campbooks_kernel::Result;
use serde::{Deserialize, Serialize};
use sqlx::{AssertSqlSafe, PgConnection, Row as _};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{PageId, tables::refused};
use crate::properties::{
    PropertyType, Spec,
    filter::{Filter, Sort, Summary, SummaryKind},
    sql::{self, Binds},
    value::parse_decimal,
};

/// What to ask.
#[derive(Debug, Clone)]
pub struct Query<'a> {
    /// The table.
    pub table: PageId,
    /// Its live properties.
    pub specs: &'a HashMap<Uuid, Spec>,
    /// Which rows.
    pub filter: Option<&'a Filter>,
    /// In what order.
    pub sorts: &'a [Sort],
    /// What to total.
    pub summaries: &'a [Summary],
    /// For a board, the select or status property its columns are.
    pub group_by: Option<Uuid>,
    /// How many rows to answer.
    pub limit: usize,
    /// How many to skip.
    pub offset: usize,
}

/// One summary, computed over every row the filter keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SummaryValue {
    /// The property.
    pub property: Uuid,
    /// What it shows.
    pub kind: SummaryKind,
    /// The figure: a number, a date or a share (0 to 100) as text; none when
    /// there is nothing to summarise.
    pub value: Option<String>,
}

/// How many rows a board column holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GroupSize {
    /// The option; none for the rows with no value.
    pub option: Option<String>,
    /// How many rows the filter keeps in it.
    pub rows: i64,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The rows of this page, in order.
    pub ids: Vec<PageId>,
    /// How many rows the filter keeps in all.
    pub matching: i64,
    /// The summaries.
    pub summaries: Vec<SummaryValue>,
    /// For a board, the size of each column.
    pub groups: Option<Vec<GroupSize>>,
}

/// Run `query`.
pub async fn run(conn: &mut PgConnection, query: &Query<'_>) -> Result<Answer> {
    // The properties the statement reads, each joined once.
    let mut wanted: Vec<Uuid> = Vec::new();
    if let Some(filter) = query.filter {
        filter.properties(&mut wanted);
    }
    wanted.extend(query.sorts.iter().map(|sort| sort.property));
    wanted.extend(query.summaries.iter().map(|summary| summary.property));
    wanted.extend(query.group_by);
    let mut aliases: HashMap<Uuid, String> = HashMap::new();
    let mut joins = String::new();
    for id in wanted {
        let spec = query.specs.get(&id).ok_or_else(|| {
            refused(crate::properties::PropertyError::FILTER)
        })?;
        if aliases.contains_key(&id) {
            continue;
        }
        let alias = format!("v{}", aliases.len());
        if let Some(join) = sql::join(spec, &alias) {
            joins.push(' ');
            joins.push_str(&join);
        }
        aliases.insert(id, alias);
    }
    let alias_of = |id: Uuid| aliases.get(&id).cloned().unwrap_or_default();
    let from = format!("FROM pages.table_rows tr JOIN pages.pages p ON p.id = tr.row_id{joins}");

    // $1 is the table; the filter's operands follow.
    let mut filter_binds = Binds::starting_at(2);
    let filter_sql = match query.filter {
        Some(filter) => sql::filter(filter, query.specs, &alias_of, &mut filter_binds)
            .map_err(refused)?,
        None => "true".to_owned(),
    };
    let wheres = format!(
        "WHERE tr.table_id = $1 AND p.trashed_at IS NULL AND {filter_sql}"
    );

    // The rows, in order.
    let mut binds = filter_binds.clone();
    let mut order = Vec::new();
    for Sort { property, direction } in query.sorts {
        let spec = &query.specs[property];
        let mut sort_binds = Binds::starting_at(2 + binds.items().len());
        order.push(
            sql::order_by(spec, &alias_of(*property), *direction, &mut sort_binds)
                .map_err(refused)?,
        );
        binds.extend(&sort_binds);
    }
    order.push("tr.number ASC".to_owned());
    let statement = format!(
        "SELECT p.id {from} {wheres} ORDER BY {} LIMIT {} OFFSET {}",
        order.join(", "),
        query.limit,
        query.offset
    );
    // The statements are built from the library's constants and the table's
    // own ids; every operand is bound.
    let rows = binds
        .apply(sqlx::query(AssertSqlSafe(statement)).bind(query.table))
        .fetch_all(&mut *conn)
        .await?;
    let ids = rows
        .iter()
        .map(|row| row.try_get::<PageId, _>(0))
        .collect::<Result<Vec<_>, _>>()?;

    // How many in all, and the summaries, in one pass.
    let mut selects = vec!["count(*)::bigint".to_owned()];
    for Summary { property, kind } in query.summaries {
        selects.push(
            sql::aggregate(&query.specs[property], &alias_of(*property), *kind)
                .map_err(refused)?,
        );
    }
    let statement = format!("SELECT {} {from} {wheres}", selects.join(", "));
    let row = filter_binds
        .apply(sqlx::query(AssertSqlSafe(statement)).bind(query.table))
        .fetch_one(&mut *conn)
        .await?;
    let matching: i64 = row.try_get(0)?;
    let mut summaries = Vec::new();
    for (index, Summary { property, kind }) in query.summaries.iter().enumerate() {
        let raw: Option<String> = row.try_get(index + 1)?;
        let ty = query.specs[property].ty;
        let value = raw.and_then(|text| match (kind, ty) {
            (SummaryKind::Count, _) => Some(text),
            (
                SummaryKind::Sum | SummaryKind::Average | SummaryKind::PercentChecked,
                _,
            )
            | (
                SummaryKind::Min | SummaryKind::Max,
                PropertyType::Number | PropertyType::UniqueId,
            ) => parse_decimal(&text),
            _ => Some(text),
        });
        summaries.push(SummaryValue {
            property: *property,
            kind: *kind,
            value,
        });
    }

    let groups = match query.group_by {
        Some(property) => {
            let key = sql::option_key(&query.specs[&property], &alias_of(property))
                .map_err(refused)?;
            let statement = format!(
                "SELECT {key} AS option, count(*)::bigint AS rows {from} {wheres} \
                 GROUP BY 1 ORDER BY 1 NULLS LAST"
            );
            let sizes = filter_binds
                .apply(sqlx::query(AssertSqlSafe(statement)).bind(query.table))
                .fetch_all(&mut *conn)
                .await?;
            Some(
                sizes
                    .iter()
                    .map(|row| {
                        Ok(GroupSize {
                            option: row.try_get(0)?,
                            rows: row.try_get(1)?,
                        })
                    })
                    .collect::<Result<Vec<_>, sqlx::Error>>()?,
            )
        }
        None => None,
    };
    Ok(Answer {
        ids,
        matching,
        summaries,
        groups,
    })
}
