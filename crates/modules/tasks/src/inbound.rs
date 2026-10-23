//! The other modules' events this module consumes, and the blob document it
//! points at, as their published contracts describe them (modules never
//! import each other): `contracts/conversations.message_read.v1` and
//! `contracts/read-texts.v1`.
//!
//! Each type reads only the fields this module uses; what the contract adds
//! later is ignored.

use campbooks_kernel::{DomainEvent, blobs::BlobRef, tenancy::UserId};
use serde::{Deserialize, Serialize};

/// An ask of a read, as `conversations.message_read` carries it: no title,
/// quote or amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadAsk {
    /// Stable for the same message, layer and position.
    pub key: String,
    /// `do` or `pay` (checked when used).
    pub verb: String,
    /// By when: a date, or an RFC 3339 time with an offset.
    #[serde(default)]
    pub due: Option<String>,
    /// The reference of an open task it repeats.
    #[serde(default)]
    pub same_as: Option<String>,
}

/// `conversations.message_read` v1: a read of one message was stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRead {
    /// The workspace.
    pub workspace: String,
    /// The message.
    pub message: String,
    /// Its conversation.
    pub conversation: String,
    /// Its inbox.
    pub inbox: String,
    /// The inbox's personal owner; absent for a shared inbox.
    #[serde(default)]
    pub owner: Option<UserId>,
    /// `structured` or `model`.
    pub layer: String,
    /// The obligations of the reader.
    #[serde(default)]
    pub asks: Vec<ReadAsk>,
    /// The `read-texts` v1 document, a blob of the workspace.
    #[serde(default)]
    pub texts: Option<BlobRef>,
}

impl DomainEvent for MessageRead {
    const NAME: &'static str = "conversations.message_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// The words of a read: the blob `texts` points at. Only the asks are read.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ReadTexts {
    /// One entry per ask that has any, by its key.
    #[serde(default)]
    pub asks: Vec<AskText>,
}

/// The words of one ask.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AskText {
    /// The ask's key.
    pub key: String,
    /// What to do.
    #[serde(default)]
    pub title: String,
    /// How much, for a payment.
    #[serde(default)]
    pub amount: Option<AskAmount>,
}

/// An amount in minor units.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AskAmount {
    /// Cents.
    pub minor: i64,
    /// ISO 4217 code.
    pub currency: String,
}
