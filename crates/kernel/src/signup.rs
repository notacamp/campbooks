//! The invitation port: whether someone has a pending invitation, so a new
//! account can be refused on an invite-only instance before anything is
//! kept (ADR 0014). Implemented by `workspaces`, which owns invitations; the
//! server registers it with
//! [`AppState::with_invitations`](crate::AppState::with_invitations).

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgPool;

use crate::Error;

/// A future returned by [`Invitations::pending_for`].
pub type InvitedFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;

/// Answers "is this address invited somewhere?".
pub trait Invitations: std::fmt::Debug + Send + Sync + 'static {
    /// Whether `email` (lower-cased) has a pending, unexpired invitation to
    /// any workspace.
    fn pending_for<'a>(&'a self, db: &'a PgPool, email: &'a str) -> InvitedFuture<'a>;
}

/// The port before a module provides one: nobody is invited.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoInvitations;

impl Invitations for NoInvitations {
    fn pending_for<'a>(&'a self, _db: &'a PgPool, _email: &'a str) -> InvitedFuture<'a> {
        Box::pin(async { Ok(false) })
    }
}

/// The registered invitation port.
pub type InvitationsPort = Arc<dyn Invitations>;
