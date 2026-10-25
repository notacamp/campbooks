//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/files` — a file or an
//! image for a page, as the raw bytes of the request.
//!
//! The bytes go to the workspace's blob store and are held by the page
//! (the sweeper never removes a blob a page holds). The answer is the blob's
//! hash, which a `file` or `image` block names; the file's name stays in the
//! body, never in a table. A block may only name a blob uploaded to its page
//! or by the same member (`pages.sync` checks).

use axum::{Json, body::Bytes, extract::State, http::HeaderMap, http::StatusCode, http::header};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{PageId, access, expect_live},
    features::get::PagePath,
    scopes::PagesWrite,
};

/// The largest file, in bytes.
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

/// A file's bytes, as the OpenAPI document describes the request body.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct UploadBytes(());

/// What was stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UploadedFile {
    /// The blob's hash: what a `file` or `image` block names.
    pub blob: String,
    /// Its size in bytes.
    pub size: u64,
    /// The media type it was sent with, lower-cased; never trusted.
    pub content_type: String,
}

/// Add a file or an image to a page.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/files",
    operation_id = "pages.upload_file",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body(content = UploadBytes, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored file.", body = UploadedFile),
        (status = BAD_REQUEST, description = "No bytes (`file_empty`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "Over 20 MiB.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<(StatusCode, Json<UploadedFile>)> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, false).await?;
    let Some(uploader) = access.user else {
        return Err(Error::Forbidden);
    };
    expect_live(&row)?;
    if bytes.is_empty() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("file_empty")));
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream");
    let workspace_id = access.workspace.to_string();
    let blob = state.blobs.put(&workspace_id, bytes, content_type).await?;
    let size = i64::try_from(blob.size).unwrap_or(i64::MAX);
    sqlx::query(
        "INSERT INTO pages.files (page_id, workspace_id, hash, size, content_type, uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
    )
    .bind(path.page_id)
    .bind(access.workspace.as_uuid())
    .bind(blob.hash.to_string())
    .bind(size)
    .bind(&blob.content_type)
    .bind(uploader)
    .execute(&mut *tx)
    .await?;
    state
        .blobs
        .attach(
            &mut tx,
            &workspace_id,
            &blob,
            &BlobOwner::new("pages.page", *path.page_id.as_uuid()),
        )
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(UploadedFile {
            blob: blob.hash.to_string(),
            size: blob.size,
            content_type: blob.content_type,
        }),
    ))
}

#[cfg(test)]
mod tests;
