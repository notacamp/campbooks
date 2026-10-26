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
    /// Only these rows (of the table, and live): to read a few by id.
    pub only: Option<&'a [PageId]>,
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

#[allow(
    clippy::too_many_lines,
    reason = "the statements are built and run in one sequence"
)]
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
        let spec = query
            .specs
            .get(&id)
            .ok_or_else(|| refused(crate::properties::PropertyError::FILTER))?;
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
        Some(filter) => {
            sql::filter(filter, query.specs, &alias_of, &mut filter_binds).map_err(refused)?
        }
        None => "true".to_owned(),
    };
    // Row ids are parsed UUIDs, so listing them as text carries nothing else.
    let only_sql = match query.only {
        Some(ids) if !ids.is_empty() => {
            let list: Vec<String> = ids.iter().map(|id| format!("'{id}'")).collect();
            format!(" AND p.id IN ({})", list.join(", "))
        }
        Some(_) => " AND false".to_owned(),
        None => String::new(),
    };
    let wheres =
        format!("WHERE tr.table_id = $1 AND p.trashed_at IS NULL AND {filter_sql}{only_sql}");

    // The rows, in order.
    let mut binds = filter_binds.clone();
    let mut order = Vec::new();
    for Sort {
        property,
        direction,
    } in query.sorts
    {
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
            sql::aggregate(&query.specs[property], &alias_of(*property), *kind).map_err(refused)?,
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
            (SummaryKind::Sum | SummaryKind::Average | SummaryKind::PercentChecked, _)
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
            let key =
                sql::option_key(&query.specs[&property], &alias_of(property)).map_err(refused)?;
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

#[cfg(test)]
mod tests {
    //! The SQL is held to the in-memory oracle (`properties::eval`) on
    //! random tables, against a real database, and measured at 20,000 rows.

    use std::{collections::HashMap, time::Instant};

    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::properties::{
        PropertyType as T, eval,
        filter::{self, Direction, Operator},
        tests::{Rng, WORDS, day, instant, members, row, specs},
    };

    const PAGE_LIMIT: usize = 100_000;

    struct World {
        table: PageId,
        specs: Vec<(&'static str, Spec)>,
        map: HashMap<Uuid, Spec>,
        ids: Vec<Uuid>,
        facts: Vec<eval::RowFacts>,
    }

    #[allow(
        clippy::too_many_lines,
        reason = "a fixture: every table the test reads"
    )]
    async fn build(db: &PgPool, rows: i64, clean: bool) -> World {
        let mut rng = Rng(0xBEEF);
        let all = specs();
        let ws = Uuid::now_v7();
        let table = PageId::new();
        let user = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO pages.pages (id, workspace_id, place, position, created_by, edited_by, kind)
             VALUES ($1, $2, 'workspace', 'a0', $3, $3, 'table')",
        )
        .bind(table)
        .bind(ws)
        .bind(user)
        .execute(db)
        .await
        .unwrap();
        for (name, spec) in &all {
            if spec.ty.stored() {
                sqlx::query(
                    "INSERT INTO pages.table_properties (id, table_id, workspace_id, name, type, config, position, created_by)
                     VALUES ($1, $2, $3, $4, $5, $6, 'a0', $7)",
                )
                .bind(spec.id)
                .bind(table)
                .bind(ws)
                .bind(*name)
                .bind(spec.ty.as_str())
                .bind(&spec.config)
                .bind(user)
                .execute(db)
                .await
                .unwrap();
            }
        }
        let clean_words = ["alpha", "beta", "gamma", "delta", "zulu", "north"];
        let mut ids = Vec::new();
        let mut facts = Vec::new();
        for number in 1..=rows {
            let mut fact = row(&mut rng, &all, number);
            if clean {
                fact.title = clean_words[rng.below(clean_words.len())].to_owned();
                for (_, spec) in &all {
                    if spec.ty == T::Text && !fact.cells[&spec.id].is_null() {
                        fact.cells
                            .insert(spec.id, json!([{"text": clean_words[rng.below(6)]}]));
                    }
                }
            }
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO pages.pages (id, workspace_id, parent_id, place, position, title, created_by,
                     edited_by, created_at, edited_at, kind)
                 VALUES ($1, $2, $3, 'workspace', 'a0', $4, $5, $6, $7, $8, 'row')",
            )
            .bind(id)
            .bind(ws)
            .bind(table)
            .bind(&fact.title)
            .bind(fact.created_by)
            .bind(fact.edited_by)
            .bind(fact.created_at)
            .bind(fact.edited_at)
            .execute(db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number) VALUES ($1, $2, $3, $4)")
                .bind(id)
                .bind(table)
                .bind(ws)
                .bind(number)
                .execute(db)
                .await
                .unwrap();
            for (property, value) in &fact.cells {
                if !value.is_null() {
                    sqlx::query(
                        "INSERT INTO pages.row_values (row_id, property_id, workspace_id, table_id, value, updated_by)
                         VALUES ($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(id)
                    .bind(property)
                    .bind(ws)
                    .bind(table)
                    .bind(value)
                    .bind(user)
                    .execute(db)
                    .await
                    .unwrap();
                }
            }
            ids.push(id);
            facts.push(fact);
        }
        let map = all.iter().map(|(_, s)| (s.id, s.clone())).collect();
        World {
            table,
            specs: all,
            map,
            ids,
            facts,
        }
    }

    fn operand(rng: &mut Rng, spec: &Spec, op: Operator) -> Value {
        match spec.ty {
            T::Title | T::Text => {
                let word = *rng.pick(WORDS);
                // A piece of a word, to try contains, starts and ends.
                let from = rng.below(word.len().max(1));
                json!(if rng.chance(50) {
                    word.to_owned()
                } else {
                    word[from..].chars().take(3).collect::<String>()
                })
            }
            T::Url => json!(format!("https://example.com/{}", rng.below(50))),
            T::Email => json!(format!("user{}@example.com", rng.below(50))),
            T::Phone => json!("+351 900"),
            T::Number => json!(format!("{}", rng.below(200))),
            T::UniqueId => json!(format!("{}", 1 + rng.below(60))),
            T::Select | T::Status | T::MultiSelect => {
                let options = crate::properties::config::options_of(&spec.config);
                if op == Operator::IsAnyOf {
                    json!([options[0].id, options[1].id])
                } else {
                    json!(rng.pick(&options).id)
                }
            }
            T::Date | T::Created | T::Edited
                if spec.config.get("of").and_then(Value::as_str) == Some("who") =>
            {
                json!(rng.pick(&members()).to_string())
            }
            T::Date | T::Created | T::Edited => {
                json!(if rng.chance(50) {
                    day(rng)
                } else {
                    instant(rng)
                })
            }
            T::Checkbox => json!(rng.chance(50)),
            T::Person => json!(rng.pick(&members()).to_string()),
            T::Relation => json!(Uuid::from_u128(100 + rng.below(5) as u128).to_string()),
            T::CampbooksLink => json!({"type": "tasks.task", "id": format!("id-{}", rng.below(5))}),
            _ => Value::Null,
        }
    }

    async fn ask(
        db: &PgPool,
        world: &World,
        filter: Option<&filter::Filter>,
        sorts: &[filter::Sort],
        summaries: &[filter::Summary],
    ) -> Answer {
        let mut conn = db.acquire().await.unwrap();
        run(
            &mut conn,
            &Query {
                table: world.table,
                specs: &world.map,
                filter,
                sorts,
                summaries,
                group_by: None,
                limit: PAGE_LIMIT,
                offset: 0,
                only: None,
            },
        )
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn every_operator_of_every_type_answers_as_the_oracle_does(db: PgPool) {
        let world = build(&db, 80, false).await;
        let mut rng = Rng(77);
        let mut checked = 0;
        for (name, spec) in &world.specs {
            for op in filter::operators(spec) {
                for _ in 0..4 {
                    let value = operand(&mut rng, spec, *op);
                    let raw = json!({"property": spec.id, "operator": op, "value": value});
                    let Ok(tree) = filter::parse(&raw, &world.map) else {
                        continue;
                    };
                    let expected: Vec<Uuid> = world
                        .facts
                        .iter()
                        .zip(&world.ids)
                        .filter(|(fact, _)| eval::matches(&tree, &world.map, fact))
                        .map(|(_, id)| *id)
                        .collect();
                    let answer = ask(&db, &world, Some(&tree), &[], &[]).await;
                    let got: Vec<Uuid> = answer.ids.iter().map(|id| *id.as_uuid()).collect();
                    assert_eq!(got, expected, "{name} {op:?} {raw}");
                    assert_eq!(answer.matching, i64::try_from(expected.len()).unwrap());
                    checked += 1;
                }
            }
        }
        assert!(checked > 150, "{checked}");
        // And groups of conditions.
        for _ in 0..30 {
            let pick = |rng: &mut Rng| {
                let (_, spec) = &world.specs[rng.below(world.specs.len())];
                let op = *rng.pick(filter::operators(spec));
                json!({"property": spec.id, "operator": op, "value": operand(rng, spec, op)})
            };
            let raw = json!({"or": [{"and": [pick(&mut rng), pick(&mut rng)]}, pick(&mut rng)]});
            let Ok(tree) = filter::parse(&raw, &world.map) else {
                continue;
            };
            let expected: Vec<Uuid> = world
                .facts
                .iter()
                .zip(&world.ids)
                .filter(|(fact, _)| eval::matches(&tree, &world.map, fact))
                .map(|(_, id)| *id)
                .collect();
            let got: Vec<Uuid> = ask(&db, &world, Some(&tree), &[], &[])
                .await
                .ids
                .iter()
                .map(|id| *id.as_uuid())
                .collect();
            assert_eq!(got, expected, "{raw}");
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sorts_and_summaries_answer_as_the_oracle_does(db: PgPool) {
        let world = build(&db, 40, true).await;
        for (name, spec) in world.specs.iter().filter(|(_, s)| s.sortable()) {
            for direction in [Direction::Asc, Direction::Desc] {
                let sort = filter::Sort {
                    property: spec.id,
                    direction,
                };
                let mut expected: Vec<usize> = (0..world.facts.len()).collect();
                expected.sort_by(|a, b| {
                    eval::compare(spec, direction, &world.facts[*a], &world.facts[*b])
                        .then(world.facts[*a].number.cmp(&world.facts[*b].number))
                });
                let got = ask(&db, &world, None, &[sort], &[]).await;
                let want: Vec<Uuid> = expected.iter().map(|i| world.ids[*i]).collect();
                let got: Vec<Uuid> = got.ids.iter().map(|id| *id.as_uuid()).collect();
                assert_eq!(got, want, "{name} {direction:?}");
            }
        }
        for (name, spec) in &world.specs {
            for kind in [
                SummaryKind::Count,
                SummaryKind::Sum,
                SummaryKind::Min,
                SummaryKind::Max,
                SummaryKind::PercentChecked,
            ] {
                if !filter::summary_allowed(spec.ty, kind)
                    || matches!(spec.ty, T::Date | T::Created | T::Edited)
                {
                    continue;
                }
                let summary = filter::Summary {
                    property: spec.id,
                    kind,
                };
                let answer = ask(&db, &world, None, &[], &[summary]).await;
                let rows: Vec<&eval::RowFacts> = world.facts.iter().collect();
                assert_eq!(
                    answer.summaries[0].value,
                    eval::summarise(spec, kind, &rows),
                    "{name} {kind:?}"
                );
            }
        }
    }

    /// 20,000 rows, a filter on two types, a sort and three summaries.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn twenty_thousand_rows_are_queried_fast(db: PgPool) {
        let all = specs();
        let ws = Uuid::now_v7();
        let table = PageId::new();
        let user = Uuid::now_v7();
        sqlx::query("INSERT INTO pages.pages (id, workspace_id, place, position, created_by, edited_by, kind) VALUES ($1, $2, 'workspace', 'a0', $3, $3, 'table')")
            .bind(table).bind(ws).bind(user).execute(&db).await.unwrap();
        let select = &all.iter().find(|(n, _)| *n == "select").unwrap().1;
        let number = &all.iter().find(|(n, _)| *n == "number").unwrap().1;
        for spec in [select, number] {
            sqlx::query("INSERT INTO pages.table_properties (id, table_id, workspace_id, name, type, config, position, created_by) VALUES ($1, $2, $3, 'p', $4, $5, 'a0', $6)")
                .bind(spec.id).bind(table).bind(ws).bind(spec.ty.as_str()).bind(&spec.config).bind(user).execute(&db).await.unwrap();
        }
        sqlx::query(
            "WITH made AS (
                 INSERT INTO pages.pages (id, workspace_id, parent_id, place, position, title, created_by, edited_by, kind)
                 SELECT gen_random_uuid(), $2, $1, 'workspace', 'a0', 'row ' || n, $3, $3, 'row' FROM generate_series(1, 20000) n
                 RETURNING id, title),
             numbered AS (INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number)
                 SELECT id, $1, $2, substr(title, 5)::bigint FROM made RETURNING row_id, number),
             opts AS (INSERT INTO pages.row_values (row_id, property_id, workspace_id, table_id, value, updated_by)
                 SELECT row_id, $4, $2, $1, to_jsonb('s' || (1 + number % 3)), $3 FROM numbered RETURNING 1)
             INSERT INTO pages.row_values (row_id, property_id, workspace_id, table_id, value, updated_by)
             SELECT row_id, $5, $2, $1, to_jsonb((number % 1000)::text), $3 FROM numbered",
        )
        .bind(table).bind(ws).bind(user).bind(select.id).bind(number.id)
        .execute(&db).await.unwrap();
        sqlx::query("ANALYZE").execute(&db).await.unwrap();
        let map: HashMap<Uuid, Spec> = [select, number]
            .iter()
            .map(|s| (s.id, (*s).clone()))
            .collect();
        let raw = json!({"and": [
            {"property": select.id, "operator": "is_not", "value": "s3"},
            {"property": number.id, "operator": "gt", "value": "100"}]});
        let tree = filter::parse(&raw, &map).unwrap();
        let sorts = [filter::Sort {
            property: number.id,
            direction: Direction::Desc,
        }];
        let summaries = [
            filter::Summary {
                property: number.id,
                kind: SummaryKind::Sum,
            },
            filter::Summary {
                property: number.id,
                kind: SummaryKind::Max,
            },
            filter::Summary {
                property: select.id,
                kind: SummaryKind::Count,
            },
        ];
        let mut conn = db.acquire().await.unwrap();
        let mut best = std::time::Duration::MAX;
        let mut answer = None;
        for _ in 0..3 {
            let started = Instant::now();
            answer = Some(
                run(
                    &mut conn,
                    &Query {
                        table,
                        specs: &map,
                        filter: Some(&tree),
                        sorts: &sorts,
                        summaries: &summaries,
                        group_by: Some(select.id),
                        limit: 100,
                        offset: 0,
                        only: None,
                    },
                )
                .await
                .unwrap(),
            );
            best = best.min(started.elapsed());
        }
        let answer = answer.unwrap();
        println!("query_table over 20000 rows: {best:?}");
        assert_eq!(answer.ids.len(), 100);
        assert!(
            answer.matching > 10_000 && answer.matching < 20_000,
            "{}",
            answer.matching
        );
        assert_eq!(
            answer.summaries[2].value.as_deref(),
            Some(&*answer.matching.to_string())
        );
        assert!(best < std::time::Duration::from_secs(2), "{best:?}");
    }
}
