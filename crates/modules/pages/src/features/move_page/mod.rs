//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/move` — put a page under
//! another, or at the top level of a place, at a position. What is sent is
//! where the page ends up, so an undo is the same call with where it was.
//!
//! Moving needs to write the page and the page it goes under. Moving a page
//! between the private and the workspace place changes who reads it, so
//! that needs to share it. A page never goes under itself or one of its
//! sub-pages, and nothing nests deeper than 12 levels.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    domain::{
        MAX_DEPTH, PageId, PageRow, PageSummary, Place, SELECT_ONE, access, announce, expect_live,
        expect_meta_revision, position, roles, tracked, view,
    },
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// Where the page should end up.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PageMove {
    /// The `meta_revision` you saw.
    pub meta_revision: i64,
    /// The page it goes under; none for the top level of `place`.
    #[serde(default)]
    pub parent: Option<PageId>,
    /// For a top-level page, `private` or `workspace` (default: where it
    /// is). A sub-page is always in its parent's place.
    #[serde(default)]
    pub place: Option<Place>,
    /// A fractional-indexing key among its new siblings. Default: after the
    /// last one.
    #[serde(default)]
    pub position: Option<String>,
}

/// The page and every page under it, with how deep each is below it (the
/// page itself is 1).
const SUBTREE: &str = "WITH RECURSIVE sub AS (
        SELECT id, 1 AS depth FROM pages.pages WHERE id = $1
        UNION ALL
        SELECT p.id, s.depth + 1 FROM pages.pages p JOIN sub s ON p.parent_id = s.id
         WHERE s.depth < 14
    ) SELECT id, depth FROM sub";

/// Move a page.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/move",
    operation_id = "pages.move",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.move")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = PageMove,
    responses(
        (status = OK, description = "The page afterwards.", body = PageSummary),
        (status = BAD_REQUEST, description = "A bad position (`invalid_position`), or a place that is not the parent's (`place_mismatch`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page (or where it goes) but not write it; moving it between the private and workspace places needs sharing it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it or where it goes.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page changed since you saw it (`page_changed`), is in the trash (`page_trashed`), would go inside itself (`page_cycle`) or nest deeper than 12 levels (`page_too_deep`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<PageMove>,
) -> Result<Json<PageSummary>> {
    let access = &workspace.access;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    let Some(mover) = access.user else {
        return Err(Error::Forbidden);
    };
    expect_live(&row)?;
    expect_meta_revision(&row, request.meta_revision)?;
    let before_place = row.place()?;

    // Where it goes.
    let place = match request.parent {
        Some(parent_id) => {
            let parent =
                access::require_page(&state, &mut tx, access, Action::Write, parent_id, false)
                    .await?;
            expect_live(&parent)?;
            let place = parent.place()?;
            if request.place.is_some_and(|asked| asked != place) {
                return Err(Error::BadRequest(crate::MESSAGES.reason("place_mismatch")));
            }
            place
        }
        None => request.place.unwrap_or(before_place),
    };
    // Who reads it changes with its place, and inside the private place with
    // its parent too (what a page is under decides who holds it): that is
    // sharing it. Reordering among the same siblings is only writing.
    let reparented = request.parent != row.parent_id;
    if place != before_place || (place == Place::Private && reparented) {
        access::require_page(&state, &mut tx, access, Action::Share, path.page_id, false).await?;
    }

    // Not inside itself, and not too deep.
    let below: Vec<(PageId, i32)> = sqlx::query_as(SUBTREE)
        .bind(path.page_id)
        .fetch_all(&mut *tx)
        .await?;
    if let Some(parent_id) = request.parent {
        if below.iter().any(|(id, _)| *id == parent_id) {
            return Err(Error::Conflict(crate::MESSAGES.reason("page_cycle")));
        }
        let above = roles::chain(&mut tx, parent_id).await?;
        let height = below.iter().map(|(_, depth)| *depth).max().unwrap_or(1);
        let height = usize::try_from(height).unwrap_or(MAX_DEPTH);
        if above.len() + height > MAX_DEPTH {
            return Err(Error::Conflict(crate::MESSAGES.reason("page_too_deep")));
        }
    }

    let key = match request.position {
        Some(key) => key,
        None => position::next(&mut tx, request.parent, place, mover).await?,
    };
    sqlx::query(
        "UPDATE pages.pages
            SET parent_id = $2, place = $3, position = $4, meta_revision = meta_revision + 1,
                revision = revision + 1, edited_by = $5, edited_at = now()
          WHERE id = $1",
    )
    .bind(path.page_id)
    .bind(request.parent)
    .bind(place.as_str())
    .bind(&key)
    .bind(mover)
    .execute(&mut *tx)
    .await?;
    let mut kinds = vec![PageChangeKind::Move];
    if place != before_place {
        spread_place(&state, &mut tx, access, path.page_id, &below, place).await?;
        kinds.push(PageChangeKind::Share);
    }
    announce(&state, &mut tx, access, path.page_id, kinds).await?;
    tracked::moved(
        &state,
        &mut tx,
        access.workspace,
        path.page_id,
        row.parent_id,
        before_place,
    )
    .await?;
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    let summary = view::summary_of(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

/// The pages under a moved page follow it into its new place.
async fn spread_place(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    moved: PageId,
    below: &[(PageId, i32)],
    place: Place,
) -> Result<()> {
    let inside: Vec<PageId> = below
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| *id != moved)
        .collect();
    sqlx::query("UPDATE pages.pages SET place = $2, revision = revision + 1 WHERE id = ANY($1)")
        .bind(&inside)
        .bind(place.as_str())
        .execute(&mut **tx)
        .await?;
    for id in inside {
        announce(state, tx, access, id, vec![PageChangeKind::Move]).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
