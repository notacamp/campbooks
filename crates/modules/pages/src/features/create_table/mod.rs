//! `POST /api/v1/w/{workspace_id}/tables` — a member makes a table: a page
//! whose rows are pages with typed properties. It starts with its title
//! property and one table view, and no rows.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        Kind, MAX_DEPTH, PageId, PageRow, Place, SELECT_ONE, ViewId, access, announce, clean_icon,
        clean_title, expect_live, position,
        resources::PAGE_RECORD,
        roles,
        table_views::ViewType,
        tables::{self, NewPageRow, TableDetail},
        tracked,
    },
    events::PageChangeKind,
    properties::{PropertyType, clean_name},
    scopes::PagesWrite,
};

/// A new table.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTable {
    /// Its title: 0 to 200 characters.
    #[serde(default)]
    pub title: Option<String>,
    /// Its icon, one of the page icons.
    #[serde(default)]
    pub icon: Option<String>,
    /// The page it goes under; none for the top level of `place`. A table
    /// goes under an ordinary page, never under a table or a row.
    #[serde(default)]
    pub parent: Option<PageId>,
    /// `private` (the default) or `workspace`, for a top-level table. A table
    /// under a page is always in its parent's place.
    #[serde(default)]
    pub place: Option<Place>,
    /// Where it sits among its siblings: a fractional-indexing key. Default:
    /// after the last one.
    #[serde(default)]
    pub position: Option<String>,
    /// The name of its title property, in the member's language. Default
    /// `Name`.
    #[serde(default)]
    pub title_property: Option<String>,
    /// The name of its first view, in the member's language. Default
    /// `Table`.
    #[serde(default)]
    pub view_name: Option<String>,
}

/// Make a table.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables",
    operation_id = "pages.create_table",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.trash")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = NewTable,
    responses(
        (status = CREATED, description = "The table, with its title property and one table view.", body = TableDetail),
        (status = BAD_REQUEST, description = "A title over 200 characters (`invalid_title`), an unknown icon, a bad position or a name that is empty or too long (`invalid_property_name`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the parent but not write in it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, tables are off, or a parent you cannot see.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The parent is in the trash (`page_trashed`), is a table or a row (`table_parent`), or pages would nest deeper than 12 levels (`page_too_deep`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Json(request): extract::Json<NewTable>,
) -> Result<(StatusCode, Json<TableDetail>)> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Create, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let title = clean_title(request.title.as_deref().unwrap_or_default())?;
    let icon = clean_icon(request.icon.as_deref())?;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let property_name =
        clean_name(request.title_property.as_deref().unwrap_or("Name")).map_err(tables::refused)?;
    let view_name =
        clean_name(request.view_name.as_deref().unwrap_or("Table")).map_err(tables::refused)?;

    let mut tx = state.db.begin_for(access.workspace).await?;
    let (parent, place) = match request.parent {
        Some(parent_id) => {
            let parent =
                access::require_page(&state, &mut tx, access, Action::Write, parent_id, false)
                    .await?;
            expect_live(&parent)?;
            if parent.kind()? != Kind::Page {
                return Err(Error::Conflict(crate::MESSAGES.reason("table_parent")));
            }
            let place = parent.place()?;
            if request.place.is_some_and(|asked| asked != place) {
                return Err(Error::BadRequest(crate::MESSAGES.reason("place_mismatch")));
            }
            let above = roles::chain(&mut tx, parent_id).await?;
            if above.len() >= MAX_DEPTH {
                return Err(Error::Conflict(crate::MESSAGES.reason("page_too_deep")));
            }
            (Some(parent_id), place)
        }
        None => (None, request.place.unwrap_or(Place::Private)),
    };
    let key = match request.position {
        Some(key) => key,
        None => position::next(&mut tx, parent, place, creator).await?,
    };
    let id = PageId::new();
    tables::insert_page(
        &mut tx,
        NewPageRow {
            id,
            workspace: access.workspace,
            parent,
            place,
            position: &key,
            title: &title,
            icon: icon.as_deref(),
            creator,
            kind: Kind::Table,
        },
    )
    .await?;
    sqlx::query("INSERT INTO pages.tables (table_id, workspace_id) VALUES ($1, $2)")
        .bind(id)
        .bind(access.workspace.as_uuid())
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO pages.table_properties (id, table_id, workspace_id, name, type, config, position, created_by)
         VALUES ($1, $2, $3, $4, $5, '{}', $6, $7)",
    )
    .bind(crate::domain::PropertyId::new())
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(&property_name)
    .bind(PropertyType::Title.as_str())
    .bind(position::after(None))
    .bind(creator)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO pages.table_views (id, table_id, workspace_id, name, type, position, config, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, '{}', $7)",
    )
    .bind(ViewId::new())
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(&view_name)
    .bind(ViewType::Table.as_str())
    .bind(position::after(None))
    .bind(creator)
    .execute(&mut *tx)
    .await?;
    announce(
        &state,
        &mut tx,
        access,
        id,
        vec![PageChangeKind::Meta, PageChangeKind::Schema],
    )
    .await?;
    tracked::simple(
        &state,
        &mut tx,
        access.workspace,
        id,
        tracked::Change::Created,
    )
    .await?;
    tracked::table_created(&state, &mut tx, access.workspace, id).await?;
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let detail = tables::detail(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[cfg(test)]
mod tests;
