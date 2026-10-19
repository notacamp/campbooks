//! `MessageChangeBatch` v1, as this module produces it
//! (`contracts/message-change-batch.v1.schema.json`).
//!
//! The conversations module reads the same document with its own types;
//! modules never import each other, so the JSON Schema file is the contract
//! and both sides test against it.

use campbooks_kernel::blobs::BlobRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::model::InboxId;

/// The batch document's media type.
pub const BATCH_CONTENT_TYPE: &str = "application/vnd.campbooks.message-changes+json; version=1";

/// What changed in one inbox, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageChangeBatch {
    /// The format version, 1.
    pub version: u16,
    /// The inbox the batch is for.
    pub inbox: InboxId,
    /// Read by the stream's first sync (its backfill): mail the inbox held
    /// before it was connected. Its streams are never mirrored to the
    /// provider.
    #[serde(default, skip_serializing_if = "is_false")]
    pub backfill: bool,
    /// Applied in this order.
    pub changes: Vec<MessageChange>,
}

impl MessageChangeBatch {
    /// A version-1 batch of `changes` for `inbox`.
    pub fn new(inbox: InboxId, changes: Vec<MessageChange>) -> Self {
        Self {
            version: 1,
            inbox,
            backfill: false,
            changes,
        }
    }
}

/// Whether `value` is false (left out of the document).
#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
const fn is_false(value: &bool) -> bool {
    !*value
}

/// A present value, as `Some` (absent stays `None`): an optional field that
/// may also be `null`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// One change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MessageChange {
    /// The message exists at the provider in this state.
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
    /// Unique and stable within the inbox.
    pub provider_message_id: String,
    /// The provider's thread id.
    pub thread_key: Option<String>,
    /// RFC 5322 `Message-ID`, without angle brackets.
    pub rfc_message_id: Option<String>,
    /// RFC 5322 `In-Reply-To`, without angle brackets.
    pub in_reply_to: Option<String>,
    /// RFC 5322 `References`, oldest first, without angle brackets.
    pub references: Vec<String>,
    /// The sender.
    pub from: Address,
    /// Recipients.
    pub to: Vec<Address>,
    /// Copied recipients.
    pub cc: Vec<Address>,
    /// `Reply-To`.
    pub reply_to: Vec<Address>,
    /// The subject.
    pub subject: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Received by the inbox, or sent from it.
    pub direction: Direction,
    /// The headers subset sorting reads.
    pub headers: Headers,
    /// The raw message (RFC 5322).
    pub raw: Option<BlobRef>,
    /// The plain-text part, UTF-8.
    pub text: Option<BlobRef>,
    /// The HTML part, UTF-8. Untrusted.
    pub html: Option<BlobRef>,
    /// Attachments, in order.
    pub attachments: Vec<Attachment>,
    /// Where the message is at the provider.
    pub mailbox: Mailbox,
    /// The provider's user labels.
    pub labels: Vec<Label>,
    /// The provider's own sorting.
    pub categories: Vec<Category>,
    /// Read at the provider.
    pub read: bool,
    /// Starred at the provider.
    pub starred: bool,
    /// The stream a received message is in at the provider (its stream
    /// mark, else its native place): absent when the adapter cannot tell
    /// (sent mail, a grant without labels), `Some(None)` (`null`) when it is
    /// in none of the four (Gmail's Social and Forums tabs, several marks).
    #[allow(
        clippy::option_option,
        reason = "absent, null or a stream: the contract's three cases"
    )]
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_stream: Option<Option<ProviderStream>>,
    /// The message's category changed at the provider while it kept a
    /// stream mark: `provider_stream` is the category's, and the mark is
    /// stale.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stream_mark_stale: bool,
}

/// A Campbooks stream, as a provider carries it (natively or by its stream
/// mark).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStream {
    /// Primary.
    Primary,
    /// Updates.
    Updates,
    /// Receipts.
    Receipts,
    /// Newsletters.
    Newsletters,
}

impl ProviderStream {
    /// Every stream.
    pub const ALL: [Self; 4] = [
        Self::Primary,
        Self::Updates,
        Self::Receipts,
        Self::Newsletters,
    ];

    /// The wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Updates => "updates",
            Self::Receipts => "receipts",
            Self::Newsletters => "newsletters",
        }
    }

    /// The stream named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == name)
    }
}

/// An email address with its display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    /// The display name, when given.
    pub name: Option<String>,
    /// The address.
    pub address: String,
}

/// Received by the inbox, or sent from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Received.
    Inbound,
    /// Sent.
    Outbound,
}

/// The headers subset sorting reads, values as sent, unfolded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Headers {
    /// `List-Id`.
    pub list_id: Option<String>,
    /// `List-Unsubscribe`.
    pub list_unsubscribe: Option<String>,
    /// `Precedence`.
    pub precedence: Option<String>,
    /// `Auto-Submitted`.
    pub auto_submitted: Option<String>,
    /// The receiving provider's own `Authentication-Results` (RFC 8601):
    /// the topmost one, kept only when its authserv-id is the provider's
    /// (`mail::Receiver`), so a sender cannot forge it. `None` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication_results: Option<String>,
}

/// An attachment, stored as a blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Its file name, as sent.
    pub filename: String,
    /// Its media type, as sent (not trusted).
    pub content_type: String,
    /// The bytes.
    pub blob: BlobRef,
    /// Shown inside the body rather than attached.
    pub inline: bool,
    /// Its `Content-ID`, without angle brackets: what the body's `cid:`
    /// images name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

/// Where a message is at the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mailbox {
    /// In the inbox (not archived).
    pub in_inbox: bool,
    /// In spam.
    pub in_spam: bool,
    /// In the trash.
    pub in_trash: bool,
}

/// A provider's user label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// The provider's stable key.
    pub key: String,
    /// Its display name.
    pub name: String,
}

/// A provider's own sorting (Gmail's category tabs), normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Notifications.
    Updates,
    /// Marketing.
    Promotions,
    /// Social networks.
    Social,
    /// Mailing lists.
    Forums,
}

/// The outgoing message document's media type
/// (`contracts/outgoing-message.v1.schema.json`).
pub const OUTGOING_CONTENT_TYPE: &str =
    "application/vnd.campbooks.outgoing-message+json; version=1";

/// A message to send, as the conversations module writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// The format version, 1.
    pub version: u16,
    /// The inbox it is sent from.
    pub inbox: InboxId,
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
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// `References`, oldest first.
    #[serde(default)]
    pub references: Vec<String>,
    /// The provider's thread to send in.
    #[serde(default)]
    pub thread_key: Option<String>,
    /// The plain-text part.
    pub text: String,
    /// The HTML alternative.
    #[serde(default)]
    pub html: Option<String>,
    /// Attachments, in order.
    #[serde(default)]
    pub attachments: Vec<OutgoingAttachment>,
}

/// An attachment to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutgoingAttachment {
    /// Its file name.
    pub filename: String,
    /// Its media type.
    pub content_type: String,
    /// The bytes, a blob of the workspace.
    pub blob: BlobRef,
}
