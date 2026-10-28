//! `POST /api/v1/w/{workspace_id}/drafts/{draft_id}/attachments` — attach a
//! file from the author's device to a draft, as the raw bytes of the request
//! (campbooks-cloud#391).
//!
//! The upload is a change to the draft, like a save: it names the draft's
//! `version` (query) and its `content_token` (`X-Content-Token`), is checked
//! against the locked row, and answers with the draft at its new version.
//! The bytes become a blob the draft holds (owner `conversations.draft`), so
//! the sweeper keeps it until the draft drops it, is discarded or is sent.
//! Sending already carries a draft's attachments by their blobs.
//!
//! The author's own request only: the operation does not opt the assistant
//! in, and a request run in-process for someone else is refused. The media
//! type sent is kept for the message and never trusted.

use axum::{Json, body::Bytes, extract::State, http::HeaderMap, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        attachments::{MAX_FILES, clean_upload_name, is_blocked, upload_type},
        drafts::{
            self, AttachmentSource, DraftView, MAX_ATTACHMENT_BYTES, StoredAttachment,
            attachment_bytes, shown_to_author,
        },
        paths::DraftPath,
    },
    events::DRAFT_RECORD,
    model::{AttachmentId, DraftId, DraftState},
    scopes::ConversationsSend,
};

/// The header carrying the draft's `content_token`.
pub const CONTENT_TOKEN_HEADER: &str = "x-content-token";

/// A file's bytes, as the OpenAPI document describes the request body.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct UploadBytes(());

/// What the upload names besides its bytes.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UploadQuery {
    /// The draft's version as last read (a save's answer, or the draft).
    pub version: u32,
    /// The file's name. Cleaned before it is stored: no path, no control
    /// characters, at most 255 bytes.
    pub filename: String,
}

/// Attach a file to a draft.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}/attachments",
    operation_id = "conversations.upload_draft_attachment",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    // The author's own request: not `.assistant()`.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
        UploadQuery,
        ("X-Content-Token" = Option<String>, Header,
            description = "The `content_token` that came with the draft on screen, as a save names it."),
    ),
    request_body(content = UploadBytes, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The draft, with the file and its new version.",
            body = DraftView),
        (status = BAD_REQUEST, description = "No bytes (`file_empty`), no usable name (`filename_missing`), a type mail providers block (`attachment_blocked`) or over 20 files (`too_many_attachments`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Changed since that version (`draft_changed`), on its way (`draft_not_editable`), or not what you were shown (`content_changed`): read it again, then attach.",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "The file, or the draft's attachments together, would pass 25 MiB (`attachments_too_large`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
    extract::Query(query): extract::Query<UploadQuery>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<(StatusCode, Json<DraftView>)> {
    let access = &workspace.access;
    let id = path.draft_id;
    // Not the assistant's or automation's, even if the catalogue listed it
    // by mistake: the author attaches their own files.
    if workspace.invoked_by.is_some() {
        return Err(Error::Forbidden);
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Write).await?;
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(query.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    // The same check a save makes, over the same hash.
    let token = headers
        .get(CONTENT_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok());
    shown_to_author(&state, access, &row, token)?;

    if bytes.is_empty() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("file_empty")));
    }
    let Some(filename) = clean_upload_name(&query.filename) else {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("filename_missing"),
        ));
    };
    if is_blocked(&filename) {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("attachment_blocked")
                .arg("name", filename),
        ));
    }
    let mut attachments = row.stored_attachments()?;
    if attachments.len() >= MAX_FILES {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("too_many_attachments")
                .arg("max", MAX_FILES),
        ));
    }
    let size = bytes.len() as u64;
    if attachment_bytes(&attachments).saturating_add(size) > MAX_ATTACHMENT_BYTES {
        return Err(Error::PayloadTooLarge(
            crate::MESSAGES
                .reason("attachments_too_large")
                .arg("max", MAX_ATTACHMENT_BYTES / (1024 * 1024)),
        ));
    }

    // Everything is checked: the bytes are written now, so a refused upload
    // leaves no blob behind.
    let content_type = upload_type(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
    );
    let workspace_id = access.workspace.to_string();
    let blob = state.blobs.put(&workspace_id, bytes, &content_type).await?;
    attachments.push(StoredAttachment {
        id: AttachmentId::new(),
        source: AttachmentSource::Uploaded,
        filename,
        content_type: blob.content_type.clone(),
        size: blob.size,
        blob: blob.clone(),
    });
    state
        .blobs
        .attach(
            &mut tx,
            &workspace_id,
            &blob,
            &BlobOwner::new(DRAFT_RECORD, id),
        )
        .await?;
    sqlx::query(
        "UPDATE conversations.drafts
            SET attachments = $2, version = version + 1, state = $3,
                failure_reason = NULL, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(json!(attachments))
    .bind(DraftState::Draft.as_str())
    .execute(&mut *tx)
    .await?;
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

#[cfg(test)]
mod tests;
