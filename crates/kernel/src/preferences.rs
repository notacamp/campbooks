//! The member's standing preferences other modules act on.
//!
//! [`MemberPreferences`] answers "may Campbooks act without asking first?":
//! when a module has evidence for a change a member did not ask for in so
//! many words (a rule Campbooks learned from their corrections, a pattern it
//! noticed), it asks this port before acting. The `assistant` module owns the
//! answer (an onboarding question); until the server registers it, the
//! answer is always "ask first".

use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::PgConnection;

use crate::{
    Error,
    tenancy::{UserId, WorkspaceId},
};

/// A future returned by a [`MemberPreferences`] port.
pub type PreferenceFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + 'a>>;

/// The port that answers a member's standing preferences.
pub trait MemberPreferences: std::fmt::Debug + Send + Sync + 'static {
    /// Whether `member` of `workspace` lets Campbooks act on what it is sure
    /// of and tell them afterwards (`true`), rather than ask first (`false`).
    /// Read inside the caller's transaction `tx`.
    fn acts_without_asking<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        workspace: WorkspaceId,
        member: UserId,
    ) -> PreferenceFuture<'a>;
}

/// The port before any module provides one: always ask first.
#[derive(Debug, Clone, Copy, Default)]
pub struct AskFirst;

impl MemberPreferences for AskFirst {
    fn acts_without_asking<'a>(
        &'a self,
        _tx: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _member: UserId,
    ) -> PreferenceFuture<'a> {
        Box::pin(async { Ok(false) })
    }
}

/// Shared handle to the registered [`MemberPreferences`] port.
pub type MemberPreferencesPort = Arc<dyn MemberPreferences>;
