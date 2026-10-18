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
    /// the key) and whoever asked for it may still send from its inbox.
    /// `None` otherwise, or when there is no such send.
    fn stored_send<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        send: Uuid,
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
