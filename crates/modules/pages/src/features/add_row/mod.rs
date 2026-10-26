//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/rows` — add a row to a
//! table, with its title and, if you have them, its cells.
//!
//! A row is a page of kind `row` under the table, with a number in the
//! table. Move, trash and restore it with the page operations.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        Kind, PageId, PageRow, SELECT_ONE, announce, cells, clean_title, expect_live, position,
        tables::{self, MAX_ROWS, NewPageRow, RowView, TablePath},
        tracked,
    },
    events::{PageChangeKind, TableChangeKind},
    scopes::PagesWrite,
};

/// A new row.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRow {
    /// Its title (the table's title property): 0 to 200 characters.
    #[serde(default)]
    pub title: Option<String>,
    /// Its cells.
    #[serde(default)]
    pub cells: Vec<cells::CellInput>,
    /// Where it sits among the table's rows: a fractional-indexing key.
    /// Default: after the last one.
    #[serde(default)]
    pub position: Option<String>,
}

/// Add a row.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/rows",
    operation_id = "pages.add_row",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.trash")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    request_body = NewRow,
    responses(
        (status = CREATED, description = "The row.", body = RowView),
        (status = BAD_REQUEST, description = "A title over 200 characters, a bad position, a cell that is not valid (`invalid_value`, `unknown_property`, `property_not_editable`, `not_a_member`, `foreign_blob`) or more than 100 cells.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, tables are off, or you cannot see it (or a row a relation cell names).",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), or it already has 20,000 rows (`table_full`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<TablePath>,
    extract::Json(request): extract::Json<NewRow>,
) -> Result<(StatusCode, Json<RowView>)> {
    let access = &workspace.access;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let title = clean_title(request.title.as_deref().unwrap_or_default())?;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    // The table is locked for the number the row takes.
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.table_rows WHERE table_id = $1")
        .bind(path.table_id)
        .fetch_one(&mut *tx)
        .await?;
    if held >= MAX_ROWS {
        return Err(Error::Conflict(crate::MESSAGES.reason("table_full")));
    }
    let number: i64 = sqlx::query_scalar(
        "UPDATE pages.tables SET next_number = next_number + 1 WHERE table_id = $1
         RETURNING next_number - 1",
    )
    .bind(path.table_id)
    .fetch_one(&mut *tx)
    .await?;
    let place = table.place()?;
    let key = match request.position {
        Some(key) => key,
        None => position::next(&mut tx, Some(path.table_id), place, creator).await?,
    };
    let id = PageId::new();
    tables::insert_page(
        &mut tx,
        NewPageRow {
            id,
            workspace: access.workspace,
            parent: Some(path.table_id),
            place,
            position: &key,
            title: &title,
            icon: None,
            creator,
            kind: Kind::Row,
        },
    )
    .await?;
    sqlx::query(
        "INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(path.table_id)
    .bind(access.workspace.as_uuid())
    .bind(number)
    .execute(&mut *tx)
    .await?;
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    announce(&state, &mut tx, access, id, vec![PageChangeKind::Meta]).await?;
    tracked::simple(
        &state,
        &mut tx,
        access.workspace,
        id,
        tracked::Change::Created,
    )
    .await?;
    if !request.cells.is_empty() {
        let properties = tables::properties(&mut tx, path.table_id).await?;
        cells::write(
            &state,
            &mut tx,
            access,
            &row,
            path.table_id,
            &properties,
            &request.cells,
        )
        .await?;
    }
    tables::changed(
        &state,
        &mut tx,
        access,
        path.table_id,
        &[TableChangeKind::Rows],
    )
    .await?;
    let made = tables::rows(&mut tx, path.table_id, &[id])
        .await?
        .pop()
        .ok_or_else(|| Error::internal("a row just made is not there"))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(made)))
}

#[cfg(test)]
mod tests;
