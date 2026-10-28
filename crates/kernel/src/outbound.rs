//! The outbound-requests port: before the module that talks to outside
//! accounts acts on one because another module asked it to (send a
//! message, mirror a change), it asks the asking module whether its own
//! records still say so (campbooks-cloud#226).
//!
//! Any transaction working in a workspace can append that workspace's
//! events, so an event alone is not proof that a member asked for
//! anything. The module that owns the request (the `conversations` module
//! for sends and changes) implements [`OutboundRequests`]; the server
//! registers it with
//! [`AppState::with_outbound`](crate::AppState::with_outbound). Until then
//! nothing is ever confirmed, so nothing is sent or changed.
//!
//! Where a send goes is bound too (campbooks-cloud#242): the module that
//! talks to outside accounts answers which connection, stream and mailbox
//! an inbox goes out through ([`OutboundDestinations`], registered with
//! [`AppState::with_destinations`](crate::AppState::with_destinations)), the
//! asking module binds that [`Destination`] into the send's MACs when the
//! member asks and when the send is released, and the sender passes the
//! destination it resolved itself when it asks whether the send stands
//! ([`OutboundRequests::stored_send`]): a send bound to another destination
//! does not stand.

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgConnection;
use uuid::Uuid;

use crate::{AppState, Error, blobs::BlobRef, tenancy::WorkspaceId};

/// A send as the module that asked for it stored it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSend {
    /// The container it is sent from (an inbox of the asking module).
    pub inbox: Uuid,
    /// The document to send, as the asking module wrote it. The sender
    /// sends this, never a document an event names.
    pub message: BlobRef,
}

/// Where a send or a change goes at the provider (campbooks-cloud#242): the
/// connection whose grant makes it, the stream it goes through and that
/// stream's mailbox at the provider (Zoho's account id; empty for Gmail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// The connection (an id of the module that talks to outside accounts).
    pub connection: Uuid,
    /// The stream.
    pub stream: Uuid,
    /// The stream's mailbox key at the provider.
    pub mailbox: String,
}

/// An inbox's [`Destination`] as resolved now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Where it goes.
    pub destination: Destination,
    /// Whether it is the destination the inbox had when destinations were
    /// first bound, as the migration that started binding them recorded it
    /// (a table the runtime role cannot write). Only then may a binding made
    /// before (which names no destination) still verify. Never bound itself.
    pub as_at_deploy: bool,
    /// The most bytes of attachments one message from here can carry, when
    /// the provider takes less than the asking module's own limit
    /// (campbooks-cloud#391): an upload past it is refused, rather than a
    /// send failing after its undo window. `None`: no lower limit known.
    /// Never bound.
    pub max_attachment_bytes: Option<u64>,
}

/// A future returned by [`OutboundDestinations::destination`].
pub type DestinationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Resolved>, Error>> + Send + 'a>>;

/// Answers where an inbox's sends go: the module that talks to outside
/// accounts implements it, the module that asks for sends binds what it
/// answers (campbooks-cloud#242).
pub trait OutboundDestinations: std::fmt::Debug + Send + Sync + 'static {
    /// The destination of `inbox` (a container of the asking module), read
    /// in `conn`, a transaction working in the inbox's workspace. `None`
    /// when no connection feeds it, or when more than one stream claims it.
    fn destination<'a>(&'a self, conn: &'a mut PgConnection, inbox: Uuid) -> DestinationFuture<'a>;
}

/// The port before a module provides one: no inbox goes anywhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDestinations;

impl OutboundDestinations for NoDestinations {
    fn destination<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _inbox: Uuid,
    ) -> DestinationFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`OutboundDestinations`] port.
pub type DestinationsPort = Arc<dyn OutboundDestinations>;

/// A future returned by [`OutboundRequests::stored_send`].
pub type StoredSendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<StoredSend>, Error>> + Send + 'a>>;

/// A future returned by [`OutboundRequests::changes_standing`]: one answer
/// per item asked about, in order.
pub type StandingFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<bool>, Error>> + Send + 'a>>;

/// Answers whether a send or a change another module is about to make at
/// the provider was really asked for.
pub trait OutboundRequests: std::fmt::Debug + Send + Sync + 'static {
    /// Send `send` of `workspace`, read in `conn` (a transaction working in
    /// that workspace): `Some` only while it waits for the provider (released,
    /// not sent, failed or cancelled), its stored binding verifies
    /// ([`state.binding`](crate::binding): the release that wrote it held
    /// the key) for `at`, the destination the sender resolved for it
    /// (`None`: none), and whoever asked for it may still send from its
    /// inbox. `None` otherwise, or when there is no such send.
    fn stored_send<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        send: Uuid,
        at: Option<&'a Resolved>,
    ) -> StoredSendFuture<'a>;

    /// For each of `items` (one requested change, as the asking module's
    /// event writes it: `{provider_message_id, change}`), whether the asking
    /// module recorded that change as part of request `request` (the id of
    /// the event that carried it) for `conversation` in `inbox`. Read in
    /// `conn`, a transaction working in `workspace`. An item that does not
    /// parse, or that the request did not hold, is `false`; so is every
    /// item of a recorded request whose binding
    /// ([`state.binding`](crate::binding)) does not verify.
    // The request is named by its parts (id, inbox, conversation, items),
    // as the event carries them.
    #[allow(clippy::too_many_arguments)]
    fn changes_standing<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
        inbox: Uuid,
        conversation: Uuid,
        items: &'a [serde_json::Value],
    ) -> StandingFuture<'a>;
}

