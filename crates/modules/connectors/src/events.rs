//! The domain events this module publishes, each in the transaction of the
//! change it describes, and the events of other modules it consumes.
//!
//! **No personal data in events** (`contracts/README.md`): the event log is
//! retained, so payloads carry ids, blob references and non-personal facts.
//! The account's address travels in a blob ([`AccountDocument`]); message
//! content in `MessageChangeBatch` blobs. The payloads of
//! [`MessagesChanged`] and [`CapabilityGranted`] are contracts with the
//! conversations module (`contracts/*.schema.json`).

use campbooks_kernel::{DomainEvent, RecordRef, blobs::BlobRef, changes::Change, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use campbooks_kernel::changes::Audience;

use crate::domain::model::{Capability, ConnectionId, InboxId, MailChoice, Provider, SendId};

/// The record type of a connection in the client change feed.
pub const CONNECTION_RECORD: &str = "connectors.connection";

/// A connection, as the change feed names it (ids only).
fn connection_record(connection: ConnectionId) -> RecordRef {
    RecordRef::new(CONNECTION_RECORD, &connection)
}

/// A connection's changes reach the members who may see it over the API:
/// the member who connected it, owners and admins, and client tokens.
fn connection_change(change: Change, granted_by: Option<UserId>) -> Change {
    match granted_by {
        Some(user) => change.for_audience(Audience::UserAndManagers(user)),
        None => change.for_audience(Audience::Clients),
    }
}

/// How a [`BlobRef`] looks on the wire, for the events catalogue.
#[derive(ToSchema)]
#[schema(as = BlobRef)]
#[allow(dead_code, reason = "a schema description only, never built")]
pub struct BlobSchema {
    /// SHA-256 of the bytes, 64 lower-case hex digits.
    hash: String,
    /// Length in bytes.
    size: u64,
    /// The media type given when it was stored.
    content_type: String,
}

/// A member connected an outside account, or connected it again (a
/// reconnect, or the same account consenting anew: `refreshed`). Published
/// in the transaction that stores the grant, so whoever handed the grant
/// over can tell which connection it became even if its own request died.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConnectionAdded {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The provider.
    pub provider: Provider,
    /// The member who granted it.
    pub granted_by: UserId,
    /// The provider's stable id of the account (`sub`; opaque, never an
    /// address).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_subject: Option<String>,
    /// The grantor as the identity module knows them (the token subject the
    /// grant was handed over for).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grantor_subject: Option<String>,
    /// An existing connection got this grant (it was not created now).
    #[serde(default)]
    pub refreshed: bool,
}

impl DomainEvent for ConnectionAdded {
    const NAME: &'static str = "connectors.connection_added";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![connection_change(
            Change::upsert(connection_record(self.connection)),
            Some(self.granted_by),
        )]
    }
}

/// A connection gained a capability: a new connection, or an incremental
/// consent. For `messages` it names the inbox this module minted; the
/// conversations module creates it, with `granted_by` as its manager
/// (`contracts/connectors.capability_granted.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CapabilityGranted {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The capability.
    pub capability: Capability,
    /// The member who granted it.
    pub granted_by: UserId,
    /// The inbox the stream feeds (`messages` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbox_id: Option<InboxId>,
    /// The account's address and name: an [`AccountDocument`] blob.
    #[schema(value_type = BlobSchema)]
    pub account: BlobRef,
}

impl DomainEvent for CapabilityGranted {
    const NAME: &'static str = "connectors.capability_granted";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![connection_change(
            Change::upsert(connection_record(self.connection)),
            Some(self.granted_by),
        )]
    }
}

/// The account behind a connection, as the blob a [`CapabilityGranted`]
/// points at (`contracts/connection-account.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountDocument {
    /// The format version, 1.
    pub version: u16,
    /// The provider.
    pub provider: Provider,
    /// The account's email address, lower-cased.
    pub address: String,
    /// Whether the provider verified it.
    pub address_verified: bool,
    /// The account's display name, when the provider gives one.
    pub display_name: Option<String>,
}

/// The media type of an [`AccountDocument`].
pub const ACCOUNT_CONTENT_TYPE: &str =
    "application/vnd.campbooks.connection-account+json; version=1";

/// A connection's grant stopped working (revoked, expired, or its stored
/// credentials can no longer be read): the member must connect again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConnectionNeedsReauth {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The member who granted it, who can grant it again.
    pub granted_by: UserId,
    /// Why (`grant_revoked`, `credentials_unreadable`).
    pub reason: String,
}

impl DomainEvent for ConnectionNeedsReauth {
    const NAME: &'static str = "connectors.connection_needs_reauth";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![connection_change(
            Change::upsert(connection_record(self.connection)),
            Some(self.granted_by),
        )]
    }
}

