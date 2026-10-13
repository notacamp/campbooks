//! The domain events this module publishes, each in the transaction of the
//! change it describes, and the events of other modules it consumes.
//!
//! **No personal data in events.** The event log is retained, so payloads
//! carry ids and non-personal facts only. Who wrote to whom, and about what,
//! is in the message's **participants document**
//! ([`ParticipantsDocument`](crate::documents::ParticipantsDocument)), a blob
//! the event points at: a consumer that needs it (parties, search) reads it
//! with `BlobStore::get` in the event's workspace, and it is deleted with the
//! workspace.

use campbooks_kernel::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::{BlobPointer, Channel, ConversationId, InboxId, LabelId, MessageId, Stream};

/// Facts about one message, shared by [`MessageReceived`] and
/// [`MessageSent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageFacts {
    /// The workspace.
    pub workspace: String,
    /// The inbox it belongs to.
    pub inbox: InboxId,
    /// Its conversation.
    pub conversation: ConversationId,
    /// The message.
    pub message: MessageId,
    /// The provider's id for it, unique within the inbox.
    pub provider_message_id: String,
    /// Its channel.
    pub channel: Channel,
    /// The conversation's stream when the message was stored.
    pub stream: Stream,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// The participants document (addresses and subject).
    pub participants: BlobPointer,
    /// The plain-text body, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<BlobPointer>,
    /// How many attachments it carries.
    pub attachment_count: u32,
}

/// An inbound message was stored for the first time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageReceived {
    /// The message.
    #[serde(flatten)]
    pub message: MessageFacts,
}

impl DomainEvent for MessageReceived {
    const NAME: &'static str = "conversations.message_received";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.message.workspace)
    }
}

/// An outbound message was stored for the first time (sent from the
/// provider's own client today; from Campbooks once sending lands).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageSent {
    /// The message.
    #[serde(flatten)]
    pub message: MessageFacts,
}

impl DomainEvent for MessageSent {
    const NAME: &'static str = "conversations.message_sent";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.message.workspace)
    }
}

/// Where a change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// A change batch from the provider: never sent back to it.
    Provider,
    /// A member (or automation, or the assistant) through an operation: the
    /// connector carries it to the provider.
    Member,
}

/// What changed about a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Read or unread.
    Read,
    /// Starred or unstarred.
    Starred,
    /// Archived or back in the inbox.
    Archived,
    /// Moved to another stream.
    Stream,
    /// A label was added or removed.
    Labels,
    /// Messages were moved in, moved out, trashed or removed.
    Messages,
    /// The conversation is gone (its last message was removed, or it was
    /// merged into another).
    Removed,
}

/// A conversation's state after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationState {
    /// Its stream.
    pub stream: Stream,
    /// Unread inbound messages.
    pub unread_count: u32,
    /// Whether any message is starred.
    pub starred: bool,
    /// Whether no message is in the inbox.
    pub archived: bool,
    /// Whether every message is in the trash or spam.
    pub trashed: bool,
}

/// A conversation's state changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// Where the change came from.
    pub origin: Origin,
    /// What changed.
    pub changes: Vec<Change>,
    /// The provider ids of the messages the change applies to, for carrying a
    /// member's change to the provider.
    pub provider_message_ids: Vec<String>,
    /// The state afterwards; `None` once the conversation is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<ConversationState>,
}

impl DomainEvent for ConversationChanged {
    const NAME: &'static str = "conversations.conversation_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A change a member asked for, to be made at the provider too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestedChange {
    /// Mark the message read.
    MarkRead,
    /// Mark the message unread.
    MarkUnread,
    /// Star (flag) the message.
    Star,
    /// Unstar the message.
    Unstar,
    /// Take the message out of the inbox.
    Archive,
    /// Put the message back in the inbox.
    Unarchive,
    /// Put a label on the message.
    AddLabel {
        /// The label.
        label: LabelId,
        /// The provider's key for it, when the provider already has it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_key: Option<String>,
    },
    /// Take a label off the message.
    RemoveLabel {
        /// The label.
        label: LabelId,
        /// The provider's key for it, when the provider has it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_key: Option<String>,
    },
    /// The conversation was moved to another stream (providers with their
    /// own categories may mirror it; others ignore it).
    MoveToStream {
        /// The stream.
        stream: Stream,
    },
}

/// One message and the change asked for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RequestedItem {
    /// The provider's id for the message.
    pub provider_message_id: String,
    /// The change.
    pub change: RequestedChange,
}

/// A member (or automation, or the assistant, through an operation) asked
/// for changes the provider should make too: the connector's outbound work.
/// Never published for changes that came from the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The inbox the messages belong to.
    pub inbox_id: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// Always `user`: changes from the provider produce no request.
    pub origin: RequestOrigin,
    /// The changes, message by message.
    pub items: Vec<RequestedItem>,
}

/// Who asked for a requested change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestOrigin {
    /// A member, automation or the assistant, through an operation.
    User,
}

impl DomainEvent for ChangesRequested {
    const NAME: &'static str = "conversations.changes_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.workspace_deleting` v1, as this module reads it (the fields it
/// needs; the publisher's payload has more).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its records are hard-deleted.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
