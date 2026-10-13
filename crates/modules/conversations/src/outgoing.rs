//! The outgoing message v1 document, as this module writes it
//! (`contracts/outgoing-message.v1.schema.json`): what a connector sends,
//! provider neutral. A blob of the workspace that
//! [`SendRequested`](crate::events::SendRequested) points at.

use campbooks_kernel::blobs::BlobRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{Address, InboxId, SendId};

/// The document's media type.
pub const OUTGOING_CONTENT_TYPE: &str =
    "application/vnd.campbooks.outgoing-message+json; version=1";

/// The format version this module writes.
pub const OUTGOING_VERSION: u16 = 1;

/// A message to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// The format version, 1.
    pub version: u16,
    /// The inbox it is sent from.
    pub inbox: InboxId,
    /// This send.
    pub send_id: SendId,
    /// The `Message-ID` to send with, without angle brackets.
    pub rfc_message_id: String,
    /// The inbox's address and name.
    pub from: Address,
    /// Recipients.
    pub to: Vec<Address>,
    /// Copied recipients.
    pub cc: Vec<Address>,
    /// Blind-copied recipients.
    pub bcc: Vec<Address>,
    /// The subject.
    pub subject: String,
    /// The `Date` header.
    pub date: DateTime<Utc>,
    /// `In-Reply-To`, without angle brackets.
    pub in_reply_to: Option<String>,
    /// `References`, oldest first, without angle brackets.
    pub references: Vec<String>,
    /// The provider's thread to send in.
    pub thread_key: Option<String>,
    /// The plain-text part.
    pub text: String,
    /// The HTML alternative.
    pub html: Option<String>,
    /// Attachments, in order.
    pub attachments: Vec<OutgoingAttachment>,
}

/// An attachment to send: a blob of the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutgoingAttachment {
    /// Its file name.
    pub filename: String,
    /// Its media type.
    pub content_type: String,
    /// The bytes.
    pub blob: BlobRef,
}
