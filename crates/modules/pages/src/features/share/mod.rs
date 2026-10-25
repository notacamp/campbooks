//! `PUT …/pages/{page_id}/grants/{member_id}` and
//! `DELETE …/pages/{page_id}/grants/{member_id}` — share a page with a
//! member of the workspace as a `reader`, `writer` or `manager`, or stop.
//!
//! A grant applies to the page and to every sub-page that has no grant of
//! its own for that member. It needs the right to share the page (its
//! creator, a workspace owner for a workspace page, or a `manager`).

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{GrantView, PageId, Role, access, announce},
    events::PageChangeKind,
    scopes::PagesWrite,
};

/// `/api/v1/w/{workspace_id}/pages/{page_id}/grants/{member_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GrantPath {
    /// The page.
    pub page_id: PageId,
    /// The member.
    pub member_id: UserId,
}

/// What the member may do.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewGrant {
    /// `reader`, `writer` or `manager`.
    pub role: Role,
}

/// Hold the member's lock until the transaction ends.
pub async fn lock_member(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: WorkspaceId,
    member: UserId,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("pages.member:{workspace}:{member}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Share a page with a member, or change their role.
#[utoipa::path(
    put,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/grants/{member_id}",
    operation_id = "pages.share",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.unshare")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Share)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
        ("member_id" = UserId, Path, description = "The member."),
    ),
    request_body = NewGrant,
    responses(
        (status = OK, description = "The member's role on the page.", body = GrantView),
        (status = BAD_REQUEST, description = "Not a member of this workspace (`not_a_member`), or yourself (`share_yourself`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not share it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn share(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<GrantPath>,
    extract::Json(request): extract::Json<NewGrant>,
) -> Result<Json<GrantView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Share, path.page_id, true).await?;
    let Some(sharer) = access.user else {
        return Err(Error::Forbidden);
    };
    crate::domain::expect_live(&row)?;
    if path.member_id == sharer {
        return Err(Error::BadRequest(crate::MESSAGES.reason("share_yourself")));
    }
    // One at a time per member with `pages.forget_member`, so a grant never
    // outlives the membership it was checked against.
    lock_member(&mut tx, access.workspace, path.member_id).await?;
    let member = state
        .membership
        .role_held(&mut tx, access.workspace, path.member_id)
        .await?;
    if member.is_none() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("not_a_member")));
    }
    sqlx::query(
        "INSERT INTO pages.grants (page_id, workspace_id, member_id, role, granted_by)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (page_id, member_id)
         DO UPDATE SET role = EXCLUDED.role, granted_by = EXCLUDED.granted_by, at = now()",
    )
    .bind(path.page_id)
    .bind(access.workspace.as_uuid())
    .bind(path.member_id)
    .bind(request.role.as_str())
    .bind(sharer)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE pages.pages SET revision = revision + 1 WHERE id = $1")
        .bind(path.page_id)
        .execute(&mut *tx)
        .await?;
    announce(
        &state,
        &mut tx,
        access,
        path.page_id,
        vec![PageChangeKind::Share],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(GrantView {
        member: path.member_id,
        role: request.role,
    }))
}

/// Stop sharing a page with a member.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/grants/{member_id}",
    operation_id = "pages.unshare",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.share")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Share)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
        ("member_id" = UserId, Path, description = "The member."),
    ),
    responses(
        (status = NO_CONTENT, description = "The page is no longer shared with the member."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not share it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it, or it was not shared with the member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unshare(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<GrantPath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    access::require_page(&state, &mut tx, access, Action::Share, path.page_id, true).await?;
    let removed = sqlx::query("DELETE FROM pages.grants WHERE page_id = $1 AND member_id = $2")
        .bind(path.page_id)
        .bind(path.member_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if removed == 0 {
        return Err(Error::NotFound);
    }
    sqlx::query("UPDATE pages.pages SET revision = revision + 1 WHERE id = $1")
        .bind(path.page_id)
        .execute(&mut *tx)
        .await?;
    announce(
        &state,
        &mut tx,
        access,
        path.page_id,
        vec![PageChangeKind::Share],
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
