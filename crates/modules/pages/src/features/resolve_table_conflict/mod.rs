//! `POST /api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}`
//! — settle a conflict between a column or a cell and Notion's version of it
//! (campbooks-cloud#377).
//!
//! When the same column or cell changed in Campbooks and in Notion between
//! two syncs, neither change is sent. The member chooses here: keep theirs
//! (`ours`, Campbooks' version, which then goes to Notion) or take Notion's
//! (`theirs`). Either way the base the sync compares with moves, and the
//! choice can be taken back with `pages.undo_resolve_table_conflict`.

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
use uuid::Uuid;

use crate::{
    domain::{
        PageId, expect_live,
        notion::tables::{
            self as synced,
            conflicts::{self, Keep},
        },
        tables,
        tracked::{self, ConflictKind},
    },
    scopes::PagesWrite,
};

/// `/api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ConflictPath {
    /// The table.
    pub table_id: PageId,
    /// The conflict.
    pub conflict: Uuid,
}

/// The choice.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ResolveNotionTableConflict)]
pub struct Resolve {
    /// What to keep.
    pub keep: Keep,
}

/// The conflict settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = ResolvedNotionTableConflict)]
pub struct Resolved {
    /// The conflict, which `pages.undo_resolve_table_conflict` takes back.
    pub conflict: Uuid,
    /// How many conflicts still wait.
    pub conflicts: i32,
}

/// Settle a conflict with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}",
    operation_id = "pages.resolve_table_conflict",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.undo_resolve_table_conflict")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
        ("conflict" = Uuid, Path, description = "The conflict."),
    ),
    request_body = Resolve,
    responses(
        (status = OK, description = "The conflict is settled.", body = Resolved),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the table but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, Notion databases are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`), in the trash (`page_trashed`), no such conflict (`no_table_conflict`), or the conflict was settled already (`conflict_settled`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ConflictPath>,
    extract::Json(request): extract::Json<Resolve>,
) -> Result<Json<Resolved>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let table =
        tables::require_table(&state, &mut tx, access, Action::Write, path.table_id, true).await?;
    expect_live(&table)?;
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    let conflict = conflicts::lock(&mut tx, path.table_id, path.conflict).await?;
    if conflict.choice.is_some() {
        return Err(Error::Conflict(crate::MESSAGES.reason("conflict_settled")));
    }
    conflicts::resolve(&state, &mut tx, access, editor, &conflict, request.keep).await?;
    tracked::notion_table_conflict_resolved(
        &state,
        &mut tx,
        access.workspace,
        path.table_id,
        if conflict.kind == "column" {
            ConflictKind::Column
        } else {
            ConflictKind::Cell
        },
        request.keep == Keep::Ours,
    )
    .await?;
    synced::schedule_push(&mut tx, access.workspace, path.table_id).await?;
    let left = synced::open_conflicts(&mut tx, path.table_id).await?;
    tx.commit().await?;
    Ok(Json(Resolved {
        conflict: path.conflict,
        conflicts: left,
    }))
}
