//! `GET /api/v1/w/{workspace_id}/pages/{page_id}/files/{hash}` — the bytes
//! of a file or an image a page holds.
//!
//! The bytes come from a member. A raster image (told by its first bytes,
//! never by the type it was sent with) is served inline, so a page can show
//! it; anything else is a download of opaque bytes. Always `nosniff`, a
//! `sandbox` policy and no caching, so nothing in them can run as a page of
//! the app.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderName, HeaderValue, header},
    response::{IntoResponse, Response},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    blobs::{BlobError, BlobHash, BlobRef},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use futures_util::StreamExt as _;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{PageId, access},
    scopes::PagesRead,
};

/// The policy of every file response: if a browser ever shows one as a
/// document, it has an opaque origin, runs nothing, loads nothing and
/// submits nowhere.
const POLICY: &str =
    "sandbox; default-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'";

const CORP: HeaderName = HeaderName::from_static("cross-origin-resource-policy");

/// `/api/v1/w/{workspace_id}/pages/{page_id}/files/{hash}`
#[derive(Debug, Clone, Deserialize)]
pub struct FilePath {
    /// The page.
    pub page_id: PageId,
    /// The blob's hash.
    pub hash: String,
}

/// A file's bytes, as the OpenAPI document describes them.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct DownloadBytes(());

/// The raster image type the first bytes say it is, if any.
fn raster(first: &[u8]) -> Option<&'static str> {
    if first.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if first.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if first.starts_with(b"GIF87a") || first.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if first.get(..4) == Some(&b"RIFF"[..]) && first.get(8..12) == Some(&b"WEBP"[..]) {
        Some("image/webp")
    } else {
        None
    }
}

/// Download a file or an image of a page.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/files/{hash}",
    operation_id = "pages.get_file",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
        ("hash" = String, Path, description = "The blob's hash."),
    ),
    responses(
        (status = OK, description = "The bytes: a raster image inline, anything else as a download of opaque bytes.",
            body = DownloadBytes, content_type = "application/octet-stream"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page or file, or you cannot see the page.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<FilePath>,
) -> Result<Response> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    access::require_page(&state, &mut tx, access, Action::Read, path.page_id, false).await?;
    let hash: BlobHash = path.hash.parse().map_err(|_| Error::NotFound)?;
    let stored: Option<(i64, String)> = sqlx::query_as(
        "SELECT size, content_type FROM pages.files WHERE page_id = $1 AND hash = $2",
    )
    .bind(path.page_id)
    .bind(hash.to_string())
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let (size, content_type) = stored.ok_or(Error::NotFound)?;
    let blob = BlobRef {
        hash,
        size: u64::try_from(size).unwrap_or(0),
        content_type,
    };
    let mut stream = state
        .blobs
        .get(&access.workspace.to_string(), &blob)
        .await?;
    let first = stream.next().await.transpose()?;
    let image = first.as_deref().and_then(raster);
    let lead = futures_util::stream::iter(first.map(Ok::<_, BlobError>));
    let body = Body::from_stream(lead.chain(stream));
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static(image.unwrap_or("application/octet-stream")),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static(if image.is_some() {
                "inline"
            } else {
                "attachment; filename=\"download\""
            }),
        ),
        (header::CONTENT_LENGTH, HeaderValue::from(blob.size)),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(POLICY),
        ),
        (CORP, HeaderValue::from_static("same-origin")),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        ),
        (header::ACCEPT_RANGES, HeaderValue::from_static("none")),
    ];
    Ok((headers, body).into_response())
}

#[cfg(test)]
mod tests;
