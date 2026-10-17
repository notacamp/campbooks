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

use campbooks_kernel::{
    DomainEvent, RecordRef,
    blobs::BlobRef,
    changes::{Audience, Change as FeedChange},
    tenancy::UserId,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::{
    BlobPointer, Channel, ClearCause, ConnectionId, ConversationId, DraftId, InboxId, LabelId,
    MessageId, NeedReason, NeedTrust, RuleId, RuleRunId, RuleState, RunKind, RunState, SendId,
    Stream,
};

/// Record type of an inbox: the container a conversation's changes are
/// visible within (the change feed's `Visibility` port).
pub const INBOX_RECORD: &str = "conversations.inbox";
/// Record type of a conversation.
pub const CONVERSATION_RECORD: &str = "conversations.conversation";
/// Record type of a message.
pub const MESSAGE_RECORD: &str = "conversations.message";
/// Record type of a taught rule (visible within its inbox).
pub const RULE_RECORD: &str = "conversations.rule";
/// Record type of a draft (visible to its author only; the feed carries
/// its id, never its content).
pub const DRAFT_RECORD: &str = "conversations.draft";

/// `inbox` as the container of a change.
fn inbox_record(inbox: InboxId) -> RecordRef {
    RecordRef::new(INBOX_RECORD, &inbox)
}

/// A draft's changes reach its author alone, or the client tokens for one a
/// client wrote.
fn draft_audience(author: Option<UserId>) -> Audience {
    author.map_or(Audience::Clients, Audience::User)
}

impl MessageFacts {
    /// The conversation and the message, upserted, within their inbox.
    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::upsert(RecordRef::new(CONVERSATION_RECORD, &self.conversation))
                .within(inbox_record(self.inbox)),
            FeedChange::upsert(RecordRef::new(MESSAGE_RECORD, &self.message))
                .within(inbox_record(self.inbox)),
        ]
    }
}

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

    fn changes(&self) -> Vec<FeedChange> {
        self.message.changes()
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

    fn changes(&self) -> Vec<FeedChange> {
        self.message.changes()
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

    fn changes(&self) -> Vec<FeedChange> {
        let record = RecordRef::new(CONVERSATION_RECORD, &self.conversation);
        let change = if self.state.is_some() {
            FeedChange::upsert(record)
        } else {
            FeedChange::delete(record)
        };
        vec![change.within(inbox_record(self.inbox))]
    }
}

/// What a conversation needs, as `need_changed` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NeedFacts {
    /// Why.
    pub reason: NeedReason,
    /// How much Campbooks stands behind it.
    pub trust: NeedTrust,
    /// When it is due, when the message names a date that resolves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<NaiveDate>,
    /// Whether it is for today.
    pub urgent: bool,
}

/// A conversation's need changed: it opened, changed, or was cleared.
/// Ids, enums and a date only (`contracts/conversations.need_changed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NeedChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox: who may see the need.
    pub inbox: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// The inbound message the need comes from.
    pub message: MessageId,
    /// Grows with every change of this conversation's need; apply only
    /// newer ones (deliveries are not ordered).
    pub revision: i64,
    /// When that message was sent.
    pub sent_at: DateTime<Utc>,
    /// The need, or `None` once cleared.
    pub need: Option<NeedFacts>,
    /// Why it was cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared_because: Option<ClearCause>,
    /// The member who cleared it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared_by: Option<UserId>,
    /// The conversation's newest inbound message: when it changes, the
    /// sender wrote again (a member's Later ends) even if the need did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_message: Option<MessageId>,
}

impl DomainEvent for NeedChanged {
    const NAME: &'static str = "conversations.need_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::upsert(RecordRef::new(CONVERSATION_RECORD, &self.conversation))
                .within(inbox_record(self.inbox)),
        ]
    }
}

/// Scout's summary of the email behind a conversation's need was written
/// (campbooks-cloud#204). Ids only: never the summary. It exists so open
/// Home items and readers ask for the conversation again and show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NeedSummarised {
    /// The workspace.
    pub workspace: String,
    /// The inbox: who may see the conversation.
    pub inbox: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// The message summarised.
    pub message: MessageId,
}

