//! `GET /api/v1/w/{workspace_id}/search` — search what the caller may open.

mod sql;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    time::Duration,
};

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    db::BeginFor as _,
    extract,
    operations::ReadRules,
    rate_limit::{Limit, RateKey},
    search::{Person, PersonRole},
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use self::sql::{Order, Scope};
use crate::{
    domain::{
        index,
        query::{self, Filter, Flag, Mailbox, PersonField, Query, Term, TooLong, Warning},
        segments::{self, Segment},
    },
    scopes::SearchRead,
};

/// Page size when none is asked for.
pub const DEFAULT_LIMIT: u32 = 20;
/// Largest page.
pub const MAX_LIMIT: u32 = 50;
/// Relevance-ordered results stop here; refine the query for more.
pub const MAX_RELEVANCE_RESULTS: i64 = 1_000;

/// Queries per caller per minute: generous for typing, a ceiling for scripts.
const LIMIT: Limit = Limit::new("search.query", 600, Duration::from_mins(1));

/// How results are ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// Best text matches first, newer ones ahead of older ones.
    #[default]
    Relevance,
    /// Newest first.
    Newest,
}

/// What to search for.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchParams {
    /// The query: words, `"phrases"`, `-word`, `a OR b` and operators
    /// (`from:`, `to:`, `cc:`, `subject:`, `has:attachment`, `in:`, `is:`,
    /// `label:`, `before:`, `after:`, `older_than:`, `newer_than:`). At most
    /// 512 characters.
    #[serde(default)]
    pub q: String,
    /// `relevance` (default) or `newest`.
    pub sort: Option<Sort>,
    /// `true` while the member is typing: the last word matches as a prefix.
    #[serde(default)]
    pub prefix: bool,
    /// Page size, 1 to 50 (default 20).
    pub limit: Option<u32>,
    /// `next_cursor` of the previous page, with the same query and order.
    pub cursor: Option<String>,
    /// The caller's time zone for `before:` and `after:` (IANA, default UTC).
    pub tz: Option<String>,
}

/// One result: a record the caller may open, with what a list row shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Hit {
    /// The record (a conversation).
    pub record: RecordRef,
    /// Its container (an inbox).
    pub container: RecordRef,
    /// The entry that matched best (a message), when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched: Option<RecordRef>,
    /// The title, hits marked.
    pub title: Vec<Segment>,
    /// A snippet of the best entry, hits marked.
    pub snippet: Vec<Segment>,
    /// Who wrote the best entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<HitPerson>,
    /// The record's time (a conversation's last message).
    pub occurred_at: DateTime<Utc>,
    /// The record's state, for the row.
    pub facets: HitFacets,
}

/// A person on a hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HitPerson {
    /// Their display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Their address.
    pub address: String,
}

/// A hit's state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HitFacets {
    /// Its stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    /// Something in it is unread.
    #[serde(default)]
    pub unread: bool,
    /// Starred.
    #[serde(default)]
    pub starred: bool,
    /// How many entries (messages) it has.
    #[serde(default)]
    pub count: u32,
    /// Some entry carries files.
    #[serde(default)]
    pub attachment: bool,
}

/// A filter the query applied, for chips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AppliedFilter {
    /// The operator (`from`, `has`, `in`, `is`, `label`, …).
    pub operator: String,
    /// Its value.
    pub value: String,
}

/// The query as understood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ParsedQuery {
    /// The free text, as searched.
    pub text: String,
    /// The filters applied.
    pub filters: Vec<AppliedFilter>,
    /// What was not understood (and was searched as text, or ignored).
    pub warnings: Vec<QueryWarning>,
}

/// Something in the query that was not understood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct QueryWarning {
    /// `unknown_operator`, `empty_value` or `invalid_value`.
    pub code: String,
    /// The operator (or `tz`).
    pub operator: String,
}

impl From<Warning> for QueryWarning {
    fn from(warning: Warning) -> Self {
        Self {
            code: warning.code.to_owned(),
            operator: warning.operator,
        }
    }
}

