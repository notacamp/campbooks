//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/files/detach` — take back
//! files that `pages.attach_from_message` made files of a page
//! (campbooks-cloud#364).
//!
//! Only a file that came from a message is taken back, and only when no
//! block of the page still names it: a file the member uploaded, or one a
//! block shows, is never touched. The bytes stay with the message (and with
//! any other page holding them); only this page lets go.

use std::collections::BTreeSet;

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::{BlobHash, BlobOwner},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{PageId, access, document, expect_live, store},
    features::{attach_from_message::MAX_ATTACHMENTS, get::PagePath},
    scopes::PagesWrite,
};

/// Which files to take back.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DetachFiles {
    /// 1 to 20 blob hashes, as `pages.attach_from_message` answered them.
    pub files: Vec<String>,
}

/// What was taken back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DetachedFiles {
    /// The hashes the page let go of.
    pub removed: Vec<String>,
    /// The hashes it kept: a block still names them, they were not saved
    /// from a message, or the page did not hold them.
    pub kept: Vec<String>,
}

/// Take back files saved from a message.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/files/detach",
    operation_id = "pages.detach_files",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.attach_from_message")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = DetachFiles,
    responses(
        (status = OK, description = "What the page let go of, and what it kept.", body = DetachedFiles),
        (status = BAD_REQUEST, description = "None, or more than 20, files, or a hash that is not one (`invalid_attachments`).",
            body = Problem, content_type = "application/problem+json"),
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
    extract::Json(request): extract::Json<DetachFiles>,
) -> Result<Json<DetachedFiles>> {
    let access = &workspace.access;
    let mut hashes: Vec<BlobHash> = Vec::new();
    let mut seen = BTreeSet::new();
    for text in &request.files {
        let hash: BlobHash = text
            .parse()
            .map_err(|_| Error::BadRequest(crate::MESSAGES.reason("invalid_attachments")))?;
        if seen.insert(hash.to_string()) {
            hashes.push(hash);
        }
    }
    if hashes.is_empty() || hashes.len() > MAX_ATTACHMENTS {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_attachments"),
        ));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    expect_live(&row)?;
    let loaded = store::load(&mut tx, path.page_id).await?;
    let named = document::inspect(&loaded.doc)?.blobs;
    let workspace_id = access.workspace.to_string();
    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for hash in hashes {
        let text = hash.to_string();
        let gone = if named.contains(&text) {
            false
        } else {
            sqlx::query(
                "DELETE FROM pages.files
                  WHERE page_id = $1 AND hash = $2 AND source_type IS NOT NULL",
            )
            .bind(path.page_id)
            .bind(&text)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0
        };
        if gone {
            state
                .blobs
                .detach(
                    &mut tx,
                    &workspace_id,
                    &hash,
                    &BlobOwner::new("pages.page", *path.page_id.as_uuid()),
                )
                .await?;
            removed.push(text);
        } else {
            kept.push(text);
        }
    }
    tx.commit().await?;
    Ok(Json(DetachedFiles { removed, kept }))
}

#[cfg(test)]
mod tests;
