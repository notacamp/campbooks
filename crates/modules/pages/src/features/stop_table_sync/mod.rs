//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/notion/stop` — stop
//! keeping a table in step with its Notion database (campbooks-cloud#377).
//! The table and its rows stay as they are, the rows' own page links end,
//! and nothing is deleted in Notion.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::features::get_table_sync::TableLink;
use crate::{
    domain::{
        PageId,
        notion::{StopReason, tables as synced},
        tables::{self, TablePath},
    },
    scopes::PagesWrite,
};

/// Stop syncing a table with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/notion/stop",
    operation_id = "pages.stop_table_sync",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    responses(
        (status = OK, description = "The link, stopped.", body = TableLink),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, Notion databases are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), not linked to Notion, or the link has ended already (`not_syncing`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<TablePath>,
) -> Result<Json<TableLink>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    let link = synced::lock_external(&mut tx, path.table_id).await?;
    if link.is_none_or(|link| link.state == "stopped") {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_syncing")));
    }
    synced::stop_table(
        &state,
        &mut tx,
        access.workspace,
        path.table_id,
        StopReason::StoppedByMember,
    )
    .await?;
    let stopped = synced::external_of(&mut tx, path.table_id)
        .await?
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(Json(stopped.view()))
}