/// A page of results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SearchResults {
    /// The results, in the order asked for.
    pub hits: Vec<Hit>,
    /// Pass it as `cursor` for the next page; `None` on the last one.
    pub next_cursor: Option<String>,
    /// The index of some source is still being built: older records may be
    /// missing for now.
    pub indexing: bool,
    /// The query as understood.
    pub query: ParsedQuery,
}

/// Search the records the caller may open.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/search",
    operation_id = "search.query",
    tag = "search",
    security(("bearer" = ["search:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["search.result"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), SearchParams),
    responses(
        (status = OK, description = "A page of results.", body = SearchResults),
        (status = BAD_REQUEST, description = "A query too long, a bad limit, or a cursor from another query.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many queries; try again shortly.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<SearchRead>,
    extract::Query(params): extract::Query<SearchParams>,
) -> Result<Json<SearchResults>> {
    let caller = match &workspace.principal.client_id {
        Some(client) if workspace.access.user.is_none() => format!("client:{client}"),
        _ => format!("subject:{}", workspace.principal.subject),
    };
    state
        .rate_limits
        .check(&state.db, &LIMIT, &RateKey::text(&caller))
        .await?;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_limit").arg("max", MAX_LIMIT),
        ));
    }
    let mut parsed = query::parse(&params.q, params.prefix).map_err(too_long)?;
    let sort = params.sort.unwrap_or_default();
    let fingerprint = fingerprint(&params, sort);
    let order = match params.cursor.as_deref() {
        Some(cursor) => decode_cursor(cursor, sort, fingerprint)?,
        None => match sort {
            Sort::Relevance => Order::Relevance { offset: 0 },
            Sort::Newest => Order::Newest { after: None },
        },
    };

    // Where the caller may look: sources they hold a read scope for, and
    // the containers the visibility port lists for them, asked now.
    let rules = ReadRules::from_state(&state);
    let access = &workspace.access;
    let mut scope = Scope {
        workspace: *access.workspace.as_uuid(),
        group_types: Vec::new(),
        containers: Vec::new(),
        tz: "UTC".to_owned(),
    };
    for source in state.search_sources.iter() {
        if !rules.allows(&workspace.principal, source.group_type()) {
            continue;
        }
        scope.group_types.push(source.group_type().to_owned());
        let visible = state
            .visibility
            .visible_containers(&state.db, access, source.container_type())
            .await?;
        scope.containers.extend(
            visible
                .into_iter()
                .filter(|c| c.record_type == source.container_type())
                .map(|c| format!("{}:{}", c.record_type, c.id)),
        );
    }

    let mut tx = state.db.begin_for(access.workspace).await?;
    for group_type in &scope.group_types {
        index::ensure_backfill(&mut tx, access.workspace, group_type).await?;
    }
    let indexing = index::indexing(&mut tx, access.workspace, &scope.group_types).await?;
    if let Some(tz) = params.tz.as_deref().filter(|tz| *tz != "UTC") {
        if uses_dates(&parsed) && known_zone(&mut tx, tz).await? {
            tz.clone_into(&mut scope.tz);
        } else if uses_dates(&parsed) {
            parsed.warnings.push(Warning {
                code: "invalid_value",
                operator: "tz".to_owned(),
            });
        }
    }

    let mut hits = Vec::new();
    let mut next_cursor = None;
    let searching = !(parsed.clauses.is_empty() && parsed.filters.is_empty());
    if searching && !scope.containers.is_empty() {
        let fetch = i64::from(limit) + 1;
        let rows = sql::page(&parsed, &scope, order, fetch)
            .build()
            .fetch_all(&mut *tx)
            .await?;
        let more = rows.len() > limit as usize;
        let rows = &rows[..rows.len().min(limit as usize)];
        let best: Vec<Uuid> = rows
            .iter()
            .filter_map(|row| row.try_get::<Option<Uuid>, _>("best").ok().flatten())
            .collect();
        let mut marks: HashMap<Uuid, Marks> = HashMap::new();
        if !best.is_empty() {
            for row in sql::highlights(&parsed, best)
                .build()
                .fetch_all(&mut *tx)
                .await?
            {
                let id: Uuid = row.try_get("id")?;
                let people: serde_json::Value = row.try_get("people")?;
                let title: Option<String> = row.try_get("title")?;
                let snippet: Option<String> = row.try_get("snippet")?;
                marks.insert(
                    id,
                    Marks {
                        record: RecordRef::new(
                            row.try_get::<String, _>("record_type")?,
                            &row.try_get::<String, _>("record_id")?,
                        ),
                        people: serde_json::from_value(people).unwrap_or_default(),
                        title: title.unwrap_or_default(),
                        snippet: snippet.unwrap_or_default(),
                    },
                );
            }
        }
        for row in rows {
            hits.push(hit(row, &marks, parsed.clauses.is_empty())?);
        }
        if more {
            next_cursor = match order {
                Order::Relevance { offset } => {
                    let next = offset + i64::from(limit);
                    (next < MAX_RELEVANCE_RESULTS)
                        .then(|| encode_cursor(&Cursor::relevance(fingerprint, next)))
                }
                Order::Newest { .. } => hits.last().and_then(|last| {
                    let id = rows.last()?.try_get::<Uuid, _>("id").ok()?;
                    Some(encode_cursor(&Cursor::newest(fingerprint, last.occurred_at, id)))
                }),
            };
        }
    }
    tx.commit().await?;
    Ok(Json(SearchResults {
        hits,
        next_cursor,
        indexing,
        query: describe(parsed),
    }))
}

/// Over a limit of the query language (characters, terms or operators).
fn too_long(_: TooLong) -> Error {
    Error::BadRequest(crate::MESSAGES.reason("query_too_long"))
}

fn uses_dates(query: &Query) -> bool {
    query
        .filters
        .iter()
        .any(|f| matches!(f, Filter::After(_) | Filter::Before(_)))
}

async fn known_zone(conn: &mut sqlx::PgConnection, tz: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1) AS "known!""#,
        tz
    )
    .fetch_one(conn)
    .await?)
}

