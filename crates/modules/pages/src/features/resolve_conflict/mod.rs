//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/notion/conflicts/{block}`
//! — settle a conflict between a block and Notion's version of it
//! (campbooks-cloud#376).
//!
//! When the same block changed in Campbooks and in Notion between two syncs,
//! the page keeps its own version in place with Notion's beneath it; the
//! member chooses here: keep theirs (`mine`), take Notion's (`notion`), or
//! keep both (`both`). The choice changes the body, moves the base the sync
//! compares with, lets the block be sent again, and can be taken back with
//! `pages.undo_resolve_conflict` (the answer names the choice to undo).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as Db;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{
        PageId, access, announce, body, expect_live,
        notion::{self, resolve},
        store,
        tracked::{self, Choice},
    },
    events::PageChangeKind,
    scopes::PagesWrite,
};

/// `/api/v1/w/{workspace_id}/pages/{page_id}/notion/conflicts/{block}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ConflictPath {
    /// The page.
    pub page_id: PageId,
    /// The block the conflict is about (its stable id in the page).
    pub block: Uuid,
}

/// What to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = NotionKeep)]
pub enum Keep {
    /// The block as it is in Campbooks.
    Mine,
    /// Notion's version.
    Notion,
    /// Both, as ordinary blocks.
    Both,
}

impl Keep {
    /// The name kept in `pages.conflict_resolutions.choice`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::Notion => "notion",
            Self::Both => "both",
        }
    }

    const fn choice(self) -> Choice {
        match self {
            Self::Mine => Choice::Mine,
            Self::Notion => Choice::Notion,
            Self::Both => Choice::Both,
        }
    }
}

/// The choice.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ResolveNotionConflict)]
pub struct Resolve {
    /// What to keep.
    pub keep: Keep,
}

/// The conflict settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Resolved {
    /// This choice: what `pages.undo_resolve_conflict` takes back.
    pub resolution: Uuid,
    /// The page's revision afterwards.
    pub revision: i64,
    /// How many conflicts still wait.
    pub conflicts: i32,
}

/// Settle a conflict with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/notion/conflicts/{block}",
    operation_id = "pages.resolve_conflict",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.undo_resolve_conflict")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
        ("block" = Uuid, Path, description = "The block the conflict is about."),
    ),
    request_body = Resolve,
    responses(
        (status = OK, description = "The conflict is settled.", body = Resolved),
        (status = BAD_REQUEST, description = "The page would not hold the result (`invalid_update`, `unknown_block`, …).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`), has no conflict about that block (`no_conflict`), or the block is gone (`conflict_block_gone`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(
    clippy::too_many_lines,
    reason = "one transaction read top to bottom: resolve, store, record, track"
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ConflictPath>,
    extract::Json(request): extract::Json<Resolve>,
) -> Result<Json<Resolved>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    expect_live(&row)?;
    let Some(member) = access.user else {
        return Err(Error::Forbidden);
    };
    let loaded = store::load(&mut tx, path.page_id).await?;
    let resolver = notion::resolver(&mut tx, &state, access.workspace, path.page_id).await?;
    let mapping = notion::mappings(&mut tx, path.page_id)
        .await?
        .into_iter()
        .find(|mapping| mapping.bid == path.block);
    let resolution = match resolve::resolve(
        &loaded.doc,
        &resolver,
        path.block,
        request.keep.choice(),
        mapping.as_ref(),
    ) {
        Ok(resolution) => resolution,
        Err(resolve::Refused::NoConflict) => {
            return Err(Error::Conflict(crate::MESSAGES.reason("no_conflict")));
        }
        Err(resolve::Refused::BlockGone) => {
            return Err(Error::Conflict(
                crate::MESSAGES.reason("conflict_block_gone"),
            ));
        }
        Err(resolve::Refused::Body(rejection)) => return Err(rejection.into()),
    };
    if let Some(update) = &resolution.update {
        body::store(
            &mut tx,
            access,
            path.page_id,
            &loaded,
            update,
            &resolution.inspection,
        )
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
    notion::write_mappings(
        &mut tx,
        access.workspace,
        path.page_id,
        resolution.upsert.as_slice(),
        &resolution.deletes,
    )
    .await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO pages.conflict_resolutions
             (id, page_id, workspace_id, bid, choice, resolved_by, ours, theirs, removed,
              theirs_edited, notion_block_id, kind, base_hash, base_edited_at, extra_bid)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(id)
    .bind(path.page_id)
    .bind(access.workspace.as_uuid())
    .bind(path.block)
    .bind(request.keep.as_str())
    .bind(member)
    .bind(Db(&resolution.ours))
    .bind(Db(&resolution.theirs))
    .bind(resolution.theirs.is_none())
    .bind(resolution.theirs_edited)
    .bind(mapping.as_ref().map(|m| m.notion_block))
    .bind(mapping.as_ref().map(|m| m.kind.as_str()))
    .bind(mapping.as_ref().map(|m| m.base_hash.to_vec()))
    .bind(mapping.as_ref().map(|m| m.base_edited))
    .bind(resolution.extra)
    .execute(&mut *tx)
    .await?;
    let conflicts = i32::try_from(resolution.conflicts).unwrap_or(i32::MAX);
    sqlx::query("UPDATE pages.external_links SET conflicts = $2 WHERE page_id = $1")
        .bind(path.page_id)
        .bind(conflicts)
        .execute(&mut *tx)
        .await?;
    tracked::notion_conflict_resolved(
        &state,
        &mut tx,
        access.workspace,
        path.page_id,
        request.keep.choice(),
    )
    .await?;
    // What the choice lets go is looked at once the member stops typing.
    notion::schedule_push(&mut tx, access.workspace, path.page_id, chrono::Utc::now()).await?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.pages WHERE id = $1")
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Resolved {
        resolution: id,
        revision,
        conflicts,
    }))
}

#[cfg(test)]
mod tests;