impl DomainEvent for NeedSummarised {
    const NAME: &'static str = "conversations.need_summarised";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::upsert(RecordRef::new(CONVERSATION_RECORD, &self.conversation))
                .within(inbox_record(self.inbox)),
        ]
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

/// A member's send left its undo window: the connector's outbound work
/// (`contracts/conversations.send_requested.v1.schema.json`). The message
/// is an outgoing-message v1 blob; the event carries ids only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SendRequested {
    /// The workspace.
    pub workspace: String,
    /// The inbox it is sent from.
    pub inbox_id: InboxId,
    /// The draft being sent.
    pub draft: DraftId,
    /// This send: unique, the connector records it once.
    pub send_id: SendId,
    /// The outgoing-message v1 document, a blob of the workspace.
    #[schema(value_type = BlobPointer)]
    pub message: BlobRef,
    /// The draft's author; `None` for a client token's draft. Its change
    /// reaches only them (a draft is private).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<UserId>,
}

impl DomainEvent for SendRequested {
    const NAME: &'static str = "conversations.send_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::upsert(RecordRef::new(DRAFT_RECORD, &self.draft))
                .within(inbox_record(self.inbox_id))
                .for_audience(draft_audience(self.author)),
        ]
    }
}

/// A draft was sent: the provider accepted it and the draft is gone. The
/// message itself arrives like any other (`message_sent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DraftSent {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
    /// The draft, now deleted.
    pub draft: DraftId,
    /// The conversation it answered, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation: Option<ConversationId>,
    /// The draft's author; `None` for a client token's draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<UserId>,
}

impl DomainEvent for DraftSent {
    const NAME: &'static str = "conversations.draft_sent";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::delete(RecordRef::new(DRAFT_RECORD, &self.draft))
                .within(inbox_record(self.inbox))
                .for_audience(draft_audience(self.author)),
        ]
    }
}

/// A send failed: the draft is editable again and says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SendFailed {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
    /// The draft.
    pub draft: DraftId,
    /// Why (`not_connected`, `needs_reauth`, `rejected`, `too_large`,
    /// `unavailable`, `unknown_outcome`).
    pub reason: String,
    /// The draft's author; `None` for a client token's draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<UserId>,
}

impl DomainEvent for SendFailed {
    const NAME: &'static str = "conversations.send_failed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![
            FeedChange::upsert(RecordRef::new(DRAFT_RECORD, &self.draft))
                .within(inbox_record(self.inbox))
                .for_audience(draft_audience(self.author)),
        ]
    }
}

/// How a send ended, as the connector reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendOutcome {
    /// The provider accepted it.
    Sent,
    /// It was not sent (or may not have been: `unknown_outcome`).
    Failed,
}

/// `connectors.send_completed` v1, as this module reads it
/// (`contracts/connectors.send_completed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendCompleted {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox_id: InboxId,
    /// The send.
    pub send_id: SendId,
    /// Sent or failed.
    pub outcome: SendOutcome,
    /// The provider's id of the sent message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_message_id: Option<String>,
    /// Why it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl DomainEvent for SendCompleted {
    const NAME: &'static str = "connectors.send_completed";
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

/// `connectors.messages_changed` v1, as this module reads it
/// (`contracts/connectors.messages_changed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagesChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox; equals the batch's `inbox`.
    pub inbox_id: InboxId,
    /// The `MessageChangeBatch` v1 document, a blob of the workspace.
    pub batch: BlobRef,
}

impl DomainEvent for MessagesChanged {
    const NAME: &'static str = "connectors.messages_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.capability_granted` v1, as this module reads it
/// (`contracts/connectors.capability_granted.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGranted {
    /// The workspace.
    pub workspace: String,
    /// The connection (an id owned by the connectors module).
    pub connection: ConnectionId,
    /// The capability granted (`messages`, `calendar`, …).
    pub capability: String,
    /// The member whose consent it is: the new inbox's manager.
    pub granted_by: UserId,
    /// The inbox to create (`messages` only).
    #[serde(default)]
    pub inbox_id: Option<InboxId>,
    /// The connection-account v1 document (address, display name), a blob.
    pub account: BlobRef,
}