/// The best entry of a result: which it is, its people, and its highlighted
/// title and snippet.
#[derive(Debug, Clone)]
struct Marks {
    record: RecordRef,
    people: Vec<Person>,
    title: String,
    snippet: String,
}

/// One result row and its highlights as a [`Hit`].
fn hit(row: &sqlx::postgres::PgRow, marks: &HashMap<Uuid, Marks>, plain: bool) -> Result<Hit> {
    let best: Option<Uuid> = row.try_get("best")?;
    let facets: serde_json::Value = row.try_get("facets")?;
    let record_type: String = row.try_get("record_type")?;
    let marks = best.and_then(|best| marks.get(&best));
    let (title, snippet) = marks.map_or((String::new(), String::new()), |m| {
        (m.title.clone(), m.snippet.clone())
    });
    let from = marks
        .into_iter()
        .flat_map(|m| m.people.iter())
        .find(|p| p.role == PersonRole::From)
        .map(|p| HitPerson {
            name: p.name.clone(),
            address: p.address.clone(),
        });
    Ok(Hit {
        record: RecordRef::new(record_type, &row.try_get::<String, _>("record_id")?),
        container: RecordRef::new(
            row.try_get::<String, _>("container_type")?,
            &row.try_get::<String, _>("container_id")?,
        ),
        matched: marks.map(|m| m.record.clone()),
        title: if plain {
            segments::plain(&title)
        } else {
            segments::split(&title)
        },
        snippet: if plain {
            segments::plain(&snippet)
        } else {
            segments::split(&snippet)
        },
        from,
        occurred_at: row.try_get("occurred_at")?,
        facets: serde_json::from_value(facets).unwrap_or_default(),
    })
}

