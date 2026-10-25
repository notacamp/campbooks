//! `GET /api/v1/w/{workspace_id}/pages/{page_id}` — one page: its details,
//! who it is shared with, and its body as a Yjs document.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;

use crate::{
    domain::{PageId, PageView, access, view},
    scopes::PagesRead,
};

/// `/api/v1/w/{workspace_id}/pages/{page_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PagePath {
    /// The page.
    pub page_id: PageId,
}

/// Read a page.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}",
    operation_id = "pages.get",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    responses(
        (status = OK, description = "The page, with its body as one Yjs update and its state vector.", body = PageView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<Json<PageView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Read, path.page_id, false).await?;
    let page = view::page_view(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok(Json(page))
}

#[cfg(test)]
mod tests;
