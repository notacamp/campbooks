//! `GET /api/v1/w/{workspace_id}/pages/notion/links` — which of the pages the
//! caller may open are kept in step with Notion, and how that is going
//! (campbooks-cloud#376): syncing, paused or ended (and why), when it last
//! moved either way, and how many conflicts wait.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::{self, Action},
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        PageId,
        notion::{self, LinkView},
        resources::PAGE_RECORD,
        roles,
    },
    scopes::PagesRead,
};

/// The links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionLinks {
    /// The links of the pages the caller may open.
    pub links: Vec<LinkView>,
}

/// List the pages kept in step with Notion.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages/notion/links",
    operation_id = "pages.list_notion_links",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The links.", body = NotionLinks),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a token acting for nobody.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
) -> Result<Json<NotionLinks>> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Read, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let readable = roles::readable(&mut tx, access).await?;
    let mut links = Vec::new();
    for page in sqlx::query_scalar::<_, PageId>(
        "SELECT page_id FROM pages.external_links ORDER BY created_at, page_id",
    )
    .fetch_all(&mut *tx)
    .await?
    {
        if !readable.contains_key(&page) {
            continue;
        }
        if let Some(link) = notion::link_of(&mut tx, page).await? {
            links.push(link.view());
        }
    }
    tx.commit().await?;
    Ok(Json(NotionLinks { links }))
}

#[cfg(test)]
mod tests;
