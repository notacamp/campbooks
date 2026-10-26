//! `POST …/pages/{page_id}/trash`, `POST …/pages/{page_id}/restore` and
//! `DELETE …/pages/{page_id}` — put a page and its sub-pages in the trash for
//! 30 days, bring them back, or delete them for good.
//!
//! Trashing and restoring act on everything that went with the page, as one.
//! Deleting for good only acts on a page in the trash that was deleted
//! itself (a trash root), and has no undo. All three need the right to
//! delete the page.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        PageId, PageRow, PageSummary, Place, SELECT_ONE, access, announce, notion, position, purge,
        tracked, view,
    },
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// Put a page and its sub-pages in the trash.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/trash",
    operation_id = "pages.trash",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.restore")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    responses(
        (status = OK, description = "The page, now in the trash for 30 days.", body = PageSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not delete it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash already (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn trash(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<Json<PageSummary>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Delete, path.page_id, true).await?;
    let Some(member) = access.user else {
        return Err(Error::Forbidden);
    };
    if row.is_trashed() {
        return Err(Error::Conflict(crate::MESSAGES.reason("page_trashed")));
    }
    let gone: Vec<PageId> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS (
             SELECT id, 1 AS depth FROM pages.pages WHERE id = $1
             UNION ALL
             SELECT p.id, s.depth + 1 FROM pages.pages p JOIN sub s ON p.parent_id = s.id
              WHERE s.depth < 14
         )
         UPDATE pages.pages
            SET trashed_at = now(), trashed_by = $2, trash_root = $1, revision = revision + 1
          WHERE id IN (SELECT id FROM sub) AND trashed_at IS NULL
      RETURNING id",
    )
    .bind(path.page_id)
    .bind(member)
    .fetch_all(&mut *tx)
    .await?;
    // A page in the trash is not kept in step with Notion any more; nothing
    // is deleted there.
    for id in &gone {
        notion::stop(
            &state,
            &mut tx,
            access.workspace,
            *id,
            notion::StopReason::TrashedHere,
        )
        .await?;
    }
    if !gone.is_empty() {
        tracked::simple(
            &state,
            &mut tx,
            access.workspace,
            path.page_id,
            tracked::Change::Trashed,
        )
        .await?;
    }
    announce(
        &state,
        &mut tx,
        access,
        path.page_id,
        vec![PageChangeKind::Trash],
    )
    .await?;
    for id in gone.into_iter().filter(|id| *id != path.page_id) {
        announce(&state, &mut tx, access, id, vec![PageChangeKind::Trash]).await?;
    }
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    let summary = view::summary_of(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

/// Bring a page in the trash back, with the sub-pages deleted with it.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/restore",
    operation_id = "pages.restore",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.trash")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page that was deleted."),
    ),
    responses(
        (status = OK, description = "The page, back where it was, or at the top level of its place when what it was under is gone or in the trash.", body = PageSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not delete it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is not in the trash (`page_not_trashed`), or was deleted with another page: restore that one (`page_not_trash_root`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn restore(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<Json<PageSummary>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Delete, path.page_id, true).await?;
    let Some(member) = access.user else {
        return Err(Error::Forbidden);
    };
    expect_trash_root(&row)?;
    let back: Vec<PageId> = sqlx::query_scalar(
        "UPDATE pages.pages
            SET trashed_at = NULL, trashed_by = NULL, trash_root = NULL, revision = revision + 1
          WHERE trash_root = $1
      RETURNING id",
    )
    .bind(path.page_id)
    .fetch_all(&mut *tx)
    .await?;
    // Back under its parent, unless that is gone or in the trash itself.
    if let Some(parent) = row.parent_id {
        let live: Option<bool> =
            sqlx::query_scalar("SELECT trashed_at IS NULL FROM pages.pages WHERE id = $1")
                .bind(parent)
                .fetch_optional(&mut *tx)
                .await?;
        if live != Some(true) {
            let place: Place = row.place()?;
            let key = position::next(&mut tx, None, place, member).await?;
            sqlx::query("UPDATE pages.pages SET parent_id = NULL, position = $2 WHERE id = $1")
                .bind(path.page_id)
                .bind(key)
                .execute(&mut *tx)
                .await?;
        }
    }
    tracked::simple(
        &state,
        &mut tx,
        access.workspace,
        path.page_id,
        tracked::Change::Restored,
    )
    .await?;
    announce(
        &state,
        &mut tx,
        access,
        path.page_id,
        vec![PageChangeKind::Trash],
    )
    .await?;
    for id in back.into_iter().filter(|id| *id != path.page_id) {
        announce(&state, &mut tx, access, id, vec![PageChangeKind::Trash]).await?;
    }
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    let summary = view::summary_of(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

fn expect_trash_root(row: &PageRow) -> Result<()> {
    if !row.is_trashed() {
        return Err(Error::Conflict(crate::MESSAGES.reason("page_not_trashed")));
    }
    if row.trash_root != Some(row.id) {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("page_not_trash_root"),
        ));
    }
    Ok(())
}

/// Delete a page in the trash, and what was deleted with it, for good.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}",
    operation_id = "pages.purge",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page that was deleted."),
    ),
    responses(
        (status = NO_CONTENT, description = "The page and everything deleted with it are gone for good."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not delete it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is not in the trash (`page_not_trashed`), or was deleted with another page (`page_not_trash_root`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn purge(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Delete, path.page_id, true).await?;
    expect_trash_root(&row)?;
    tracked::simple(
        &state,
        &mut tx,
        access.workspace,
        path.page_id,
        tracked::Change::Purged,
    )
    .await?;
    purge::root(&state, &mut tx, access.workspace, &row).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