impl DomainEvent for CapabilityGranted {
    const NAME: &'static str = "connectors.capability_granted";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// The connection-account v1 document, as this module reads it
/// (`contracts/connection-account.v1.schema.json`).
/// The inbox changed in a way that may change who sees it, or what they
/// may do there: a member was given it, a membership ended, its owner
/// left, or it was kept read-only after its connection was removed. Ids
/// only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InboxChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
}

impl DomainEvent for InboxChanged {
    const NAME: &'static str = "conversations.inbox_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![FeedChange::upsert(inbox_record(self.inbox)).within(inbox_record(self.inbox))]
    }
}

/// The inbox was deleted with everything in it (its connection was removed
/// and its mail not kept). Ids only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InboxRemoved {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
}

impl DomainEvent for InboxRemoved {
    const NAME: &'static str = "conversations.inbox_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        vec![FeedChange::delete(inbox_record(self.inbox)).within(inbox_record(self.inbox))]
    }
}

/// What becomes of a removed connection's mail
/// (`contracts/connectors.connection_removed.v1.schema.json`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailChoice {
    /// Delete the inboxes it fed (also when the field is absent).
    #[default]
    Delete,
    /// Keep them read-only.
    Keep,
}

/// `connectors.connection_removed` v1, as this module reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRemoved {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The inboxes it fed.
    pub inbox_ids: Vec<InboxId>,
    /// Delete or keep their mail.
    #[serde(default)]
    pub mail: MailChoice,
}

impl DomainEvent for ConnectionRemoved {
    const NAME: &'static str = "connectors.connection_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_left` v1, as this module reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_suspended` v1, as this module reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberSuspended {
    /// The workspace.
    pub workspace: String,
    /// The member.
    pub user: UserId,
    /// When it took effect: orders the change only when it has no `seq`.
    pub at: DateTime<Utc>,
    /// The account's count of suspensions and lifts as of this change: the
    /// highest is the truth. Absent on changes from before it existed,
    /// which are older than any that has one.
    #[serde(default)]
    pub seq: Option<i64>,
}

impl DomainEvent for MemberSuspended {
    const NAME: &'static str = "workspaces.member_suspended";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_unsuspended` v1, as this module reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberUnsuspended {
    /// The workspace.
    pub workspace: String,
    /// The member.
    pub user: UserId,
    /// When it took effect: orders the change only when it has no `seq`.
    pub at: DateTime<Utc>,
    /// The account's count of suspensions and lifts as of this change, as
    /// [`MemberSuspended::seq`].
    #[serde(default)]
    pub seq: Option<i64>,
}

impl DomainEvent for MemberUnsuspended {
    const NAME: &'static str = "workspaces.member_unsuspended";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountDocument {
    /// The format version, 1.
    pub version: u16,
    /// The account's email address, lower-cased.
    pub address: String,
    /// The account's display name, when the provider gives one.
    #[serde(default)]
    pub display_name: Option<String>,
}

/// A taught rule's run, as `rule_changed` carries it: counts only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RunFacts {
    /// The run.
    pub id: RuleRunId,
    /// Applying the rule, or putting back what it changed.
    pub kind: RunKind,
    /// Where it is.
    pub state: RunState,
    /// Conversations it will look at.
    pub total: u32,
    /// Conversations done so far.
    pub done: u32,
}

/// A taught rule or its run changed (no other module consumes it; its schema
/// is in `openapi/events.json`).
/// Ids, enums and counts only: never its conditions, the member's words or
/// an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox: who may see the rule.
    pub inbox: InboxId,
    /// The rule.
    pub rule: RuleId,
    /// Its state.
    pub state: RuleState,
    /// Its current run, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunFacts>,
}

impl DomainEvent for RuleChanged {
    const NAME: &'static str = "conversations.rule_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<FeedChange> {
        // An upsert even once removed: a removed rule stays readable (and
        // its revert run followed) until it is purged with its inbox, so
        // clients refetch it rather than forget it.
        let record = RecordRef::new(RULE_RECORD, &self.rule);
        vec![FeedChange::upsert(record).within(inbox_record(self.inbox))]
    }
}
