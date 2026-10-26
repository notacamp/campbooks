//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/notion/stop` — stop
//! keeping a page in step with its Notion page (campbooks-cloud#376). The
//! page stays as it is, the Notion page is left alone, and nothing is deleted
//! on either side. Starting again is an import.

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
        PageId, access,
        notion::{self, LinkView, StopReason},
    },
    features::get::PagePath,
    scopes::PagesWrite,
};

/// Stop syncing a page with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/notion/stop",
    operation_id = "pages.stop_sync",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    responses(
        (status = OK, description = "The link, stopped.", body = LinkView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is not linked to a Notion page, or the link has ended already (`not_syncing`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
) -> Result<Json<LinkView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    let link = notion::lock_link(&mut tx, path.page_id).await?;
    if link.is_none_or(|link| link.state == "stopped") {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_syncing")));
    }
    notion::stop(
        &state,
        &mut tx,
        access.workspace,
        path.page_id,
        StopReason::StoppedByMember,
    )
    .await?;
    let stopped = notion::link_of(&mut tx, path.page_id)
        .await?
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(Json(stopped.view()))
}

#[cfg(test)]
mod tests;
