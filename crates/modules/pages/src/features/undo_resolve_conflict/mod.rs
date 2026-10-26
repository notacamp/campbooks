//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/notion/resolutions/{resolution}/undo`
//! — take back the choice made for a conflict with Notion
//! (campbooks-cloud#376): the conflict is back as it was, beneath the block,
//! and the sync's base where it stood. A choice is taken back once, and only
//! while the block it was about is still in the page.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as Db;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{
        PageId, access, announce, body, expect_live,
        notion::{
            self,
            merge::Mapping,
            model::{Block, Kind},
            resolve::{self, Taken},
        },
        store,
        tracked::{self, Choice},
    },
    events::PageChangeKind,
    scopes::PagesWrite,
};

/// `/api/v1/w/{workspace_id}/pages/{page_id}/notion/resolutions/{resolution}/undo`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ResolutionPath {
    /// The page.
    pub page_id: PageId,
    /// The choice.
    pub resolution: Uuid,
}

/// The choice taken back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Undone {
    /// The page's revision afterwards.
    pub revision: i64,
    /// How many conflicts wait now.
    pub conflicts: i32,
}

type Saved = (
    Uuid,
    String,
    Db<Block>,
    Db<Option<Block>>,
    bool,
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Option<String>,
    Option<Vec<u8>>,
    Option<DateTime<Utc>>,
    Option<Uuid>,
    bool,
);

/// Take back a choice for a conflict with Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/notion/resolutions/{resolution}/undo",
    operation_id = "pages.undo_resolve_conflict",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
        ("resolution" = Uuid, Path, description = "The choice `pages.resolve_conflict` answered."),
    ),
    responses(
        (status = OK, description = "The conflict is back.", body = Undone),
        (status = BAD_REQUEST, description = "The page would not hold the result.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "There is no such choice, it was taken back already or cannot be (`cannot_undo`), the block is gone (`conflict_block_gone`), or the page is in the trash.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<ResolutionPath>,
) -> Result<Json<Undone>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    expect_live(&row)?;
    let saved: Option<Saved> = sqlx::query_as(
        "SELECT bid, choice, ours, theirs, removed, theirs_edited, notion_block_id, kind,
                base_hash, base_edited_at, extra_bid, undone
           FROM pages.conflict_resolutions
          WHERE id = $1 AND page_id = $2 FOR UPDATE",
    )
    .bind(path.resolution)
    .bind(path.page_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((
        bid,
        choice,
        ours,
        theirs,
        removed,
        theirs_edited,
        notion_block,
        kind,
        base_hash,
        base_edited,
        extra,
        undone,
    )) = saved
    else {
        // No such choice (of this page): nothing to take back.
        return Err(Error::Conflict(crate::MESSAGES.reason("cannot_undo")));
    };
    let choice = match choice.as_str() {
        "mine" => Choice::Mine,
        "notion" => Choice::Notion,
        _ => Choice::Both,
    };
    // Notion's removal taken for good cannot be put back from here.
    if undone || (choice == Choice::Notion && removed) {
        return Err(Error::Conflict(crate::MESSAGES.reason("cannot_undo")));
    }
    let loaded = store::load(&mut tx, path.page_id).await?;
    let resolver = notion::resolver(&mut tx, &state, access.workspace, path.page_id).await?;
    let taken = Taken {
        bid,
        choice,
        ours: &ours.0,
        theirs: theirs.0.as_ref(),
        theirs_edited,
        extra,
    };
    let undone_by = match resolve::undo(&loaded.doc, &resolver, &taken) {
        Ok(undone_by) => undone_by,
        Err(resolve::Refused::BlockGone) => {
            return Err(Error::Conflict(
                crate::MESSAGES.reason("conflict_block_gone"),
            ));
        }
        Err(resolve::Refused::NoConflict) => {
            return Err(Error::Conflict(crate::MESSAGES.reason("cannot_undo")));
        }
        Err(resolve::Refused::Body(rejection)) => return Err(rejection.into()),
    };
    if let Some(update) = &undone_by.update {
        body::store(
            &mut tx,
            access,
            path.page_id,
            &loaded,
            update,
            &undone_by.inspection,
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
    // The base where it stood.
    if let (Some(notion_block), Some(kind), Some(hash), Some(edited)) = (
        notion_block,
        kind.as_deref().and_then(Kind::parse),
        base_hash
            .as_deref()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok()),
        base_edited,
    ) {
        notion::write_mappings(
            &mut tx,
            access.workspace,
            path.page_id,
            &[Mapping {
                bid,
                notion_block,
                kind,
                base_hash: hash,
                base_edited: edited,
            }],
            &[],
        )
        .await?;
    }
    sqlx::query("UPDATE pages.conflict_resolutions SET undone = true WHERE id = $1")
        .bind(path.resolution)
        .execute(&mut *tx)
        .await?;
    let conflicts = i32::try_from(undone_by.conflicts).unwrap_or(i32::MAX);
    sqlx::query("UPDATE pages.external_links SET conflicts = $2 WHERE page_id = $1")
        .bind(path.page_id)
        .bind(conflicts)
        .execute(&mut *tx)
        .await?;
    tracked::notion_conflict_raised(&state, &mut tx, access.workspace, path.page_id, 1).await?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.pages WHERE id = $1")
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Undone {
        revision,
        conflicts,
    }))
}

#[cfg(test)]
mod tests;
