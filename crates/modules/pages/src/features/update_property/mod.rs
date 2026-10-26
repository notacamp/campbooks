//! `PATCH /api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}`
//! — rename a property, move it, change its config or change its type.
//!
//! A change of type or config converts the values it can (the properties
//! library) and clears those it cannot, **keeping what every changed cell
//! held** for 30 days: `pages.restore_property` puts them back.

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
        PageId, PropertyId, expect_live, position,
        tables::{self, PropertyPath, PropertyView, refused},
    },
    events::TableChangeKind,
    properties::{PropertyError, PropertyType, clean_name, config, convert},
    scopes::PagesWrite,
};

/// What to change. What is absent stays.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PropertyUpdate {
    /// Its new name: 1 to 100 characters.
    #[serde(default)]
    pub name: Option<String>,
    /// Its new type. The values that fit stay (as text, numbers, options),
    /// the rest are cleared and kept for 30 days.
    #[serde(default, rename = "type")]
    pub property_type: Option<PropertyType>,
    /// Its new config. With a new type and no config, the type's default.
    #[serde(default)]
    pub config: Option<Value>,
    /// Its new place among the table's properties: a fractional-indexing key.
    #[serde(default)]
    pub position: Option<String>,
}

/// Change a property.
#[allow(
    clippy::too_many_lines,
    reason = "one transaction: the property and its cells"
)]
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}",
    operation_id = "pages.update_property",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.update_property")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("property_id" = PropertyId, Path, description = "The property."),
    ),
    request_body = PropertyUpdate,
    responses(
        (status = OK, description = "The property afterwards. `can_revert` says whether `pages.restore_property` can take a change of type back.", body = PropertyView),
        (status = BAD_REQUEST, description = "A name that is empty or too long, a config that does not fit the type (`invalid_config`), a type that cannot be made (`property_type_reserved`), or a change the title or a relation does not take (`property_not_editable`).",
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
    extract::Json(request): extract::Json<PropertyUpdate>,
) -> Result<Json<PropertyView>> {
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
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    let property = tables::property(&mut tx, path.table_id, path.property_id, true).await?;
    if property.removed_at.is_some() {
        return Err(Error::NotFound);
    }
    let from = property.spec()?;
    let to = request.property_type.unwrap_or(from.ty);
    let retyped = to != from.ty;
    let reconfigured = request.config.is_some();

    if retyped || reconfigured {
        if matches!(from.ty, PropertyType::Title | PropertyType::Relation) {
            return Err(refused(PropertyError::NOT_EDITABLE));
        }
        if !to.creatable() || to == PropertyType::Relation {
            return Err(refused(if to.reserved() || to == PropertyType::Title {
                PropertyError::RESERVED
            } else {
                PropertyError::CONFIG
            }));
        }
    }
    let new_config = match (&request.config, retyped) {
        (Some(raw), _) => config::normalise(to, raw).map_err(refused)?,
        (None, true) => config::default_config(to),
        (None, false) => property.config.clone(),
    };

    // The cells, converted when the type or the config of a stored property
    // changes.
    let mut converted = 0_usize;
    let mut final_config = new_config.clone();
    if (retyped || new_config != property.config) && from.ty.editable() {
        if !to.editable() {
            return Err(refused(PropertyError::CONFIG));
        }
        let cells: Vec<(PageId, Value)> = sqlx::query_as(
            "SELECT row_id, value FROM pages.row_values
              WHERE property_id = $1 AND value IS NOT NULL AND value <> 'null'::jsonb
              FOR UPDATE",
        )
        .bind(path.property_id)
        .fetch_all(&mut *tx)
        .await?;
        let plan = convert::plan(&from, to, Some(&new_config), &cells).map_err(refused)?;
        final_config = plan.config.clone();
        sqlx::query("UPDATE pages.row_values SET previous = NULL WHERE property_id = $1")
            .bind(path.property_id)
            .execute(&mut *tx)
            .await?;
        let (mut rows, mut values) = (Vec::new(), Vec::new());
        for ((row, original), (_, new)) in cells.iter().zip(&plan.values) {
            let new = new.clone().unwrap_or(Value::Null);
            if &new != original {
                rows.push(*row);
                values.push(new);
            }
        }
        converted = rows.len();
        if !rows.is_empty() {
            sqlx::query(
                "UPDATE pages.row_values v
                    SET previous = v.value, value = u.value, version = v.version + 1,
                        updated_by = $3, updated_at = now()
                   FROM unnest($1::uuid[], $2::jsonb[]) AS u(row_id, value)
                  WHERE v.property_id = $4 AND v.row_id = u.row_id",
            )
            .bind(&rows)
            .bind(&values)
            .bind(editor)
            .bind(path.property_id)
            .execute(&mut *tx)
            .await?;
        }
    } else if reconfigured && !from.ty.stored() {
        // A computed property has no cells: only its config.
        final_config = new_config.clone();
    }

    let changed_shape = retyped || final_config != property.config;
    sqlx::query(
        "UPDATE pages.table_properties
            SET name = COALESCE($3, name),
                position = COALESCE($4, position),
                type = $5, config = $6,
                previous_type = CASE WHEN $7 THEN $8 ELSE previous_type END,
                previous_config = CASE WHEN $7 THEN $9 ELSE previous_config END,
                converted_at = CASE WHEN $7 THEN now() ELSE converted_at END
          WHERE id = $1 AND table_id = $2",
    )
    .bind(path.property_id)
    .bind(path.table_id)
    .bind(name)
    .bind(request.position)
    .bind(to.as_str())
    .bind(&final_config)
    .bind(changed_shape && from.ty.editable() && to.editable())
    .bind(from.ty.as_str())
    .bind(&property.config)
    .execute(&mut *tx)
    .await?;
    tracing::debug!(converted, "property changed");
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
