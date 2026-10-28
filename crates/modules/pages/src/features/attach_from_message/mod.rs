//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/files/from-message` — make
//! files of a message's attachments the files of a page, without copying
//! their bytes (campbooks-cloud#364).
//!
//! Blobs are content-addressed per workspace, so a page holds the same bytes
//! as the message by attaching its own owner. Which blob an attachment is
//! comes from the kernel's `AttachmentBlobs` port, which the module that owns
//! mail answers only to a caller who may read that conversation, and only
//! for an attachment of that message. If any of the attachments is not
//! answered the whole call is a 404 and nothing is attached: the same answer
//! whether the conversation is another member's, the message is a wrong one
//! or the attachment does not exist.
//!
//! The answer carries each file's name, type and size so the client can add
//! `file` blocks for them (through `pages.sync`, which announces the change
//! to the body: attaching alone changes no block). A file's name is page
//! content and lives in the body only; the table keeps where the file came
//! from.

use std::collections::BTreeSet;

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
    text,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::upload::MAX_FILE_BYTES;
use crate::{
    domain::{Kind, PageId, access, expect_live, tracked},
    features::get::PagePath,
    scopes::PagesWrite,
};

/// The most attachments one call takes.
pub const MAX_ATTACHMENTS: usize = 20;

/// The longest file name a block holds, in characters.
const MAX_NAME: usize = 255;

/// Which attachments to make files of the page.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachFromMessage {
    /// The conversation the message is in.
    pub conversation: Uuid,
    /// The message.
    pub message: Uuid,
    /// 1 to 20 of its attachments.
    pub attachments: Vec<Uuid>,
}

/// A file the page now holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AttachedFile {
    /// The attachment it came from.
    pub attachment: Uuid,
    /// The blob's hash: what a `file` or `image` block names.
    pub blob: String,
    /// The file's name as the message has it, cleaned for a block.
    pub name: String,
    /// The media type it was sent with, lower-cased; never trusted.
    pub content_type: String,
    /// Its size in bytes.
    pub size: u64,
}

/// The files the page holds now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AttachedFiles {
    /// One per attachment asked for, in order.
    pub files: Vec<AttachedFile>,
}

/// A file name as a block holds it: displayable characters only, at most
/// 255 of them, never empty.
fn block_name(raw: &str) -> String {
    let cleaned = text::displayable(raw);
    let name: String = cleaned
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .take(MAX_NAME)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        "file".to_owned()
    } else {
        name.to_owned()
    }
}

/// Make files of a message's attachments for a page.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/files/from-message",
    operation_id = "pages.attach_from_message",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.detach_files")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = AttachFromMessage,
    responses(
        (status = OK, description = "The files the page holds now, for the client's file blocks.", body = AttachedFiles),
        (status = BAD_REQUEST, description = "None, or more than 20, attachments (`invalid_attachments`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it; or any of the attachments is not one of that message that you may read. Nothing is attached.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`), or is a table (`not_a_page`).",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "An attachment is over 20 MiB (`file_too_large`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<AttachFromMessage>,
) -> Result<Json<AttachedFiles>> {
    let access = &workspace.access;
    // Each attachment once, in the order asked.
    let mut seen = BTreeSet::new();
    let asked: Vec<Uuid> = request
        .attachments
        .iter()
        .copied()
        .filter(|attachment| seen.insert(*attachment))
        .collect();
    if asked.is_empty() || asked.len() > MAX_ATTACHMENTS {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_attachments"),
        ));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    // Only a member writes (a client acting for nobody cannot see the page).
    let Some(uploader) = access.user else {
        return Err(Error::Forbidden);
    };
    expect_live(&row)?;
    if row.kind()? == Kind::Table {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_a_page")));
    }
    // Every attachment is answered by the mail module for this caller, or the
    // call is a 404 and nothing is attached.
    let mut answered = Vec::new();
    for attachment in &asked {
        let Some(found) = state
            .attachment_blobs
            .blob_of(
                &state,
                &mut tx,
                access,
                request.conversation,
                request.message,
                *attachment,
            )
            .await?
        else {
            return Err(Error::NotFound);
        };
        if found.blob.size > u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX) {
            return Err(Error::PayloadTooLarge(
                crate::MESSAGES.reason("file_too_large"),
            ));
        }
        answered.push((*attachment, found));
    }
    let workspace_id = access.workspace.to_string();
    let mut files = Vec::new();
    for (attachment, found) in answered {
        let size = i64::try_from(found.blob.size).unwrap_or(i64::MAX);
        sqlx::query(
            "INSERT INTO pages.files
                 (page_id, workspace_id, hash, size, content_type, uploaded_by, source_type, source_id)
             VALUES ($1, $2, $3, $4, $5, $6, 'conversations.attachment', $7)
             ON CONFLICT DO NOTHING",
        )
        .bind(path.page_id)
        .bind(access.workspace.as_uuid())
        .bind(found.blob.hash.to_string())
        .bind(size)
        .bind(&found.blob.content_type)
        .bind(uploader)
        .bind(attachment.to_string())
        .execute(&mut *tx)
        .await?;
        state
            .blobs
            .attach(
                &mut tx,
                &workspace_id,
                &found.blob,
                &BlobOwner::new("pages.page", *path.page_id.as_uuid()),
            )
            .await?;
        files.push(AttachedFile {
            attachment,
            blob: found.blob.hash.to_string(),
            name: block_name(&found.filename),
            content_type: found.blob.content_type.clone(),
            size: found.blob.size,
        });
    }
    tracked::files_attached(&state, &mut tx, access.workspace, path.page_id, files.len()).await?;
    tx.commit().await?;
    Ok(Json(AttachedFiles { files }))
}

#[cfg(test)]
mod tests;
