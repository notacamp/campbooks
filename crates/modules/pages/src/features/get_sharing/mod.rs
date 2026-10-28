//! `GET /api/v1/w/{workspace_id}/pages/{page_id}/sharing` — who can read a
//! page, as it really is, and whether it is kept in step with Notion
//! (campbooks-cloud#364).
//!
//! Scout reads this before it proposes putting anything on a page, so the
//! card can say who will read it. It is the *effective* audience: a workspace
//! page is every member's; any other page is read by its creator, by the
//! creator of every page above it and by everyone granted a role on it or on
//! a page above it (grants reach sub-pages). Ids only: names are the
//! workspace's to give. Any reader of the page may ask.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{PageId, Place, access, roles},
    features::get::PagePath,
    scopes::PagesRead,
};

/// Who reads a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageSharing {
    /// Private or workspace.
    pub place: Place,
    /// Every member of the workspace reads it (a workspace page): `readers`
    /// is then empty.
    pub everyone: bool,
    /// The members who read a private page, the caller included: its
    /// creator, the creators of the pages above it and everyone granted a
    /// role on it or above it.
    pub readers: Vec<UserId>,
    /// Whether the page is kept in step with a Notion page, so what is added
    /// here is sent there too.
    pub notion: bool,
}

/// Who can read a page, and whether it is in step with Notion.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/sharing",
    operation_id = "pages.get_sharing",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    responses(
        (status = OK, description = "Who reads the page, and whether Notion keeps a copy.", body = PageSharing),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<Json<PageSharing>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Read, path.page_id, false).await?;
    let place = row.place()?;
    let chain = roles::chain(&mut tx, path.page_id).await?;
    let ids: Vec<PageId> = chain.iter().map(|node| node.id).collect();
    let mut readers: Vec<UserId> = Vec::new();
    if place == Place::Private {
        readers.extend(chain.iter().map(|node| node.created_by));
        let granted: Vec<UserId> =
            sqlx::query_scalar("SELECT member_id FROM pages.grants WHERE page_id = ANY($1)")
                .bind(&ids)
                .fetch_all(&mut *tx)
                .await?;
        readers.extend(granted);
        readers.sort();
        readers.dedup();
    }
    let notion: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.external_links
                         WHERE page_id = $1 AND state <> 'stopped')",
    )
    .bind(path.page_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(PageSharing {
        place,
        everyone: place == Place::Workspace,
        readers,
        notion,
    }))
}

#[cfg(test)]
mod tests;
