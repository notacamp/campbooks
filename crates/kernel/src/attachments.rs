//! A mail attachment's blob, for another module to hold.
//!
//! Attachments are stored as blobs (`blobs`, ADR 0011). A page can hold the
//! same bytes as one of a message's attachments without copying them, but
//! only a caller who may read that message may learn which blob it is. The
//! module that owns mail implements [`AttachmentBlobs`] and the server
//! registers it with
//! [`AppState::with_attachment_blobs`](crate::AppState::with_attachment_blobs).
//! Until then nothing is answered, so nothing can be attached.
//!
//! The file name reaches the caller in its process only: it is never put in
//! an event, a job or a log.

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{AppState, Error, blobs::BlobRef, tenancy::Access};

/// An attachment's blob, and the name the sender gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentBlob {
    /// The stored bytes: hash, size and the type the sender gave.
    pub blob: BlobRef,
    /// The file name as the message has it. Not cleaned: the caller cleans
    /// it for where it goes.
    pub filename: String,
}

/// A future returned by [`AttachmentBlobs::blob_of`].
pub type AttachmentBlobFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<AttachmentBlob>, Error>> + Send + 'a>>;

/// Answers an attachment's blob for a caller who may read its message.
pub trait AttachmentBlobs: std::fmt::Debug + Send + Sync + 'static {
    /// The blob of `attachment`, an attachment of `message` of
    /// `conversation`, read in `conn` (a transaction working in the
    /// caller's workspace). `None` unless `access` may read that
    /// conversation **and** the attachment belongs to that message of that
    /// conversation: a missing record, another member's inbox and a wrong
    /// message are all the same answer.
    fn blob_of<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        access: &'a Access,
        conversation: Uuid,
        message: Uuid,
        attachment: Uuid,
    ) -> AttachmentBlobFuture<'a>;
}

/// The port before a module provides one: no attachment is ever answered.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAttachmentBlobs;

impl AttachmentBlobs for NoAttachmentBlobs {
    fn blob_of<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _access: &'a Access,
        _conversation: Uuid,
        _message: Uuid,
        _attachment: Uuid,
    ) -> AttachmentBlobFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`AttachmentBlobs`] port.
pub type AttachmentBlobsPort = Arc<dyn AttachmentBlobs>;
