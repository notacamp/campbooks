//! The ingest contract: how message changes reach this module.
//!
//! A connector (the `connectors` module, ADR 0010) reads a provider and
//! normalises what changed into a [`MessageChangeBatch`]: an ordered list of
//! upserts and removals of messages of **one inbox**, in provider-neutral
//! vocabulary. It stores the batch as a JSON blob ([`BATCH_CONTENT_TYPE`]) in
//! the workspace, and the job `conversations.apply_changes` (payload
//! [`ApplyChangesPayload`]: the inbox and the batch's blob reference, ids
//! only) applies it. Bodies and attachments are blobs too, put by the
//! connector before the batch.
//!
//! Applying is idempotent per message (`(inbox, provider_message_id)`), so a
//! batch can be applied again, and batches of one inbox apply in order.
//!
//! The JSON shape is a versioned contract (`version: 1`). Connectors are a
//! separate module and never import this crate; they produce the same
//! document from its description in the module's spec.

use campbooks_kernel::blobs::BlobRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{Address, Direction, InboxId};

/// The batch document's media type.
pub const BATCH_CONTENT_TYPE: &str = "application/vnd.campbooks.message-changes+json; version=1";

/// The batch format version this module reads.
pub const BATCH_VERSION: u16 = 1;

/// The payload of `conversations.apply_changes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyChangesPayload {
    /// The inbox the batch is for.
    pub inbox: InboxId,
    /// The batch document, a blob in the job's workspace.
    pub batch: BlobRef,
}

/// What changed in one inbox, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageChangeBatch {
    /// The format version, [`BATCH_VERSION`].
    pub version: u16,
    /// The inbox; must match the job's.
    pub inbox: InboxId,
    /// The changes, applied in this order.
    pub changes: Vec<MessageChange>,
}

/// One change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MessageChange {
    /// The message exists at the provider in this state: insert it, or
    /// update its flags, mailbox state and labels.
    Upsert {
        /// The message.
        message: Box<NormalizedMessage>,
    },
    /// The message is gone from the provider.
    Remove {
        /// The provider's id for it.
        provider_message_id: String,
    },
}

/// A message in provider-neutral form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedMessage {
    /// The provider's id, unique and stable within the inbox.
    pub provider_message_id: String,
    /// The provider's thread id (Gmail `threadId`, Graph `conversationId`),
    /// when it has one. Messages sharing it form one conversation.
    #[serde(default)]
    pub thread_key: Option<String>,
    /// RFC 5322 `Message-ID`, without angle brackets.
    #[serde(default)]
    pub rfc_message_id: Option<String>,
    /// RFC 5322 `In-Reply-To`.
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// RFC 5322 `References`, oldest first.
    #[serde(default)]
    pub references: Vec<String>,
    /// The sender.
    pub from: Address,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<Address>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Vec<Address>,
    /// `Reply-To`.
    #[serde(default)]
    pub reply_to: Vec<Address>,
    /// The subject.
    #[serde(default)]
    pub subject: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Received by the inbox or sent from it.
    pub direction: Direction,
    /// The headers subset sorting reads.
    #[serde(default)]
    pub headers: Headers,
    /// The raw message (RFC 5322 for email), when kept.
    #[serde(default)]
    pub raw: Option<BlobRef>,
    /// The plain-text part (UTF-8).
    #[serde(default)]
    pub text: Option<BlobRef>,
    /// The HTML part (UTF-8). Untrusted: shown only in a sandboxed frame.
    #[serde(default)]
    pub html: Option<BlobRef>,
    /// Attachments, in order.
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    /// Where the message is at the provider.
    pub mailbox: Mailbox,
    /// The provider's user labels on it.
    #[serde(default)]
    pub labels: Vec<ProviderLabel>,
    /// The provider's own categories (Gmail's tabs), normalised.
    #[serde(default)]
    pub categories: Vec<ProviderCategory>,
    /// Read at the provider.
    #[serde(default)]
    pub read: bool,
    /// Starred (flagged) at the provider.
    #[serde(default)]
    pub starred: bool,
}

/// The headers subset sorting reads (email). Values as sent, unfolded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Headers {
    /// `List-Id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_id: Option<String>,
    /// `List-Unsubscribe`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_unsubscribe: Option<String>,
    /// `Precedence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedence: Option<String>,
    /// `Auto-Submitted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_submitted: Option<String>,
    /// The receiving provider's own `Authentication-Results`, as the
    /// connector vouches for it (never a header the sender wrote).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication_results: Option<String>,
}

/// An attachment, stored as a blob by the connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    /// Its file name, as sent.
    pub filename: String,
    /// Its media type, as sent (not trusted).
    pub content_type: String,
    /// The bytes.
    pub blob: BlobRef,
    /// Shown inside the body (an inline image) rather than attached.
    #[serde(default)]
    pub inline: bool,
    /// Its `Content-ID`, without angle brackets: what the HTML body's
    /// `cid:` images name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

/// Where a message is at the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mailbox {
    /// In the inbox (not archived).
    pub in_inbox: bool,
    /// In spam / junk.
    #[serde(default)]
    pub in_spam: bool,
    /// In the trash / deleted items.
    #[serde(default)]
    pub in_trash: bool,
}

/// A provider's user label (Gmail label, Graph category, IMAP keyword).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderLabel {
    /// The provider's stable key for it.
    pub key: String,
    /// Its display name.
    pub name: String,
}

/// A provider's own sorting of a message (Gmail's category tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCategory {
    /// Notifications and alerts.
    Updates,
    /// Marketing.
    Promotions,
    /// Social networks.
    Social,
    /// Mailing lists and forums.
    Forums,
}

impl ProviderCategory {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Updates => "updates",
            Self::Promotions => "promotions",
            Self::Social => "social",
            Self::Forums => "forums",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_batch_parses_with_defaults() {
        let inbox = InboxId::new();
        let batch: MessageChangeBatch = serde_json::from_value(serde_json::json!({
            "version": 1,
            "inbox": inbox,
            "changes": [
                {"op": "upsert", "message": {
                    "provider_message_id": "m1",
                    "from": {"address": "ana@example.com"},
                    "sent_at": "2026-10-01T09:00:00Z",
                    "direction": "inbound",
                    "mailbox": {"in_inbox": true}
                }},
                {"op": "remove", "provider_message_id": "m0"}
            ]
        }))
        .unwrap();
        assert_eq!(batch.changes.len(), 2);
        let MessageChange::Upsert { message } = &batch.changes[0] else {
            panic!("expected an upsert");
        };
        assert!(!message.read && message.labels.is_empty() && message.text.is_none());
        assert_eq!(message.headers, Headers::default());
    }
}