/// A connection was disconnected: its grant is revoked and its streams are
/// gone. `mail` says what becomes of the inboxes it fed
/// (`contracts/connectors.connection_removed.v1.schema.json`): the
/// conversations module deletes them, or keeps them read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConnectionRemoved {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The inboxes its streams fed.
    pub inbox_ids: Vec<InboxId>,
    /// The member who had connected it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granted_by: Option<UserId>,
    /// Its provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    /// The provider's stable id of the account (the `OpenID` `sub`; opaque, never
    /// an address), so a way in that came only from this connection can be
    /// dropped with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_subject: Option<String>,
    /// Delete the mail it brought in (the default), or keep it read-only.
    #[serde(default)]
    pub mail: MailChoice,
}

impl DomainEvent for ConnectionRemoved {
    const NAME: &'static str = "connectors.connection_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![connection_change(
            Change::delete(connection_record(self.connection)),
            self.granted_by,
        )]
    }
}

/// A messages stream produced a `MessageChangeBatch` for an inbox
/// (`contracts/connectors.messages_changed.v1.schema.json`). Published in
/// the transaction that advances the stream's cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessagesChanged {
    /// The workspace.
    pub workspace: String,
    /// The conversations inbox; equals the batch's `inbox`.
    pub inbox_id: InboxId,
    /// The `MessageChangeBatch` v1 document, a blob of the workspace.
    #[schema(value_type = BlobSchema)]
    pub batch: BlobRef,
    /// The stream's cursor after this batch (opaque, never personal data).
    pub cursor: String,
}

impl DomainEvent for MessagesChanged {
    const NAME: &'static str = "connectors.messages_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How a send ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SendOutcome {
    /// The provider accepted it.
    Sent,
    /// It was not sent, or may not have been (`unknown_outcome`).
    Failed,
}

/// A send asked for by `conversations.send_requested` ended
/// (`contracts/connectors.send_completed.v1.schema.json`). Ids only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SendCompleted {
    /// The workspace.
    pub workspace: String,
    /// The inbox it was sent from.
    pub inbox_id: InboxId,
    /// The send.
    pub send_id: SendId,
    /// Sent or failed.
    pub outcome: SendOutcome,
    /// The provider's id of the sent message (sent only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_message_id: Option<String>,
    /// Why it failed (failed only): `not_connected`, `needs_reauth`,
    /// `rejected`, `too_large`, `unavailable` (nothing was sent) or
    /// `unknown_outcome` (it may have been).
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

/// `conversations.send_requested` v1, as this module reads it
/// (`contracts/conversations.send_requested.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendRequested {
    /// The workspace.
    pub workspace: String,
    /// The inbox to send from.
    pub inbox_id: InboxId,
    /// The send: recorded once.
    pub send_id: SendId,
    /// The outgoing-message v1 document, a blob of the workspace. Not
    /// trusted: the connector sends the document the conversations module
    /// recorded for the send (the outbound-requests port).
    pub message: BlobRef,
}

impl DomainEvent for SendRequested {
    const NAME: &'static str = "conversations.send_requested";
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
    /// What becomes of the mail of the connections they made: an owner or
    /// admin removing them may keep it; otherwise it is deleted.
    #[serde(default)]
    pub mail: MailChoice,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.workspace_deleting` v1, as this module reads it.
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

/// `conversations.changes_requested` v1, as this module reads it
/// (`contracts/conversations.changes_requested.v1.schema.json`): a member's
/// changes the provider should mirror. Changes from the provider never
/// produce it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The inbox the messages belong to (minted here, owned by
    /// conversations).
    pub inbox_id: InboxId,
    /// The conversation (an id of the conversations module).
    pub conversation: uuid::Uuid,
    /// Always `user`.
    pub origin: String,
    /// The changes, message by message.
    pub items: Vec<RequestedItem>,
}

impl DomainEvent for ChangesRequested {
    const NAME: &'static str = "conversations.changes_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// One message and the change asked for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedItem {
    /// The provider's id for the message.
    pub provider_message_id: String,
    /// The change.
    pub change: RequestedChange,
}

/// A change a member asked for. Kinds and streams a later contract version
/// adds read as [`Unsupported`](Self::Unsupported) and are skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestedChange {
    /// Mark read.
    MarkRead,
    /// Mark unread.
    MarkUnread,
    /// Star.
    Star,
    /// Unstar.
    Unstar,
    /// Out of the inbox.
    Archive,
    /// Back in the inbox.
    Unarchive,
    /// Put a label on.
    AddLabel {
        /// The conversations label (not used here).
        label: uuid::Uuid,
        /// The provider's key, when the provider has the label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_key: Option<String>,
    },
    /// Take a label off.
    RemoveLabel {
        /// The conversations label (not used here).
        label: uuid::Uuid,
        /// The provider's key, when the provider has the label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_key: Option<String>,
    },
    /// The conversation moved to another stream.
    MoveToStream {
        /// The stream (`primary`, `updates`, `receipts`, `newsletters`).
        stream: String,
    },
    /// A kind this version does not know.
    #[serde(other)]
    Unsupported,
}
