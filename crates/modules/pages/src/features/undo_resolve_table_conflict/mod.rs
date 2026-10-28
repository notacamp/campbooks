//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}/undo`
//! — take back the choice made for a conflict with Notion
//! (campbooks-cloud#377): the conflict waits again, the sync's base where it
//! stood, and Campbooks' version put back when Notion's had been taken.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::features::resolve_table_conflict::ConflictPath;
use crate::{
    domain::{
        PageId, expect_live,
        notion::tables::{self as synced, conflicts},
        tables,
        tracked::{self, ConflictKind},
    },
    scopes::PagesWrite,
};

/// The choice taken back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = UndoneNotionTableConflict)]
pub struct Undone {
    /// How many conflicts wait now.
    pub conflicts: i32,
}

/// Take back a choice for a conflict with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}/undo",
    operation_id = "pages.undo_resolve_table_conflict",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.resolve_table_conflict")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("conflict" = Uuid, Path, description = "The conflict."),
    ),
    responses(
        (status = OK, description = "The conflict waits again.", body = Undone),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, Notion databases are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), no such conflict (`no_table_conflict`), or there is no choice to take back (`nothing_to_undo`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ConflictPath>,
) -> Result<Json<Undone>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    let conflict = conflicts::lock(&mut tx, path.table_id, path.conflict).await?;
    conflicts::undo(&state, &mut tx, access, editor, &conflict).await?;
    tracked::notion_table_conflict_raised(
        &state,
        &mut tx,
        access.workspace,
        path.table_id,
        if conflict.kind == "column" {
            ConflictKind::Column
        } else {
            ConflictKind::Cell
        },
        1,
    )
    .await?;
    let left = synced::open_conflicts(&mut tx, path.table_id).await?;
    tx.commit().await?;
    Ok(Json(Undone { conflicts: left }))
}
