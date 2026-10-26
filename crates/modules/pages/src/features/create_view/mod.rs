//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/views` — add a view to a
//! table: a table (grid) or a board (cards in a column per option).
//!
//! `list`, `calendar` and `gallery` are reserved and refused. Sending the
//! `id` of a view that was removed, with its config, puts it back: that is
//! the undo of `pages.remove_view`.

use axum::{Json, extract::State, http::StatusCode};
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
        table_views::{self, MAX_VIEWS, ViewType},
        tables::{self, TablePath, TableViewView, ViewRow, refused},
        tracked,
    },
    events::TableChangeKind,
    properties::clean_name,
    scopes::PagesWrite,
};

/// A new view.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewView {
    /// A view that was removed, to put back with the same id.
    #[serde(default)]
    pub id: Option<ViewId>,
    /// Its name: 1 to 100 characters.
    pub name: String,
    /// `table` or `board`.
    #[serde(rename = "type")]
    pub view_type: ViewType,
    /// Its filter, sorts, summaries and columns; a board needs `group_by`.
    #[serde(default)]
    pub config: Option<Value>,
    /// Where it sits among the views: a fractional-indexing key. Default:
    /// after the last one.
    #[serde(default)]
    pub position: Option<String>,
}

/// Add a view.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/views",
    operation_id = "pages.create_view",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.remove_view")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    request_body = NewView,
    responses(
        (status = CREATED, description = "The view.", body = TableViewView),
        (status = BAD_REQUEST, description = "A name that is empty or too long, a reserved type (`view_type_reserved`), a config that does not fit the table (`invalid_view`, `invalid_filter`, `invalid_sort`, `invalid_summary`) or a bad position.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, tables are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), the id is taken (`view_exists`) or the table has 50 views (`too_many_views`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<TablePath>,
    extract::Json(request): extract::Json<NewView>,
) -> Result<(StatusCode, Json<TableViewView>)> {
    let access = &workspace.access;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let name = clean_name(&request.name).map_err(refused)?;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let props = tables::properties(&mut tx, path.table_id).await?;
    let specs = tables::specs(&props)?;
    let config = table_views::normalise(
        request.view_type,
        request.config.as_ref().unwrap_or(&Value::Null),
        &specs,
    )?;
    let existing = tables::views(&mut tx, path.table_id).await?;
    if existing.len() >= MAX_VIEWS {
        return Err(Error::Conflict(crate::MESSAGES.reason("too_many_views")));
    }
    let id = request.id.unwrap_or_else(ViewId::new);
    if request.id.is_some() {
        let taken: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pages.table_views WHERE id = $1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if taken {
            return Err(Error::Conflict(crate::MESSAGES.reason("view_exists")));
        }
    }
    let key = match request.position {
        Some(key) => key,
        None => position::after(existing.last().map(|view| view.position.as_str())),
    };
    sqlx::query(
        "INSERT INTO pages.table_views
             (id, table_id, workspace_id, name, type, position, config, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(path.table_id)
    .bind(access.workspace.as_uuid())
    .bind(&name)
    .bind(request.view_type.as_str())
    .bind(&key)
    .bind(&config)
    .bind(creator)
    .execute(&mut *tx)
    .await?;
    tables::changed(&state, &mut tx, access, path.table_id, &[TableChangeKind::Views]).await?;
    tracked::view_created(&state, &mut tx, access.workspace, path.table_id, request.view_type)
        .await?;
    let made: ViewRow = tables::views(&mut tx, path.table_id)
        .await?
        .into_iter()
        .find(|view| view.id == id)
        .ok_or_else(|| Error::internal("a view just made is not there"))?;
    let view = made.view()?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(view)))
}

#[cfg(test)]
mod tests;
