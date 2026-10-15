//! `GET /admin/v1/workspaces/workspaces` — Admin → Workspaces: every
//! workspace of the instance, newest first, searchable by name prefix.
//!
//! An admin operation (ADR 0015). Requires `workspaces.read` (or `super`).
//! Metadata only: name, kind, when, how many members and who owns it.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::WorkspacesRead},
    db, extract,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::{
    admin_page::{AdminPage, Paging},
    model::Kind,
};

/// A workspace, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminWorkspace {
    /// The workspace.
    pub id: WorkspaceId,
    /// Its name.
    pub name: String,
    /// What it is for.
    pub kind: Kind,
    /// When it was created.
    pub created_at: DateTime<Utc>,
    /// When it was deleted (it is purged later); absent while it lives.
    pub deleted_at: Option<DateTime<Utc>>,
    /// How many members it has.
    pub member_count: i64,
    /// Its owners.
    pub owner_ids: Vec<UserId>,
}

/// One page of workspaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminWorkspacePage {
    /// Newest first.
    pub workspaces: Vec<AdminWorkspace>,
    /// The `after` of the next page; absent on the last one.
    pub next: Option<String>,
}

/// List workspaces.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/workspaces",
    operation_id = "workspaces.admin_list_workspaces",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<WorkspacesRead>()))),
    params(AdminPage),
    responses(
        (status = OK, description = "A page of workspaces.", body = AdminWorkspacePage),
        (status = BAD_REQUEST, description = "`invalid_query`, `invalid_limit`, \
            `invalid_cursor` or `query_too_long`.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `workspaces.read` (the \
            same answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<WorkspacesRead>,
    State(state): State<AppState>,
    extract::Query(page): extract::Query<AdminPage>,
) -> Result<Json<AdminWorkspacePage>> {
    let paging = page.paging()?;
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let page = execute(&mut tx, &paging).await?;
    tx.commit().await?;
    Ok(Json(page))
}

/// One page of workspaces, read in an admin transaction.
pub async fn execute(conn: &mut sqlx::PgConnection, paging: &Paging) -> Result<AdminWorkspacePage> {
    let rows = sqlx::query!(
        r#"
        SELECT w.id AS "id: WorkspaceId", w.name, w.kind, w.created_at, w.deleted_at,
               (SELECT count(*) FROM workspaces.members m
                 WHERE m.workspace_id = w.id) AS "member_count!",
               array(SELECT m.user_id FROM workspaces.members m
                      WHERE m.workspace_id = w.id AND m.role = 'owner'
                      ORDER BY m.joined_at, m.user_id) AS "owner_ids!: Vec<UserId>"
          FROM workspaces.workspaces w
         WHERE ($1::text IS NULL OR w.name ILIKE $1)
           AND ($2::timestamptz IS NULL OR (w.created_at, w.id) < ($2, $3))
         ORDER BY w.created_at DESC, w.id DESC
         LIMIT $4
        "#,
        paging.prefix,
        paging.after_at(),
        paging.after_id(),
        paging.fetch(),
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|row| {
        Ok(AdminWorkspace {
            id: row.id,
            name: row.name,
            kind: row.kind.parse()?,
            created_at: row.created_at,
            deleted_at: row.deleted_at,
            member_count: row.member_count,
            owner_ids: row.owner_ids,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let (workspaces, next) = paging.finish(rows, |workspace: &AdminWorkspace| {
        (workspace.created_at, *workspace.id.as_uuid())
    });
    Ok(AdminWorkspacePage { workspaces, next })
}

#[cfg(test)]
mod tests;
