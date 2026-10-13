//! The grant-intake port: how a mailbox grant obtained by signing in reaches
//! the module that keeps connections to outside accounts (ADR 0014).
//!
//! The `identity` module runs the provider round trip (one consent for
//! identity and mail). The `connectors` module owns connections, their
//! credentials and their sync. Modules don't import modules, so `connectors`
//! implements [`GrantIntake`] and the server registers it with
//! [`AppState::with_grant_intake`](crate::AppState::with_grant_intake).
//! Tokens cross in-process only, as [`SecretString`]s; they never enter an
//! event or a job payload.

use std::{future::Future, pin::Pin, sync::Arc};

use secrecy::SecretString;

use crate::{AppState, Error, tenancy::WorkspaceId};

/// A provider grant in hand, for one account in one workspace.
pub struct ProviderGrant {
    /// The workspace the connection belongs to.
    pub workspace: WorkspaceId,
    /// The person, by their token subject (the Campbooks account id). The
    /// implementation resolves their membership itself.
    pub subject: String,
    /// The provider: `google`, `microsoft` or `zoho`.
    pub provider: String,
    /// The provider's stable id of the account (`sub`).
    pub account_subject: String,
    /// The account's email address, as the provider gave it.
    pub address: String,
    /// Whether the provider says it verified the address.
    pub address_verified: bool,
    /// The account's display name.
    pub display_name: Option<String>,
    /// The scopes the provider granted.
    pub scopes: Vec<String>,
    /// The access token.
    pub access_token: SecretString,
    /// The refresh token (offline access).
    pub refresh_token: Option<SecretString>,
    /// Seconds until the access token expires, as the provider said.
    pub expires_in: Option<u64>,
}

impl std::fmt::Debug for ProviderGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderGrant")
            .field("workspace", &self.workspace)
            .field("provider", &self.provider)
            .field("scopes", &self.scopes)
            .field("refresh_token", &self.refresh_token.is_some())
            .finish_non_exhaustive()
    }
}

/// A future returned by [`GrantIntake::adopt`]: the connection's id.
pub type AdoptFuture<'a> = Pin<Box<dyn Future<Output = Result<String, Error>> + Send + 'a>>;

/// Takes provider grants into connections. Implemented by `connectors`.
pub trait GrantIntake: std::fmt::Debug + Send + Sync + 'static {
    /// The provider scopes this instance can turn into a mailbox connection
    /// for `provider` (empty when it has no adapter for it). A sign-in asks
    /// for these in the same consent.
    fn mailbox_scopes(&self, provider: &str) -> Vec<String>;

    /// Create the connection of `grant` in its workspace, or refresh the
    /// existing one of the same account. Errors are the implementation's
    /// problems (e.g. the account is another member's connection).
    fn adopt<'a>(&'a self, state: &'a AppState, grant: ProviderGrant) -> AdoptFuture<'a>;
}

/// The port before a module provides one: no mailbox can be connected.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoGrantIntake;

impl GrantIntake for NoGrantIntake {
    fn mailbox_scopes(&self, _provider: &str) -> Vec<String> {
        Vec::new()
    }

    fn adopt<'a>(&'a self, _state: &'a AppState, _grant: ProviderGrant) -> AdoptFuture<'a> {
        Box::pin(async { Err(Error::Conflict(crate::i18n::MESSAGES.reason("mailbox_unsupported"))) })
    }
}

/// The registered grant intake.
pub type GrantIntakePort = Arc<dyn GrantIntake>;
