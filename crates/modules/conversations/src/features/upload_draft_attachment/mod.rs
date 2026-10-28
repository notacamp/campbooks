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

use axum::{Extension, Json, body::Body, extract::State, http::HeaderMap, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
    time_limit::{self, Deadline},
};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgConnection;
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        attachments::{MAX_FILES, clean_upload_name, is_blocked, upload_type},
        drafts::{
            self, AttachmentSource, DraftView, MAX_ATTACHMENT_BYTES, StoredAttachment,
            attachment_bytes, shown_to_author,
        },
        paths::DraftPath,
        tracked,
    },
    events::DRAFT_RECORD,
    model::{AttachmentId, DraftId, DraftState},
    scopes::ConversationsSend,
    tracking,
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
        (status = BAD_REQUEST, description = "No bytes (`file_empty`), cut short (`upload_incomplete`), no usable name (`filename_missing`), a type mail providers block (`attachment_blocked`) or over 20 files (`too_many_attachments`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Changed since that version (`draft_changed`), on its way (`draft_not_editable`), or not what you were shown (`content_changed`): read it again, then attach.",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "The file, or the draft's attachments together, would pass 25 MiB (`attachments_too_large`), or what the draft's mailbox can send in one message (`attachments_too_large_for_mailbox`; `conversations.list_inboxes` says it as `max_attachment_bytes`).",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The bytes took longer to arrive than a request may run (`request_timed_out`, 100 s here): try again, or a smaller file.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
    extract::Query(query): extract::Query<UploadQuery>,
    deadline: Option<Extension<Deadline>>,
    headers: HeaderMap,
    body: Body,
) -> Result<(StatusCode, Json<DraftView>)> {
    let access = &workspace.access;
    let id = path.draft_id;
    // Not the assistant's or automation's, even if the catalogue listed it
    // by mistake: the author attaches their own files.
    if workspace.invoked_by.is_some() {
        return Err(Error::Forbidden);
    }
    // And a signed-in member's: a caller with no user has no draft to add
    // to (one with no author must never match it), so none is found.
    if access.user.is_none() {
        return Err(Error::NotFound);
    }
    // A large file over a slow line takes longer than a request's usual
    // limit: this one may run to the longest a handler may allow, and the
    // bytes are read here, after that, never before the handler runs.
    if let Some(Extension(deadline)) = &deadline {
        deadline.allow(time_limit::MAX);
    }
    let token = headers
        .get(CONTENT_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let content_type = upload_type(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
    );
    let (bytes, filename) = read_file(body, &query.filename).await?;
    let size = bytes.len() as u64;

    // Whether it may go, read without holding the draft: only then are the
    // bytes written (with no transaction or lock held for them), so nobody
    // stores a blob for a draft that is not theirs or that would refuse it.
    {
        let mut tx = state.db.begin_for(access.workspace).await?;
        let row = drafts::load(&state, &mut tx, access, id, false, Action::Write).await?;
        may_take(
            &state,
            &mut tx,
            access,
            &row,
            &query,
            token.as_deref(),
            size,
        )
        .await?;
    }
    let workspace_id = access.workspace.to_string();
    let blob = state.blobs.put(&workspace_id, bytes, &content_type).await?;

    // Checked again under the row's lock: it may have changed meanwhile. A
    // blob left behind by a refusal here is nobody's, and the sweeper takes
    // it after its grace period.
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Write).await?;
    let mut attachments = may_take(
        &state,
        &mut tx,
        access,
        &row,
        &query,
        token.as_deref(),
        size,
    )
    .await?;
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
    // A file added is a change to the draft, tracked as a save's change is.
    let workspace = access.workspace;
    tracked::track_draft(&state.tracker, &mut tx, id, |draft| {
        tracking::DraftUpdated { workspace, draft }
    })
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

/// The file the request carries, and its cleaned name: refused when it is
/// past [`MAX_ATTACHMENT_BYTES`], cut short, empty, unnamed or of a blocked
/// type, before anything is looked up or stored.
async fn read_file(body: Body, name: &str) -> Result<(axum::body::Bytes, String)> {
    let max = usize::try_from(MAX_ATTACHMENT_BYTES).unwrap_or(usize::MAX);
    let bytes = match axum::body::to_bytes(body, max).await {
        Ok(bytes) => bytes,
        Err(error) if is_too_long(&error) => return Err(too_large(MAX_ATTACHMENT_BYTES)),
        Err(_) => {
            return Err(Error::BadRequest(
                crate::MESSAGES.reason("upload_incomplete"),
            ));
        }
    };
    if bytes.is_empty() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("file_empty")));
    }
    let Some(filename) = clean_upload_name(name) else {
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
    Ok((bytes, filename))
}

/// Whether `row` may take a file of `size` bytes, as the request names it
/// (its version and content token): its attachments when it may. Refuses a
/// draft on its way, a stale or unseen one, a 21st file, and a file past
/// the limit of the draft's mailbox ([`max_attachment_bytes`]).
async fn may_take(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    row: &drafts::DraftRow,
    query: &UploadQuery,
    token: Option<&str>,
    size: u64,
) -> Result<Vec<StoredAttachment>> {
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(query.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    // The same check a save makes, over the same hash.
    shown_to_author(state, access, row, token)?;
    let attachments = row.stored_attachments()?;
    if attachments.len() >= MAX_FILES {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("too_many_attachments")
                .arg("max", MAX_FILES),
        ));
    }
    let limit = max_attachment_bytes(state, conn, row).await?;
    if attachment_bytes(&attachments).saturating_add(size) > limit {
        return Err(if limit < MAX_ATTACHMENT_BYTES {
            Error::PayloadTooLarge(
                crate::MESSAGES
                    .reason("attachments_too_large_for_mailbox")
                    .arg("max", megabytes(limit)),
            )
        } else {
            too_large(limit)
        });
    }
    Ok(attachments)
}

/// The most bytes of attachments the draft's mailbox can send in one
/// message: [`MAX_ATTACHMENT_BYTES`], or less when its provider takes less
/// (Microsoft Graph, campbooks-cloud#391).
pub async fn max_attachment_bytes(
    state: &AppState,
    conn: &mut PgConnection,
    row: &drafts::DraftRow,
) -> Result<u64> {
    let resolved = state
        .destinations
        .destination(conn, *row.inbox_id.as_uuid())
        .await?;
    Ok(resolved
        .and_then(|at| at.max_attachment_bytes)
        .map_or(MAX_ATTACHMENT_BYTES, |max| max.min(MAX_ATTACHMENT_BYTES)))
}

/// 413 `attachments_too_large`, naming `limit` in MB.
fn too_large(limit: u64) -> Error {
    Error::PayloadTooLarge(
        crate::MESSAGES
            .reason("attachments_too_large")
            .arg("max", megabytes(limit)),
    )
}

/// `bytes` in MB as the message says it: one decimal, rounded down, under
/// 10 MB ("2.1"), whole MiB above (25 MiB is "25").
fn megabytes(bytes: u64) -> String {
    if bytes < 10_000_000 {
        let tenths = bytes / 100_000;
        format!("{}.{}", tenths / 10, tenths % 10)
    } else {
        format!("{}", bytes / (1024 * 1024))
    }
}

/// Whether reading the body stopped at its limit.
fn is_too_long(error: &axum::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(at) = source {
        if at.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = at.source();
    }
    false
}

#[cfg(test)]
mod tests;
