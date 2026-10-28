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
    /// The account's calendars: a `calendar-list` v1 blob (`calendar` only).
    /// The provider's calendar ids can be addresses, so they travel in the
    /// blob and never in the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<BlobSchema>)]
    pub calendars: Option<BlobRef>,
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

/// A calendar stream produced a `calendar-change-batch` for one provider
/// calendar (`contracts/connectors.calendar_changed.v1.schema.json`).
/// Published in the transaction that advances the stream's cursor; the
/// schedule module applies it. Declared with the contract, published by the
/// Google Calendar adapter (campbooks-cloud#42).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CalendarChanged {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The `calendar-change-batch` v1 document, a blob of the workspace.
    #[schema(value_type = BlobSchema)]
    pub batch: BlobRef,
}

impl DomainEvent for CalendarChanged {
    const NAME: &'static str = "connectors.calendar_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How a calendar change asked for by `schedule.outbound_requested` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalendarOutcome {
    /// Written at the provider.
    Applied,
    /// The event had changed at the provider (412).
    Superseded,
    /// Nothing was written.
    Skipped,
    /// It failed.
    Failed,
}

/// Why a calendar change did not apply: a closed set, never the provider's
/// words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalendarReason {
    /// The event changed at the provider.
    ChangedAtProvider,
    /// The schedule module's records did not confirm the request.
    Unverified,
    /// The connection is gone.
    NotConnected,
    /// The grant needs the member again.
    NeedsReauth,
    /// The provider refused it.
    Rejected,
    /// The event is not at the provider.
    NotFound,
    /// The provider could not be reached.
    Unavailable,
    /// The calendar API is not enabled for the app.
    ApiNotEnabled,
    /// It was tried the most times allowed.
    TooManyAttempts,
}

/// A calendar change ended
/// (`contracts/connectors.calendar_change_completed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CalendarChangeCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request (an id of the schedule module).
    pub request: uuid::Uuid,
    /// The calendar (an id of the schedule module).
    pub calendar: uuid::Uuid,
    /// The event (an id of the schedule module).
    pub event: uuid::Uuid,
    /// How it ended.
    pub outcome: CalendarOutcome,
    /// The event's id at the provider (applied only; opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_event_id: Option<String>,
    /// The new etag (applied only; opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Why it did not apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CalendarReason>,
}

impl DomainEvent for CalendarChangeCompleted {
    const NAME: &'static str = "connectors.calendar_change_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A Notion page's blocks were read
/// (`contracts/connectors.notion_page_read.v1.schema.json`): the snapshot
/// the pages module applies to the page linked to it. Published in the
/// transaction that advances the stream's cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionPageRead {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: ConnectionId,
    /// The Notion page.
    pub notion_page: uuid::Uuid,
    /// The pages module's page it is linked to.
    pub page: uuid::Uuid,
    /// The `notion-page-snapshot` v1 document, a blob of the workspace.
    #[schema(value_type = BlobSchema)]
    pub snapshot: BlobRef,
    /// When it was read: a later read of the page has a later time.
    pub read_at: DateTime<Utc>,
}

impl DomainEvent for NotionPageRead {
    const NAME: &'static str = "connectors.notion_page_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// Why a Notion page is no longer read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotionStop {
    /// Notion no longer shares the page with the integration.
    NotShared,
    /// The page went to the trash in Notion.
    TrashedInNotion,
    /// Notion is not offered on this instance any more.
    NotionUnavailable,
}

/// A Notion page is no longer read
/// (`contracts/connectors.notion_link_stopped.v1.schema.json`): its stream
/// is gone and the pages module ends the link. Nothing is deleted at Notion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionLinkStopped {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: ConnectionId,
    /// The Notion page.
    pub notion_page: uuid::Uuid,
    /// The pages module's page it was linked to.
    pub page: uuid::Uuid,
    /// Why.
    pub reason: NotionStop,
}

impl DomainEvent for NotionLinkStopped {
    const NAME: &'static str = "connectors.notion_link_stopped";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How a block change asked for by `pages.notion_changes_requested` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotionOutcome {
    /// Written at Notion.
    Applied,
    /// Nothing was written.
    Skipped,
    /// It failed.
    Failed,
}

/// Why a block change did not apply: a closed set, never Notion's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotionReason {
    /// The block was edited at Notion since the last sync.
    ChangedInNotion,
    /// The pages module's records did not confirm the request.
    Unverified,
    /// The connection or the link is gone.
    NotConnected,
    /// The grant needs the member again.
    NeedsReauth,
    /// Notion refused it.
    Rejected,
    /// The block is not at Notion.
    NotFound,
    /// Notion could not be reached.
    Unavailable,
    /// It was tried the most times allowed.
    TooManyAttempts,
    /// A kind of block that is never written.
    Unsupported,
    /// Removing a column at Notion needs the member's confirmation, and the
    /// change did not carry it.
    ConfirmationRequired,
}

