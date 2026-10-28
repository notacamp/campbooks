//! The kernel's `AttachmentBlobs` port: the blob of a message's attachment,
//! for another module (pages) that holds the same bytes without copying
//! them.
//!
//! It answers what `conversations.get_attachment` would let the caller
//! download, and nothing else: the caller must be allowed to read the
//! attachment (the catalogue's `read`), the conversation's inbox must be one
//! they belong to, and the attachment must be one of that message of that
//! conversation. Anything else (no such record, another member's inbox, a
//! wrong message) is the same answer, `None`. The bytes are not read here.

use std::sync::Arc;

use campbooks_kernel::{
    AppState, Error, RecordRef,
    attachments::{AttachmentBlob, AttachmentBlobFuture, AttachmentBlobs, AttachmentBlobsPort},
    authz::Action,
    blobs::BlobRef,
    tenancy::Access,
};
use sqlx::PgConnection;
use sqlx::types::Uuid;

use crate::{
    domain::{access::visible_inboxes, redact, resources},
    features::apply_changes::ATTACHMENT_RECORD,
};

/// The port, for the server to register.
pub fn attachment_blobs() -> AttachmentBlobsPort {
    Arc::new(Attachments)
}

#[derive(Debug, Clone, Copy)]
struct Attachments;

impl AttachmentBlobs for Attachments {
    fn blob_of<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        access: &'a Access,
        conversation: Uuid,
        message: Uuid,
        attachment: Uuid,
    ) -> AttachmentBlobFuture<'a> {
        Box::pin(async move {
            let record = RecordRef::new(ATTACHMENT_RECORD, &attachment);
            match resources::authorize(state, conn, access, Action::Read, &record).await {
                Ok(()) => {}
                // Not theirs to see, or not theirs to touch: one answer.
                Err(Error::NotFound | Error::Denied(_) | Error::Forbidden) => return Ok(None),
                Err(other) => return Err(other),
            }
            let inboxes: Vec<Uuid> = visible_inboxes(conn, access)
                .await?
                .iter()
                .map(|inbox| *inbox.as_uuid())
                .collect();
            let row: Option<(String, serde_json::Value)> = sqlx::query_as(
                "SELECT a.filename, a.blob
                   FROM conversations.attachments a
                   JOIN conversations.messages m ON m.id = a.message_id
                  WHERE a.id = $1 AND a.message_id = $2 AND m.conversation_id = $3
                    AND m.inbox_id = ANY($4)",
            )
            .bind(attachment)
            .bind(message)
            .bind(conversation)
            .bind(&inboxes)
            .fetch_optional(conn)
            .await?;
            let Some((filename, blob)) = row else {
                return Ok(None);
            };
            let blob: BlobRef =
                serde_json::from_value(blob).map_err(|e| Error::internal(redact::json(&e)))?;
            Ok(Some(AttachmentBlob { blob, filename }))
        })
    }
}

#[cfg(test)]
mod tests;
