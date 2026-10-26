//! `DELETE /api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}` —
//! remove a view. A table keeps at least one. Sending the answer to
//! `pages.create_view` puts it back.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        PageId, ViewId, expect_live,
        tables::{self, TableViewView, ViewPath},
    },
    events::TableChangeKind,
    scopes::PagesWrite,
};

/// Remove a view.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}",
    operation_id = "pages.remove_view",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.create_view")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("view_id" = ViewId, Path, description = "The view."),
    ),
    responses(
        (status = OK, description = "The view that was removed.", body = TableViewView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or view, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), or the table's last view (`last_view`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ViewPath>,
) -> Result<Json<TableViewView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let views = tables::views(&mut tx, path.table_id).await?;
    let view = views
        .iter()
        .find(|view| view.id == path.view_id)
        .ok_or(Error::NotFound)?;
    if views.len() <= 1 {
        return Err(Error::Conflict(crate::MESSAGES.reason("last_view")));
    }
    let removed = view.view()?;
    sqlx::query("DELETE FROM pages.table_views WHERE id = $1 AND table_id = $2")
        .bind(path.view_id)
        .bind(path.table_id)
        .execute(&mut *tx)
        .await?;
    tables::changed(
        &state,
        &mut tx,
        access,
        path.table_id,
        &[TableChangeKind::Views],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(removed))
}

#[cfg(test)]
mod tests;