/// The query as the client shows it: its free text and filters as chips.
fn describe(query: Query) -> ParsedQuery {
    let text = query
        .clauses
        .iter()
        .map(|clause| {
            let any: Vec<String> = clause
                .any
                .iter()
                .map(|term| match term {
                    Term::Word(word) | Term::Prefix(word) => word.clone(),
                    Term::Phrase(phrase) => format!("\"{phrase}\""),
                })
                .collect();
            let joined = any.join(" OR ");
            if clause.negated {
                format!("-{joined}")
            } else {
                joined
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let filters = query
        .filters
        .iter()
        .map(|filter| {
            let (operator, value) = match filter {
                Filter::Person(PersonField::From, v) => ("from", v.clone()),
                Filter::Person(PersonField::To, v) => ("to", v.clone()),
                Filter::Person(PersonField::Cc, v) => ("cc", v.clone()),
                Filter::Subject(v) => ("subject", v.clone()),
                Filter::HasAttachment => ("has", "attachment".to_owned()),
                Filter::Mailbox(m) => (
                    "in",
                    match m {
                        Mailbox::Inbox => "inbox",
                        Mailbox::Archive => "archive",
                        Mailbox::Spam => "spam",
                        Mailbox::Trash => "trash",
                        Mailbox::Anywhere => "anywhere",
                    }
                    .to_owned(),
                ),
                Filter::Stream(s) => ("in", s.clone()),
                Filter::Is(flag) => (
                    "is",
                    match flag {
                        Flag::Unread => "unread",
                        Flag::Read => "read",
                        Flag::Starred => "starred",
                    }
                    .to_owned(),
                ),
                Filter::Label(v) => ("label", v.clone()),
                Filter::After(d) => ("after", d.to_string()),
                Filter::Before(d) => ("before", d.to_string()),
                Filter::NewerThanDays(n) => ("newer_than", format!("{n}d")),
                Filter::OlderThanDays(n) => ("older_than", format!("{n}d")),
            };
            AppliedFilter {
                operator: operator.to_owned(),
                value,
            }
        })
        .collect();
    ParsedQuery {
        text,
        filters,
        warnings: query.warnings.into_iter().map(Into::into).collect(),
    }
}

/// A cursor's content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Cursor {
    /// The query and order it belongs to.
    q: u64,
    /// The offset (relevance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    o: Option<i64>,
    /// The last group's time and id (newest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    t: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    i: Option<Uuid>,
}

impl Cursor {
    fn relevance(q: u64, offset: i64) -> Self {
        Self {
            q,
            o: Some(offset),
            t: None,
            i: None,
        }
    }

    fn newest(q: u64, at: DateTime<Utc>, id: Uuid) -> Self {
        Self {
            q,
            o: None,
            t: Some(at),
            i: Some(id),
        }
    }
}

/// What a cursor is bound to: the query text, prefix mode, time zone and
/// order. Stable for the same server build, which is all a cursor needs.
fn fingerprint(params: &SearchParams, sort: Sort) -> u64 {
    let mut hasher = DefaultHasher::new();
    params.q.trim().hash(&mut hasher);
    params.prefix.hash(&mut hasher);
    params.tz.hash(&mut hasher);
    (sort == Sort::Newest).hash(&mut hasher);
    hasher.finish()
}

fn encode_cursor(cursor: &Cursor) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).unwrap_or_default())
}

fn decode_cursor(cursor: &str, sort: Sort, fingerprint: u64) -> Result<Order> {
    let mismatch = || Error::BadRequest(crate::MESSAGES.reason("cursor_mismatch"));
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| mismatch())?;
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| mismatch())?;
    if cursor.q != fingerprint {
        return Err(mismatch());
    }
    match (sort, cursor) {
        (Sort::Relevance, Cursor { o: Some(offset), .. }) if (0..MAX_RELEVANCE_RESULTS).contains(&offset) => {
            Ok(Order::Relevance { offset })
        }
        (
            Sort::Newest,
            Cursor {
                t: Some(at),
                i: Some(id),
                ..
            },
        ) => Ok(Order::Newest {
            after: Some((at, id)),
        }),
        _ => Err(mismatch()),
    }
}
