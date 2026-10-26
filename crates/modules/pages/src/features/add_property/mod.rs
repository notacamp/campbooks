//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/properties` — add a
//! property (a column) to a table.
//!
//! A relation names the table it relates to (this one, or another the caller
//! may read). With `paired_name` it is paired: the other table gets the
//! property that shows the relation from its side, made in the same
//! transaction (which needs the right to write that table). The types
//! reserved for Notion cannot be made.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId, PropertyId, expect_live, position,
        tables::{self, PropertyView, TablePath, refused},
        tracked,
    },
    events::TableChangeKind,
    properties::{MAX_PROPERTIES, PropertyError, PropertyType, clean_name, config},
    scopes::PagesWrite,
};

/// A new property.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewProperty {
    /// Its name: 1 to 100 characters.
    pub name: String,
    /// Its type. `title` and the types reserved for Notion cannot be made.
    #[serde(rename = "type")]
    pub property_type: PropertyType,
    /// Its config, per type (see `PropertyView.config`). A relation needs
    /// `table`; `pair` is made by the server.
    #[serde(default)]
    pub config: Option<Value>,
    /// Where it sits among the table's properties: a fractional-indexing
    /// key. Default: after the last one.
    #[serde(default)]
    pub position: Option<String>,
    /// For a relation: the name of the property that shows it from the other
    /// table's side, which makes it a paired relation.
    #[serde(default)]
    pub paired_name: Option<String>,
}

/// The property added, and for a paired relation the one it made on the
/// other table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AddedProperty {
    /// The property.
    pub property: PropertyView,
    /// Its pair on the other table.
    pub paired: Option<PropertyView>,
}

/// Add a property.
#[allow(
    clippy::too_many_lines,
    reason = "one transaction: the property and its pair"
)]
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/properties",
    operation_id = "pages.add_property",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.remove_property")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    request_body = NewProperty,
    responses(
        (status = CREATED, description = "The property.", body = AddedProperty),
        (status = BAD_REQUEST, description = "A name that is empty or too long (`invalid_property_name`), a type that cannot be made (`property_type_reserved`), a config that does not fit the type (`invalid_config`) or a bad position.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table (or the table it is paired with) but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, tables are off, or a table you cannot see.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), or it would have more than 200 properties (`too_many_properties`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<TablePath>,
    extract::Json(request): extract::Json<NewProperty>,
) -> Result<(StatusCode, Json<AddedProperty>)> {
    let access = &workspace.access;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let ty = request.property_type;
    if !ty.creatable() {
        return Err(refused(PropertyError::RESERVED));
    }
    let name = clean_name(&request.name).map_err(refused)?;
    let raw = request
        .config
        .clone()
        .unwrap_or_else(|| config::default_config(ty));
    let mut config = config::normalise(ty, &raw).map_err(refused)?;
    if ty != PropertyType::Relation && request.paired_name.is_some() {
        return Err(refused(PropertyError::CONFIG));
    }
    if ty == PropertyType::Relation && config.get("pair").is_some() {
        return Err(refused(PropertyError::CONFIG));
    }
    if let Some(key) = &request.position {
        position::check(key)?;
    }

    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;

    let id = PropertyId::new();
    let mut paired: Option<(PageId, PropertyId, String)> = None;
    if ty == PropertyType::Relation {
        let target: PageId = config
            .get("table")
            .and_then(Value::as_str)
            .and_then(|table| table.parse().ok())
            .ok_or_else(|| refused(PropertyError::CONFIG))?;
        let needs = if request.paired_name.is_some() {
            Action::Write
        } else {
            Action::Read
        };
        let related = if target == path.table_id {
            table.clone()
        } else {
            tables::require_table(&state, &mut tx, access, needs, target, true).await?
        };
        expect_live(&related)?;
        if let Some(paired_name) = &request.paired_name {
            let pair = PropertyId::new();
            config["pair"] = json!(pair);
            paired = Some((target, pair, clean_name(paired_name).map_err(refused)?));
        }
    }

    // Both ends count when they are in this table.
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.table_properties WHERE table_id = $1 AND removed_at IS NULL",
    )
    .bind(path.table_id)
    .fetch_one(&mut *tx)
    .await?;
    let adding = 1 + i64::from(
        paired
            .as_ref()
            .is_some_and(|(target, ..)| *target == path.table_id),
    );
    if live + adding > i64::try_from(MAX_PROPERTIES).unwrap_or(i64::MAX) {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("too_many_properties"),
        ));
    }
    let key = match request.position {
        Some(key) => key,
        None => next_position(&mut tx, path.table_id).await?,
    };
    insert(
        &mut tx,
        access.workspace.as_uuid(),
        path.table_id,
        id,
        &name,
        ty,
        &config,
        &key,
        creator,
    )
    .await?;
    let mut paired_view = None;
    if let Some((target, pair, paired_name)) = paired {
        let reverse = json!({"table": path.table_id.to_string(), "pair": id.to_string()});
        let reverse_key = next_position(&mut tx, target).await?;
        insert(
            &mut tx,
            access.workspace.as_uuid(),
            target,
            pair,
            &paired_name,
            PropertyType::Relation,
            &reverse,
            &reverse_key,
            creator,
        )
        .await?;
        let row = tables::property(&mut tx, target, pair, false).await?;
        paired_view = Some(row.view()?);
        if target != path.table_id {
            tables::changed(&state, &mut tx, access, target, &[TableChangeKind::Schema]).await?;
        }
        tracked::property_added(
            &state,
            &mut tx,
            access.workspace,
            target,
            PropertyType::Relation,
        )
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
    tracked::property_added(&state, &mut tx, access.workspace, path.table_id, ty).await?;
    let row = tables::property(&mut tx, path.table_id, id, false).await?;
    let property = row.view()?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(AddedProperty {
            property,
            paired: paired_view,
        }),
    ))
}

/// The key after the table's last property (removed ones count: a restored
/// property keeps its place).
async fn next_position(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: PageId,
) -> Result<String> {
    let last: Option<String> =
        sqlx::query_scalar("SELECT max(position) FROM pages.table_properties WHERE table_id = $1")
            .bind(table)
            .fetch_one(&mut **tx)
            .await?;
    Ok(position::after(last.as_deref()))
}

#[allow(clippy::too_many_arguments)]
async fn insert(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: &uuid::Uuid,
    table: PageId,
    id: PropertyId,
    name: &str,
    ty: PropertyType,
    config: &Value,
    position: &str,
    creator: campbooks_kernel::tenancy::UserId,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pages.table_properties
             (id, table_id, workspace_id, name, type, config, position, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(table)
    .bind(workspace)
    .bind(name)
    .bind(ty.as_str())
    .bind(config)
    .bind(position)
    .bind(creator)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
