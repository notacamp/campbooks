//! Versioned JSON documents this module writes to blob storage for other
//! modules to read. Their schema is a contract, like an event's: never
//! change a field's meaning; add a version instead.

use serde::{Deserialize, Serialize};

use crate::model::Address;

/// The participants document's media type.
pub const PARTICIPANTS_CONTENT_TYPE: &str =
    "application/vnd.campbooks.message-participants+json; version=1";

/// Who wrote a message to whom, and its subject: the personal facts events
/// leave out. Stored once per distinct content (blobs are content-addressed)
/// and pointed at by `message_received` / `message_sent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantsDocument {
    /// Always 1 for this shape.
    pub version: u16,
    /// The sender.
    pub from: Address,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<Address>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Vec<Address>,
    /// Where replies go, when not to the sender.
    #[serde(default)]
    pub reply_to: Vec<Address>,
    /// The subject.
    #[serde(default)]
    pub subject: String,
}
