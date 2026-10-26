//! `PATCH /api/v1/w/{workspace_id}/tables/{table_id}/rows/{row_id}/cells` —
//! write cells of a row. This is also how a card moves on a board: setting
//! the value of the property the board groups by.
//!
//! Each value is checked against its property's type. Each cell has its own
//! version, so two people writing two cells of a row never collide; a write
//! to a cell that changed since you read it still goes through and answers
//! the value it replaced, which is what an undo writes back.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId, cells, expect_live,
        tables::{self, RowPath, RowView},
    },
    scopes::PagesWrite,
};

/// The cells to write.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CellsUpdate {
    /// Up to 100 cells, each property once.
    pub cells: Vec<cells::CellInput>,
}

/// What was written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CellsWritten {
    /// For each cell: its new version, whether it was stale, and what it
    /// held before.
    pub results: Vec<cells::CellResult>,
    /// The row afterwards.
    pub row: RowView,
}

/// Write cells.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/rows/{row_id}/cells",
    operation_id = "pages.set_cells",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.set_cells")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("row_id" = PageId, Path, description = "The row."),
    ),
    request_body = CellsUpdate,
    responses(
        (status = OK, description = "What each cell was and became, and the row.", body = CellsWritten),
        (status = BAD_REQUEST, description = "A value that does not fit its property (`invalid_value`; nothing is stored), an unknown property (`unknown_property`), a computed or reserved one (`property_not_editable`), someone who is not a member (`not_a_member`), a file not uploaded to the row (`foreign_blob`) or more than 100 cells.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the row (or a row a paired relation changes) but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such row, tables are off, or you cannot see it (or a row a relation cell names).",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a row (`not_a_row`) or in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<RowPath>,
    extract::Json(request): extract::Json<CellsUpdate>,
) -> Result<Json<CellsWritten>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = tables::require_row(
        &state,
        &mut tx,
        access,
        Action::Write,
        path.table_id,
        path.row_id,
        true,
    )
    .await?;
    expect_live(&row)?;
    let properties = tables::properties(&mut tx, path.table_id).await?;
    let results = cells::write(
        &state,
        &mut tx,
        access,
        &row,
        path.table_id,
        &properties,
        &request.cells,
    )
    .await?;
    let row = tables::rows(&mut tx, path.table_id, &[path.row_id])
        .await?
        .pop()
        .ok_or(campbooks_kernel::Error::NotFound)?;
    tx.commit().await?;
    Ok(Json(CellsWritten { results, row }))
}

#[cfg(test)]
mod tests;
