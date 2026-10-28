//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/unappend` — take back what
//! `pages.append` added (campbooks-cloud#364).
//!
//! Each block the append added is removed only if it still holds what it did
//! when it was added: a block someone has edited since stays, and the answer
//! says how many stayed. Taking back an append that is not there (undone
//! already, or never made on this page) removes nothing and says so, so an
//! undo can be sent again.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{PageId, access, announce, append, body, expect_live, notion, store},
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// Which append to take back.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UnappendRequest {
    /// `append_id` of the append.
    pub append_id: Uuid,
}

/// What was taken back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Unappended {
    /// How many blocks were removed.
    pub removed: u32,
    /// How many stayed because they were changed since.
    pub stayed: u32,
}

/// Take back an appended section.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/unappend",
    operation_id = "pages.unappend",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.append")
        .assistant()
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = UnappendRequest,
    responses(
        (status = OK, description = "How many blocks were removed and how many stayed.", body = Unappended),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<UnappendRequest>,
) -> Result<Json<Unappended>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    expect_live(&row)?;
    // Only this page's own appends, each taken back once.
    let units: Option<sqlx::types::Json<Vec<append::Unit>>> = sqlx::query_scalar(
        "DELETE FROM pages.appends WHERE id = $1 AND page_id = $2 RETURNING units",
    )
    .bind(request.append_id)
    .bind(path.page_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(sqlx::types::Json(units)) = units else {
        tx.commit().await?;
        return Ok(Json(Unappended {
            removed: 0,
            stayed: 0,
        }));
    };
    let loaded = store::load(&mut tx, path.page_id).await?;
    let taken = append::remove(&loaded.doc, &units)?;
    let removed = u32::try_from(taken.removed).unwrap_or(u32::MAX);
    let stayed = u32::try_from(taken.stayed).unwrap_or(u32::MAX);
    if taken.removed > 0 {
        let accepted = body::accept(
            &state,
            &mut tx,
            access,
            path.page_id,
            &loaded.doc,
            &taken.update,
        )
        .await?;
        if accepted.changed {
            body::store(
                &mut tx,
                access,
                path.page_id,
                &loaded,
                &taken.update,
                &accepted.inspection,
            )
            .await?;
            notion::schedule_push(&mut tx, access.workspace, path.page_id, chrono::Utc::now())
                .await?;
            announce(
                &state,
                &mut tx,
                access,
                path.page_id,
                vec![PageChangeKind::Body],
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(Unappended { removed, stayed }))
}

#[cfg(test)]
mod tests;
