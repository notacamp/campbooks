//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/query` — the rows of a
//! table for a view, or for a filter and sorts of your own, 100 at a time,
//! with the summaries of the view.
//!
//! It is a read served by POST: the filter holds what members typed, which
//! URLs would leak to logs and history. The answer is computed in the
//! database, cast by type. The rows are the table's live ones; who may read
//! the table may read them.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId, ViewId,
        table_query::{self, GroupSize, Query, SummaryValue},
        table_views::{self, PAGE, ViewType},
        tables::{self, MAX_ROWS, RowView, TablePath, refused},
        tracked,
    },
    properties::filter,
    scopes::PagesRead,
};

/// What to ask. Everything is optional: with nothing, all rows in the order
/// they were added.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TableQuery {
    /// A view to ask for: its filter, sorts, summaries and (for a board) its
    /// groups.
    #[serde(default)]
    pub view: Option<ViewId>,
    /// A filter of your own, in the form of a view's: a tree of
    /// `{"and": [...]}` / `{"or": [...]}` groups (three levels) of
    /// `{"property", "operator", "value"}` conditions. With a view, both
    /// must hold.
    #[serde(default)]
    pub filter: Option<Value>,
    /// Sorts of your own (up to five `{"property", "direction"}`), in place
    /// of the view's.
    #[serde(default)]
    pub sorts: Option<Value>,
    /// Summaries of your own (`{"property", "kind"}`), in place of the
    /// view's.
    #[serde(default)]
    pub summaries: Option<Value>,
    /// How many rows, 1 to 100. Default 100.
    #[serde(default)]
    pub limit: Option<usize>,
    /// The `next_cursor` of the previous answer.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// The rows asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TableRows {
    /// The rows of this page, in order.
    pub rows: Vec<RowView>,
    /// Pass it back as `cursor` for the next rows; none at the end.
    pub next_cursor: Option<String>,
    /// How many rows the filter keeps in all.
    pub matching: i64,
    /// The summaries, over every row the filter keeps.
    pub summaries: Vec<SummaryValue>,
    /// For a board: how many rows each column holds.
    pub groups: Option<Vec<GroupSize>>,
}

fn offset_of(cursor: Option<&str>) -> Result<usize> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    cursor
        .strip_prefix('o')
        .and_then(|number| number.parse::<usize>().ok())
        .filter(|offset| *offset <= usize::try_from(MAX_ROWS).unwrap_or(usize::MAX))
        .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_cursor")))
}

/// Query a table.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/query",
    operation_id = "pages.query_table",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::body_query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    request_body = TableQuery,
    responses(
        (status = OK, description = "The rows.", body = TableRows),
        (status = BAD_REQUEST, description = "A filter, sort or summary that does not fit the table's properties (`invalid_filter`, `invalid_sort`, `invalid_summary`), or a bad cursor (`invalid_cursor`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or view, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is not a table (`not_a_table`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<TablePath>,
    extract::Json(request): extract::Json<TableQuery>,
) -> Result<Json<TableRows>> {
    let access = &workspace.access;
    let limit = request.limit.unwrap_or(PAGE);
    if limit == 0 || limit > PAGE {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_cursor")));
    }
    let offset = offset_of(request.cursor.as_deref())?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Read, path.table_id, false).await?;
    let props = tables::properties(&mut tx, table.id).await?;
    let specs = tables::specs(&props)?;

    let (mut resolved, view_type) = match request.view {
        Some(view) => {
            let views = tables::views(&mut tx, table.id).await?;
            let view = views
                .into_iter()
                .find(|candidate| candidate.id == view)
                .ok_or(Error::NotFound)?;
            let view_type: ViewType = view.view_type.parse()?;
            (table_views::resolve(&view.config, &specs)?, Some(view_type))
        }
        None => (table_views::Resolved::default(), None),
    };
    if let Some(raw) = request.filter.as_ref().filter(|raw| !raw.is_null()) {
        let own = filter::parse(raw, &specs).map_err(refused)?;
        resolved.filter = Some(match resolved.filter.take() {
            Some(from_view) => filter::Filter::Group {
                any: false,
                items: vec![from_view, own],
            },
            None => own,
        });
    }
    if let Some(raw) = request.sorts.as_ref() {
        resolved.sorts = filter::parse_sorts(raw, &specs).map_err(refused)?;
    }
    if let Some(raw) = request.summaries.as_ref() {
        resolved.summaries = filter::parse_summaries(raw, &specs).map_err(refused)?;
    }
    let answer = table_query::run(
        &mut tx,
        &Query {
            table: table.id,
            specs: &specs,
            filter: resolved.filter.as_ref(),
            sorts: &resolved.sorts,
            summaries: &resolved.summaries,
            group_by: resolved
                .group_by
                .filter(|_| view_type == Some(ViewType::Board)),
            limit,
            offset,
        },
    )
    .await?;
    let rows = tables::rows(&mut tx, table.id, &answer.ids).await?;
    let seen = offset + rows.len();
    let next_cursor = (i64::try_from(seen).unwrap_or(i64::MAX) < answer.matching
        && !rows.is_empty())
    .then(|| format!("o{seen}"));
    tracked::rows_queried(&state, &mut tx, access.workspace, table.id, view_type, rows.len())
        .await?;
    tx.commit().await?;
    Ok(Json(TableRows {
        rows,
        next_cursor,
        matching: answer.matching,
        summaries: answer.summaries,
        groups: answer.groups,
    }))
}

#[cfg(test)]
mod tests;
