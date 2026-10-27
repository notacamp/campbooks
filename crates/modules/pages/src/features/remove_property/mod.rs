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
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    domain::{
        PageId, PropertyId, expect_live,
        tables::{self, PropertyPath, PropertyView},
    },
    events::TableChangeKind,
    properties::PropertyType,
    scopes::PagesWrite,
};

/// How a removal is asked for.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RemoveParams {
    /// For a table kept in step with a Notion database: the member confirms
    /// that the column is removed in Notion too. Without it, removing a
    /// column of such a table is refused (`confirmation_required`) and
    /// nothing changes anywhere.
    #[serde(default)]
    pub confirm_remote_delete: bool,
}

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
        RemoveParams,
    ),
    responses(
        (status = OK, description = "The property that was removed.", body = PropertyView),
        (status = BAD_REQUEST, description = "The table is kept in step with a Notion database, the column is Notion's too, and the removal was not confirmed with `confirm_remote_delete` (`confirmation_required`).",
            body = Problem, content_type = "application/problem+json"),
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
    extract::Query(params): extract::Query<RemoveParams>,
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
    // A column that is Notion's too is removed there as well, and only when
    // the member said so: otherwise nothing changes, here or there.
    let synced: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.external_properties p
                          JOIN pages.external_tables t ON t.table_id = p.table_id
                         WHERE p.property_id = $1 AND t.state = 'syncing')",
    )
    .bind(path.property_id)
    .fetch_one(&mut *tx)
    .await?;
    if synced && !params.confirm_remote_delete {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("confirmation_required"),
        ));
    }
    sqlx::query("UPDATE pages.table_properties SET removed_at = now() WHERE id = $1")
        .bind(path.property_id)
        .execute(&mut *tx)
        .await?;
    if synced {
        sqlx::query(
            "UPDATE pages.external_properties SET remove_confirmed = true WHERE property_id = $1",
        )
        .bind(path.property_id)
        .execute(&mut *tx)
        .await?;
    }
    tables::changed(
        &state,
        &mut tx,
        access,
        path.table_id,
        &[TableChangeKind::Schema],
    )
    .await?;
    let view = property.view()?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
