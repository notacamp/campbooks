//! `PATCH /api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}` —
//! rename a view, move it, or change its filter, sorts, summaries and
//! columns. A view keeps its type.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId, ViewId, expect_live, position,
        table_views::{self, ViewType},
        tables::{self, TableViewView, ViewPath, refused},
    },
    events::TableChangeKind,
    properties::clean_name,
    scopes::PagesWrite,
};

/// What to change. What is absent stays.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewUpdate {
    /// Its new name: 1 to 100 characters.
    #[serde(default)]
    pub name: Option<String>,
    /// Its new config, whole: what is sent is what the view becomes.
    #[serde(default)]
    pub config: Option<Value>,
    /// Its new place among the views.
    #[serde(default)]
    pub position: Option<String>,
}

/// Change a view.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}",
    operation_id = "pages.update_view",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.update_view")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("view_id" = ViewId, Path, description = "The view."),
    ),
    request_body = ViewUpdate,
    responses(
        (status = OK, description = "The view afterwards.", body = TableViewView),
        (status = BAD_REQUEST, description = "A name that is empty or too long, a config that does not fit the table (`invalid_view`, `invalid_filter`, `invalid_sort`, `invalid_summary`) or a bad position.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or view, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`) or in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ViewPath>,
    extract::Json(request): extract::Json<ViewUpdate>,
) -> Result<Json<TableViewView>> {
    let access = &workspace.access;
    let name = request
        .name
        .as_deref()
        .map(clean_name)
        .transpose()
        .map_err(refused)?;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let view = tables::views(&mut tx, path.table_id)
        .await?
        .into_iter()
        .find(|view| view.id == path.view_id)
        .ok_or(Error::NotFound)?;
    let config = match &request.config {
        Some(raw) => {
            let props = tables::properties(&mut tx, path.table_id).await?;
            let specs = tables::specs(&props)?;
            let ty: ViewType = view.view_type.parse()?;
            Some(table_views::normalise(ty, raw, &specs)?)
        }
        None => None,
    };
    sqlx::query(
        "UPDATE pages.table_views
            SET name = COALESCE($3, name), position = COALESCE($4, position),
                config = COALESCE($5, config)
          WHERE id = $1 AND table_id = $2",
    )
    .bind(path.view_id)
    .bind(path.table_id)
    .bind(name)
    .bind(request.position)
    .bind(config)
    .execute(&mut *tx)
    .await?;
    tables::changed(&state, &mut tx, access, path.table_id, &[TableChangeKind::Views]).await?;
    let updated = tables::views(&mut tx, path.table_id)
        .await?
        .into_iter()
        .find(|view| view.id == path.view_id)
        .ok_or(Error::NotFound)?
        .view()?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[cfg(test)]
mod tests;
