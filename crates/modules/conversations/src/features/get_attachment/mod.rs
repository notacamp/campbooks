//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}`
//! — one attachment's bytes, streamed from blob storage.
//!
//! The bytes, their name and their type come from the sender. They go out
//! as a download (`Content-Disposition: attachment`) under a sanitised file
//! name, with the stored type only when it is on the safe list
//! (`domain::attachments`, otherwise `application/octet-stream`), `nosniff`
//! and a `sandbox` policy, so nothing in them can run as a page of the app.
//! `inline=true` is honoured for raster images and PDFs only (the browser's
//! own viewer, in a sandboxed top-level document). No ranges: the store
//! verifies the whole object's hash as it streams.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderName, HeaderValue, header},
    response::{IntoResponse, Response},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::{BlobError, BlobRef},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use futures_util::StreamExt as _;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::domain::resources;
use crate::features::apply_changes::ATTACHMENT_RECORD;
use crate::{
    domain::{
        access::visible_inboxes,
        attachments::{
            OCTET_STREAM, content_disposition, may_be_inline, safe_filename, served_type, sniff,
        },
        paths::AttachmentPath,
    },
    model::{AttachmentId, ConversationId, InboxId, MessageId},
    scopes::ConversationsRead,
};
use campbooks_kernel::RecordRef;
use campbooks_kernel::authz::Action;

/// The policy of every attachment response: if a browser ever shows one as
/// a document, it has an opaque origin, runs nothing, loads nothing, cannot
/// be framed and submits nowhere.
pub const ATTACHMENT_POLICY: &str =
    "sandbox; default-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'";

/// `Cross-Origin-Resource-Policy`: no other site may embed the bytes.
pub const CROSS_ORIGIN_RESOURCE_POLICY: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");

/// An attachment's bytes, as the OpenAPI document describes them (a binary
/// string); the handler streams them.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct AttachmentBytes(());

/// How to serve it.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AttachmentQuery {
    /// `true` to show it in the browser (`Content-Disposition: inline`):
    /// honoured for raster images and PDFs only; anything else is still a
    /// download.
    #[serde(default)]
    pub inline: bool,
}

/// Download one attachment of a message.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}",
    operation_id = "conversations.get_attachment",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.message"]).access("conversations.attachment", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("message_id" = MessageId, Path, description = "The message."),
        ("attachment_id" = AttachmentId, Path, description = "The attachment."),
        AttachmentQuery,
    ),
    responses(
        (status = OK, description = "The bytes, as a download. `Content-Type` is the stored type when it is on the safe list (raster images, PDF, plain text, CSV, calendar, archives, office documents, common audio and video), else `application/octet-stream`; `Content-Disposition` carries the sanitised file name.",
            body = AttachmentBytes, content_type = "application/octet-stream"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such attachment, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<AttachmentPath>,
    extract::Query(query): extract::Query<AttachmentQuery>,
) -> Result<Response> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(ATTACHMENT_RECORD, &path.attachment),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let row = sqlx::query!(
        r#"SELECT a.filename, a.content_type, a.blob
             FROM conversations.attachments a
             JOIN conversations.messages m ON m.id = a.message_id
            WHERE a.id = $1 AND a.message_id = $2 AND m.conversation_id = $3
              AND m.inbox_id = ANY($4)"#,
        path.attachment as AttachmentId,
        path.message as MessageId,
        path.conversation as ConversationId,
        &inboxes as &[InboxId],
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    tx.commit().await?;
    let blob: BlobRef = serde_json::from_value(row.blob)
        .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
    let mut stream = state
        .blobs
        .get(&access.workspace.to_string(), &blob)
        .await?;

    // A type that may be shown inline must also look like one: its first
    // bytes decide which (an image sent as another is served as itself),
    // and anything else becomes an opaque download.
    let declared = served_type(&row.content_type);
    let (served, body) = if may_be_inline(declared) {
        let first = stream.next().await.transpose()?;
        let served = first.as_deref().and_then(sniff).unwrap_or(OCTET_STREAM);
        let head = futures_util::stream::iter(first.map(Ok::<_, BlobError>));
        (served, Body::from_stream(head.chain(stream)))
    } else {
        (declared, Body::from_stream(stream))
    };
    let inline = query.inline && may_be_inline(served);
    let name = safe_filename(&row.filename);
    let headers = [
        (header::CONTENT_TYPE, HeaderValue::from_static(served)),
        (
            header::CONTENT_DISPOSITION,
            content_disposition(inline, &name),
        ),
        (header::CONTENT_LENGTH, HeaderValue::from(blob.size)),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(ATTACHMENT_POLICY),
        ),
        (
            CROSS_ORIGIN_RESOURCE_POLICY,
            HeaderValue::from_static("same-origin"),
        ),
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