/// How one block change of a request ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionItemResult {
    /// The block (an id of the pages module).
    pub block: uuid::Uuid,
    /// How it ended.
    pub outcome: NotionOutcome,
    /// The block's id at Notion (applied; for an added block, the new one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notion_block: Option<uuid::Uuid>,
    /// When Notion last edited the block after the write (applied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<DateTime<Utc>>,
    /// Why it did not apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<NotionReason>,
}

/// A request of block changes ended
/// (`contracts/connectors.notion_changes_completed.v1.schema.json`): every
/// item of the pages module's request has an end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionChangesCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request (an id of the pages module).
    pub request: uuid::Uuid,
    /// The page (an id of the pages module).
    pub page: uuid::Uuid,
    /// How each item ended, in the request's order.
    pub results: Vec<NotionItemResult>,
}

impl DomainEvent for NotionChangesCompleted {
    const NAME: &'static str = "connectors.notion_changes_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_link_requested` v1, as this module reads it
/// (`contracts/pages.notion_link_requested.v1.schema.json`): a page was
/// linked to a Notion page, which now needs a stream. Not proof that anyone
/// asked: the pages module is asked through the kernel's page-requests port
/// whether the link stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionLinkRequested {
    /// The workspace.
    pub workspace: String,
    /// The page (an id of the pages module).
    pub page: uuid::Uuid,
    /// The Notion connection it is read through.
    pub connection: ConnectionId,
    /// The Notion page, or the Notion database (data source) when `kind` says
    /// so.
    pub notion_page: uuid::Uuid,
    /// What is linked: a page (the default) or a database as a table.
    #[serde(default)]
    pub kind: LinkKind,
}

/// What a Notion link links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// A page, kept in step block by block.
    #[default]
    Page,
    /// A Notion database (a data source), kept in step as a table.
    DataSource,
}

impl DomainEvent for NotionLinkRequested {
    const NAME: &'static str = "pages.notion_link_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_link_ended` v1, as this module reads it
/// (`contracts/pages.notion_link_ended.v1.schema.json`): a page stopped
/// being kept in step with its Notion page, so its stream goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionLinkEnded {
    /// The workspace.
    pub workspace: String,
    /// The page (an id of the pages module).
    pub page: uuid::Uuid,
    /// The Notion connection.
    pub connection: ConnectionId,
    /// The Notion page.
    pub notion_page: uuid::Uuid,
}

impl DomainEvent for NotionLinkEnded {
    const NAME: &'static str = "pages.notion_link_ended";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_changes_requested` v1, as this module reads it
/// (`contracts/pages.notion_changes_requested.v1.schema.json`): block
/// changes a member's edits asked for. Not proof that anyone asked
/// (campbooks-cloud#227): the pages module is asked through the kernel's
/// page-requests port before anything is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The request: recorded once.
    pub request: uuid::Uuid,
    /// The page (an id of the pages module).
    pub page: uuid::Uuid,
    /// The Notion connection whose grant makes the changes.
    pub connection: ConnectionId,
    /// The Notion page.
    pub notion_page: uuid::Uuid,
    /// The `notion-block-changes` v1 document. Not trusted: the connector
    /// writes the document the pages module stored for the request.
    pub body: BlobRef,
}

impl DomainEvent for NotionChangesRequested {
    const NAME: &'static str = "pages.notion_changes_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.notion_data_source_read` v1
/// (`contracts/connectors.notion_data_source_read.v1.schema.json`): a Notion
/// database was read, and the pages module applies the snapshot to the table
/// linked to it. Ids, a blob reference and a time; never a name or a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionDataSourceRead {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: ConnectionId,
    /// The data source at Notion.
    pub data_source: uuid::Uuid,
    /// The pages module's table it is linked to.
    pub table: uuid::Uuid,
    /// The `notion-data-source-snapshot` v1 document, a blob of the
    /// workspace.
    #[schema(value_type = BlobSchema)]
    pub snapshot: BlobRef,
    /// When it was read: a later read has a later time.
    pub read_at: DateTime<Utc>,
}

impl DomainEvent for NotionDataSourceRead {
    const NAME: &'static str = "connectors.notion_data_source_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// What a table change of a request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotionTableChange {
    /// A column added.
    SchemaAdd,
    /// A column renamed, retyped or given options.
    SchemaUpdate,
    /// A column removed.
    SchemaRemove,
    /// A row added.
    RowCreate,
    /// A cell set.
    CellUpdate,
    /// A row moved to Notion's trash.
    RowTrash,
}