/// The port before a module provides one: nothing was asked for.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOutboundRequests;

impl OutboundRequests for NoOutboundRequests {
    fn stored_send<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _send: Uuid,
        _at: Option<&'a Resolved>,
    ) -> StoredSendFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    fn changes_standing<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _request: Uuid,
        _inbox: Uuid,
        _conversation: Uuid,
        items: &'a [serde_json::Value],
    ) -> StandingFuture<'a> {
        Box::pin(async move { Ok(vec![false; items.len()]) })
    }
}

/// Shared handle to the registered [`OutboundRequests`] port.
pub type OutboundPort = Arc<dyn OutboundRequests>;

/// A calendar change as the schedule module stored it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCalendarChange {
    /// The calendar the event is on (an id of the schedule module).
    pub calendar: Uuid,
    /// The event.
    pub event: Uuid,
    /// The connection whose grant makes the change.
    pub connection: Uuid,
    /// The `calendar-change` document, as the schedule module wrote it. The
    /// connector writes this, never a document an event names.
    pub body: BlobRef,
}

/// A future returned by [`CalendarRequests::stored_change`].
pub type StoredCalendarChangeFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<StoredCalendarChange>, Error>> + Send + 'a>>;

/// Answers whether a change to an event on a provider calendar was really
/// asked for, before the connector makes it: the schedule module's side of
/// the outbound-requests port, as [`OutboundRequests`] is the conversations
/// module's. Any transaction working in a workspace can append that
/// workspace's events, so `schedule.outbound_requested` alone proves nothing.
pub trait CalendarRequests: std::fmt::Debug + Send + Sync + 'static {
    /// The change of request `request` in `workspace`, read in `conn` (a
    /// transaction working in that workspace): `Some` only while it waits for
    /// the provider, its stored binding verifies
    /// ([`state.binding`](crate::binding)), and the member who asked may
    /// still change that calendar. `None` otherwise, or when there is no such
    /// request.
    fn stored_change<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> StoredCalendarChangeFuture<'a>;
}

/// The port before a module provides one: no calendar change was asked for.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCalendarRequests;

impl CalendarRequests for NoCalendarRequests {
    fn stored_change<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _request: Uuid,
    ) -> StoredCalendarChangeFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`CalendarRequests`] port.
pub type CalendarPort = Arc<dyn CalendarRequests>;

/// What a block change asked for in a page that is kept in step with an
/// outside page does to the block there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageItemKind {
    /// Rewrite a block that is there.
    Update,
    /// Add a block.
    Append,
    /// Remove a block the member deleted here.
    Delete,
}

/// One block change of a [`StoredPageChanges`], as the pages module stored it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPageItem {
    /// What to do.
    pub kind: PageItemKind,
    /// The block's id in the pages module.
    pub block: Uuid,
    /// The block's id at the outside page (update, delete).
    pub external: Option<Uuid>,
    /// The block or page at the outside account it goes under (append);
    /// none when it goes under the block of this request `parent_block`
    /// names, or under the page itself.
    pub parent: Option<Uuid>,
    /// The block of this request it goes under, when that one is new too
    /// (append).
    pub parent_block: Option<Uuid>,
    /// The block at the outside page it goes after (append); none: first.
    pub after: Option<Uuid>,
    /// The block of this request it goes after, when that one is new too
    /// (append): its id at the outside page is known once it is made.
    pub after_block: Option<Uuid>,
    /// When the block was last edited at the outside page as of the last
    /// sync: a block edited since is not written (update, delete).
    pub base_edited_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A page's block changes as the pages module stored them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPageChanges {
    /// The page (an id of the pages module).
    pub page: Uuid,
    /// The connection whose grant makes the changes.
    pub connection: Uuid,
    /// The page at the outside account.
    pub external_page: Uuid,
    /// The changes, in the order they are made.
    pub items: Vec<StoredPageItem>,
    /// The document holding what to write, as the pages module wrote it.
    /// The connector writes this, never a document an event names.
    pub body: BlobRef,
}

