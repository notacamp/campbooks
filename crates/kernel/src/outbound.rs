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
