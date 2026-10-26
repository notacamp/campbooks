//! `GET /api/v1/w/{workspace_id}/pages` — the pages the caller may open, as
//! a flat list the client makes a tree of. A page under one the caller may
//! not open is listed all the same: it is a root of its own for them.

use std::collections::HashSet;

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{PageId, PageRow, PageSummary, Place, columns, resources::PAGE_RECORD, roles},
    scopes::PagesRead,
};

/// Every page but the rows of tables, which follow their table. While
/// tables are off, the tables are left out too.
const SELECT_ALL: &str = concat!(
    "SELECT ",
    columns!(),
    " FROM pages.pages WHERE kind <> 'row' AND ($1 OR kind = 'page')"
);

/// What to list.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// `true` for the trash: the pages deleted in the last 30 days, each the
    /// root of what was deleted with it. Default `false`.
    #[serde(default)]
    pub trashed: bool,
    /// Only the sub-pages of this page.
    pub parent: Option<PageId>,
    /// Only the pages in this place.
    #[param(inline)]
    pub place: Option<Place>,
}

/// The pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageList {
    /// The pages, sorted by position among their siblings.
    pub pages: Vec<PageSummary>,
    /// Whether tables are on here: when not, there are no table routes, and
    /// a client offers none.
    pub tables: bool,
}

/// List the pages the caller may open.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages",
    operation_id = "pages.list",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListParams),
    responses(
        (status = OK, description = "The pages the caller may open.", body = PageList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a token acting for nobody.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    Extension(tables): Extension<crate::config::TablesOn>,
    extract::Query(params): extract::Query<ListParams>,
) -> Result<Json<PageList>> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Read, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rows: Vec<PageRow> = sqlx::query_as(SELECT_ALL)
        .bind(tables.0)
        .fetch_all(&mut *tx)
        .await?;
    let readable = roles::readable(&mut tx, access).await?;
    tx.commit().await?;
    let with_children: HashSet<PageId> = rows
        .iter()
        .filter(|row| !row.is_trashed())
        .filter_map(|row| row.parent_id)
        .collect();
    let mut shown: Vec<&PageRow> = rows
        .iter()
        .filter(|row| readable.contains_key(&row.id))
        .filter(|row| {
            if params.trashed {
                row.trash_root == Some(row.id)
            } else {
                !row.is_trashed()
            }
        })
        .filter(|row| {
            params
                .parent
                .is_none_or(|parent| row.parent_id == Some(parent))
        })
        .collect();
    if let Some(place) = params.place {
        shown.retain(|row| row.place == place.as_str());
    }
    shown.sort_by(|a, b| a.position.cmp(&b.position).then(a.id.cmp(&b.id)));
    let pages = shown
        .into_iter()
        .filter_map(|row| readable.get(&row.id).map(|role| (row, *role)))
        .map(|(row, role)| row.summary(role, with_children.contains(&row.id)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(PageList {
        pages,
        tables: tables.0,
    }))
}

#[cfg(test)]
mod tests;
