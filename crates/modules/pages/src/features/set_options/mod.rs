//! `PUT /api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/options`
//! — set the options of a select, multi-select or status property: add,
//! rename, recolour, reorder and remove them, by id.
//!
//! Values hold option ids, so **renaming or recolouring an option changes
//! nothing in the rows**: every row that held it shows the new name. An
//! option left out of the list is removed; the cells that held it keep its
//! id and show nothing, and sending the option again (with the same id)
//! brings them back, which is how an undo works.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId, PropertyId, expect_live,
        tables::{self, PropertyPath, PropertyView, refused},
    },
    events::TableChangeKind,
    properties::config,
    scopes::PagesWrite,
};

/// The options as they should become.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OptionsUpdate {
    /// All the options, in order. An option with an `id` is that option
    /// (renamed, recoloured, moved); one without is new. At most 100, with
    /// distinct names.
    pub options: Vec<config::SelectOption>,
}

/// Set the options of a property.
#[utoipa::path(
    put,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/options",
    operation_id = "pages.set_options",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.set_options")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("property_id" = PropertyId, Path, description = "The property."),
    ),
    request_body = OptionsUpdate,
    responses(
        (status = OK, description = "The property with its options.", body = PropertyView),
        (status = BAD_REQUEST, description = "Options that do not fit (`invalid_config`): a repeated id or name, an unknown colour, more than 100, or a property that holds no options.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or property, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`) or in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PropertyPath>,
    extract::Json(request): extract::Json<OptionsUpdate>,
) -> Result<Json<PropertyView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let property = tables::property(&mut tx, path.table_id, path.property_id, true).await?;
    if property.removed_at.is_some() {
        return Err(campbooks_kernel::Error::NotFound);
    }
    let ty = property.ty()?;
    let new = config::with_options(ty, &request.options).map_err(refused)?;
    if new == property.config {
        let view = property.view()?;
        tx.commit().await?;
        return Ok(Json(view));
    }
    sqlx::query("UPDATE pages.table_properties SET config = $2 WHERE id = $1")
        .bind(path.property_id)
        .bind(&new)
        .execute(&mut *tx)
        .await?;
    tables::changed(&state, &mut tx, access, path.table_id, &[TableChangeKind::Schema]).await?;
    let row = tables::property(&mut tx, path.table_id, path.property_id, false).await?;
    let view = row.view()?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
