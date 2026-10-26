//! `GET /api/v1/w/{workspace_id}/tables/{table_id}` — a table's properties
//! and views.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        PageId,
        tables::{self, TableDetail, TablePath},
    },
    scopes::PagesRead,
};

/// Read a table: its properties, its views and how many rows it has.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}",
    operation_id = "pages.get_table",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    responses(
        (status = OK, description = "The table.", body = TableDetail),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is not a table (`not_a_table`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<TablePath>,
) -> Result<Json<TableDetail>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Read, path.table_id, false).await?;
    let detail = tables::detail(&mut tx, access, &table).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

#[cfg(test)]
mod tests;
