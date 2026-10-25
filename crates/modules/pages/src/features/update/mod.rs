//! `PATCH /api/v1/w/{workspace_id}/pages/{page_id}` — change a page's title
//! and icon, both at once: what is sent is what the page becomes, so an
//! undo is the same call with the values it had.

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
        PageId, PageRow, PageSummary, SELECT_ONE, access, announce, clean_icon, clean_title,
        expect_live, expect_meta_revision, tracked, view,
    },
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// The page as it should become.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PageUpdate {
    /// The `meta_revision` you saw.
    pub meta_revision: i64,
    /// Its title: 0 to 200 characters.
    pub title: String,
    /// Its icon, one of the page icons; none clears it.
    #[serde(default)]
    pub icon: Option<String>,
}

/// Change a page's title or icon.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}",
    operation_id = "pages.update",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.update")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = PageUpdate,
    responses(
        (status = OK, description = "The page afterwards.", body = PageSummary),
        (status = BAD_REQUEST, description = "A title over 200 characters (`invalid_title`) or an unknown icon (`invalid_icon`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not write it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page changed since you saw it (`page_changed`), or it is in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<PageUpdate>,
) -> Result<Json<PageSummary>> {
    let access = &workspace.access;
    let title = clean_title(&request.title)?;
    let icon = clean_icon(request.icon.as_deref())?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    expect_live(&row)?;
    expect_meta_revision(&row, request.meta_revision)?;
    let (title_changed, icon_changed) = (row.title != title, row.icon != icon);
    if title_changed || icon_changed {
        let Some(editor) = access.user else {
            return Err(campbooks_kernel::Error::Forbidden);
        };
        sqlx::query(
            "UPDATE pages.pages
                SET title = $2, icon = $3, meta_revision = meta_revision + 1,
                    revision = revision + 1, edited_by = $4, edited_at = now()
              WHERE id = $1",
        )
        .bind(path.page_id)
        .bind(&title)
        .bind(icon)
        .bind(editor)
        .execute(&mut *tx)
        .await?;
        announce(
            &state,
            &mut tx,
            access,
            path.page_id,
            vec![PageChangeKind::Meta],
        )
        .await?;
        tracked::edited(
            &state,
            &mut tx,
            access.workspace,
            path.page_id,
            title_changed,
            icon_changed,
        )
        .await?;
    }
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    let summary = view::summary_of(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

#[cfg(test)]
mod tests;
