//! `DELETE /api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}`
//! — take a property out of a table. Its cells are kept for 30 days, so
//! `pages.restore_property` brings it back with its values.

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
        PageId, PropertyId, expect_live,
        tables::{self, PropertyPath, PropertyView},
    },
    events::TableChangeKind,
    properties::PropertyType,
    scopes::PagesWrite,
};

/// Remove a property.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}",
    operation_id = "pages.remove_property",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.restore_property")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("property_id" = PropertyId, Path, description = "The property."),
    ),
    responses(
        (status = OK, description = "The property that was removed.", body = PropertyView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or property, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), or the property is the title (`title_property`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PropertyPath>,
) -> Result<Json<PropertyView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let property = tables::property(&mut tx, path.table_id, path.property_id, true).await?;
    if property.removed_at.is_some() {
        return Err(Error::NotFound);
    }
    if property.ty()? == PropertyType::Title {
        return Err(Error::Conflict(crate::MESSAGES.reason("title_property")));
    }
    sqlx::query("UPDATE pages.table_properties SET removed_at = now() WHERE id = $1")
        .bind(path.property_id)
        .execute(&mut *tx)
        .await?;
    tables::changed(&state, &mut tx, access, path.table_id, &[TableChangeKind::Schema]).await?;
    let view = property.view()?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
