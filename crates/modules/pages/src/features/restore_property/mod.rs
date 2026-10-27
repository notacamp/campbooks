//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/restore`
//! — undo the removal of a property, or its last change of type. Everything
//! is put back as it was: the type, the config and every cell the change
//! cleared or converted (a cell edited since keeps the edit).

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
    properties::MAX_PROPERTIES,
    scopes::PagesWrite,
};

/// Restore a property.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/restore",
    operation_id = "pages.restore_property",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.remove_property")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("property_id" = PropertyId, Path, description = "The property."),
    ),
    responses(
        (status = OK, description = "The property as it was.", body = PropertyView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table or property, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), there is nothing to take back (`nothing_to_restore`), or the table would have more than 200 properties (`too_many_properties`).",
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
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    let property = tables::property(&mut tx, path.table_id, path.property_id, true).await?;
    if property.removed_at.is_some() {
        let live: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pages.table_properties
              WHERE table_id = $1 AND removed_at IS NULL",
        )
        .bind(path.table_id)
        .fetch_one(&mut *tx)
        .await?;
        if live >= i64::try_from(MAX_PROPERTIES).unwrap_or(i64::MAX) {
            return Err(Error::Conflict(
                crate::MESSAGES.reason("too_many_properties"),
            ));
        }
        sqlx::query("UPDATE pages.table_properties SET removed_at = NULL WHERE id = $1")
            .bind(path.property_id)
            .execute(&mut *tx)
            .await?;
        // Not removed at Notion yet: it never will be. Removed there: the
        // column has no base any more, so the sync makes it again at Notion
        // and writes its values back.
        sqlx::query(
            "UPDATE pages.external_properties SET remove_confirmed = false WHERE property_id = $1",
        )
        .bind(path.property_id)
        .execute(&mut *tx)
        .await?;
    } else if let (Some(ty), Some(config)) = (&property.previous_type, &property.previous_config) {
        sqlx::query(
            "UPDATE pages.table_properties
                SET type = $2, config = $3, previous_type = NULL, previous_config = NULL,
                    converted_at = NULL
              WHERE id = $1",
        )
        .bind(path.property_id)
        .bind(ty)
        .bind(config)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE pages.row_values
                SET value = previous, previous = NULL, version = version + 1,
                    updated_by = $2, updated_at = now()
              WHERE property_id = $1 AND previous IS NOT NULL",
        )
        .bind(path.property_id)
        .bind(editor)
        .execute(&mut *tx)
        .await?;
    } else {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("nothing_to_restore"),
        ));
    }
    tables::changed(
        &state,
        &mut tx,
        access,
        path.table_id,
        &[TableChangeKind::Schema],
    )
    .await?;
    let row = tables::property(&mut tx, path.table_id, path.property_id, false).await?;
    let view = row.view()?;
    tx.commit().await?;
    Ok(Json(view))
}

#[cfg(test)]
mod tests;