impl NotionTableChange {
    /// The kind kept in the database as `name`, when it is one.
    pub fn of(name: &str) -> Option<Self> {
        Some(match name {
            "schema_add" => Self::SchemaAdd,
            "schema_update" => Self::SchemaUpdate,
            "schema_remove" => Self::SchemaRemove,
            "row_create" => Self::RowCreate,
            "cell_update" => Self::CellUpdate,
            "row_trash" => Self::RowTrash,
            _ => return None,
        })
    }
}

/// How one table change of a request ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionTableItemResult {
    /// The column or row (an id of the pages module).
    pub subject: uuid::Uuid,
    /// The kind of change.
    pub kind: NotionTableChange,
    /// How it ended.
    pub outcome: NotionOutcome,
    /// The column's or the row's id at Notion (applied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
    /// When Notion last edited it after the write (applied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<DateTime<Utc>>,
    /// A hash of what Notion holds now (applied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Why it did not apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<NotionReason>,
}

/// A request of table changes ended
/// (`contracts/connectors.notion_table_changes_completed.v1.schema.json`):
/// every item of the pages module's request has an end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionTableChangesCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request (an id of the pages module).
    pub request: uuid::Uuid,
    /// The table (an id of the pages module).
    pub table: uuid::Uuid,
    /// How each item ended, in the request's order.
    pub results: Vec<NotionTableItemResult>,
}

impl DomainEvent for NotionTableChangesCompleted {
    const NAME: &'static str = "connectors.notion_table_changes_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_table_changes_requested` v1, as this module reads it
/// (`contracts/pages.notion_table_changes_requested.v1.schema.json`): table
/// changes a member's edits asked for. Not proof that anyone asked: the pages
/// module is asked through the kernel's page-requests port before anything is
/// recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionTableChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The request: recorded once.
    pub request: uuid::Uuid,
    /// The table (an id of the pages module).
    pub table: uuid::Uuid,
    /// The Notion connection whose grant makes the changes.
    pub connection: ConnectionId,
    /// The data source at Notion.
    pub data_source: uuid::Uuid,
    /// The `notion-table-changes` v1 document. Not trusted: the connector
    /// writes the document the pages module stored for the request.
    pub body: BlobRef,
}

impl DomainEvent for NotionTableChangesRequested {
    const NAME: &'static str = "pages.notion_table_changes_requested";
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

/// `schedule.outbound_requested` v1, as this module reads it
/// (`contracts/schedule.outbound_requested.v1.schema.json`): a member's
/// change to an event on a provider calendar. Not proof that anyone asked
/// (campbooks-cloud#227): the schedule module is asked through the kernel's
/// calendar-requests port before anything is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundRequested {
    /// The workspace.
    pub workspace: String,
    /// The request: recorded once.
    pub request: uuid::Uuid,
    /// The calendar (an id of the schedule module).
    pub calendar: uuid::Uuid,
    /// The event (an id of the schedule module).
    pub event: uuid::Uuid,
    /// The connection whose grant makes the change.
    pub connection: ConnectionId,
    /// The kind of change.
    pub change: String,
    /// The `calendar-change` v1 document. Not trusted: the connector writes
    /// the document the schedule module stored for the request.
    pub body: BlobRef,
}

impl DomainEvent for OutboundRequested {
    const NAME: &'static str = "schedule.outbound_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `schedule.calendar_sync_changed` v1, as this module reads it
/// (`contracts/schedule.calendar_sync_changed.v1.schema.json`): a member
/// turned a provider calendar on or off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarSyncChanged {
    /// The workspace.
    pub workspace: String,
    /// The calendar (an id of the schedule module).
    pub calendar: uuid::Uuid,
    /// The connection whose calendar it is.
    pub connection: ConnectionId,
    /// Whether it syncs now.
    pub syncing: bool,
    /// Which calendar of the provider it is: a `calendar-ref` v1 document
    /// (a blob), since the provider's id can be an address.
    #[serde(default)]
    pub provider_calendar: Option<BlobRef>,
}

impl DomainEvent for CalendarSyncChanged {
    const NAME: &'static str = "schedule.calendar_sync_changed";
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
    /// Into the trash.
    Trash,
    /// Out of the trash, back where it was.
    Untrash {
        /// Mail the inbox received: where the provider cannot tell where it
        /// was, it goes to the inbox.
        #[serde(default)]
        inbound: bool,
    },
    /// Reported as spam: into the spam folder, out of the inbox.
    Spam,
    /// Not spam: out of the spam folder, and back in the inbox when it is
    /// mail the inbox received.
    Unspam {
        /// Mail the inbox received.
        #[serde(default)]
        inbound: bool,
    },
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