/// What a table change asked for in a table kept in step with an outside
/// database does there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableItemKind {
    /// Add a column.
    SchemaAdd,
    /// Rename a column, change its type or its options.
    SchemaUpdate,
    /// Remove a column the member removed here and confirmed.
    SchemaRemove,
    /// Add a row.
    RowCreate,
    /// Set one cell of a row.
    CellUpdate,
    /// Move a row the member deleted here to the outside trash.
    RowTrash,
}

/// One change of a [`StoredTableChanges`], as the pages module stored it:
/// ids, kinds, times and hashes. Never a name, an option or a value (those
/// are in the request's document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTableItem {
    /// The change's place in the request.
    pub seq: u32,
    /// What to do.
    pub kind: TableItemKind,
    /// The column (a schema or cell change) or the row (row create or trash)
    /// in the pages module.
    pub subject: Uuid,
    /// The column in the pages module a cell change is in.
    pub property: Option<Uuid>,
    /// The column at the outside database (schema update and remove, cell
    /// update): an id the outside service gave it.
    pub external_property: Option<String>,
    /// The row at the outside database (cell update, row trash).
    pub external_page: Option<Uuid>,
    /// A hash of what the outside service held for the column or the cell
    /// as of the last sync: one that moved is not written over.
    pub base_hash: Option<String>,
    /// A hash of the outside database's whole schema as of the last sync.
    pub schema_hash: Option<String>,
    /// When the outside service last edited the row as of the last sync (row
    /// trash): a row edited since is not trashed.
    pub base_edited_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The member confirmed this removal here and at the outside database.
    pub confirmed: bool,
}

/// A table's changes as the pages module stored them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTableChanges {
    /// The table (an id of the pages module).
    pub table: Uuid,
    /// The connection whose grant makes the changes.
    pub connection: Uuid,
    /// The database at the outside account.
    pub external: Uuid,
    /// The changes, in the order they are made.
    pub items: Vec<StoredTableItem>,
    /// The document holding what to write, as the pages module wrote it.
    /// The connector writes this, never a document an event names.
    pub body: BlobRef,
}

/// A future returned by [`PageRequests::stored_table_changes`].
pub type StoredTableChangesFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<StoredTableChanges>, Error>> + Send + 'a>>;

/// A future returned by [`PageRequests::stored_changes`].
pub type StoredPageChangesFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<StoredPageChanges>, Error>> + Send + 'a>>;

/// A future returned by [`PageRequests::link_standing`].
pub type LinkStandingFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;

/// Answers whether a change to a page kept in step with an outside page, or
/// the link itself, was really asked for, before the connector acts on it:
/// the pages module's side of the outbound-requests port. Any transaction
/// working in a workspace can append that workspace's events, so
/// `pages.notion_changes_requested` alone proves nothing.
pub trait PageRequests: std::fmt::Debug + Send + Sync + 'static {
    /// The changes of request `request` in `workspace`, read in `conn` (a
    /// transaction working in that workspace): `Some` only while they wait
    /// for the connector, their stored binding verifies
    /// ([`state.binding`](crate::binding)), and the page's link to the
    /// outside page is syncing. `None` otherwise, or when there is no such
    /// request.
    fn stored_changes<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> StoredPageChangesFuture<'a>;

    /// Whether `page` of `workspace` is linked to `external_page` through
    /// `connection` and the link is syncing, read in `conn`.
    fn link_standing<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        page: Uuid,
        connection: Uuid,
        external_page: Uuid,
    ) -> LinkStandingFuture<'a>;

    /// Whether `table` of `workspace` is linked to the outside database
    /// (data source) `external` through `connection` and the link is
    /// syncing, read in `conn`.
    fn table_link_standing<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        table: Uuid,
        connection: Uuid,
        external: Uuid,
    ) -> LinkStandingFuture<'a>;

    /// The table changes of request `request` in `workspace`, read in
    /// `conn` (a transaction working in that workspace): `Some` only while
    /// they wait for the connector, their stored binding verifies
    /// ([`state.binding`](crate::binding)), and the table's link to the
    /// outside database is syncing. `None` otherwise, or when there is no
    /// such request.
    fn stored_table_changes<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> StoredTableChangesFuture<'a>;
}

/// The port before a module provides one: no page change was asked for and
/// no link stands.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPageRequests;

impl PageRequests for NoPageRequests {
    fn stored_changes<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _request: Uuid,
    ) -> StoredPageChangesFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    fn link_standing<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _page: Uuid,
        _connection: Uuid,
        _external_page: Uuid,
    ) -> LinkStandingFuture<'a> {
        Box::pin(async { Ok(false) })
    }

    fn table_link_standing<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _table: Uuid,
        _connection: Uuid,
        _external: Uuid,
    ) -> LinkStandingFuture<'a> {
        Box::pin(async { Ok(false) })
    }

    fn stored_table_changes<'a>(
        &'a self,
        _state: &'a AppState,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _request: Uuid,
    ) -> StoredTableChangesFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`PageRequests`] port.
pub type PageRequestsPort = Arc<dyn PageRequests>;
