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
//!
//! The other way round, `identity` is the one front door for every provider
//! consent, including the ones `connectors` starts: `identity` implements
//! [`ConsentDoor`] and the server registers it with
//! [`AppState::with_consent_door`](crate::AppState::with_consent_door).

use std::{future::Future, pin::Pin, sync::Arc};

use secrecy::SecretString;

use chrono::{DateTime, Utc};

use crate::{AppState, Error, auth::Principal, tenancy::WorkspaceId};

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
    /// The connection this grant re-authorises (a reconnect): the same
    /// account must have consented.
    pub extending: Option<String>,
}

impl std::fmt::Debug for ProviderGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderGrant")
            .field("workspace", &self.workspace)
            .field("provider", &self.provider)
            .field("scopes", &self.scopes)
            .field("refresh_token", &self.refresh_token.is_some())
            .field("extending", &self.extending)
            .finish_non_exhaustive()
    }
}

/// A future returned by [`GrantIntake::adopt`]: the connection's id.
pub type AdoptFuture<'a> = Pin<Box<dyn Future<Output = Result<String, Error>> + Send + 'a>>;

/// A connection someone may reconnect, as [`GrantIntake::reconnect_target`]
/// describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectTarget {
    /// Its provider.
    pub provider: String,
    /// Its mailbox address (to suggest the account at the provider).
    pub address: String,
}

/// A future returned by [`GrantIntake::reconnect_target`].
pub type TargetFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ReconnectTarget, Error>> + Send + 'a>>;

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

    /// The connection `connection` of workspace `workspace`, when the person
    /// with token subject `subject` granted it there and may consent to it
    /// again; [`Error::NotFound`] otherwise.
    fn reconnect_target<'a>(
        &'a self,
        state: &'a AppState,
        subject: &'a str,
        workspace: WorkspaceId,
        connection: &'a str,
    ) -> TargetFuture<'a>;
}

/// The port before a module provides one: no mailbox can be connected.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoGrantIntake;

impl GrantIntake for NoGrantIntake {
    fn mailbox_scopes(&self, _provider: &str) -> Vec<String> {
        Vec::new()
    }

    fn adopt<'a>(&'a self, _state: &'a AppState, _grant: ProviderGrant) -> AdoptFuture<'a> {
        Box::pin(async {
            Err(Error::Conflict(
                crate::i18n::MESSAGES.reason("mailbox_unsupported"),
            ))
        })
    }

    fn reconnect_target<'a>(
        &'a self,
        _state: &'a AppState,
        _subject: &'a str,
        _workspace: WorkspaceId,
        _connection: &'a str,
    ) -> TargetFuture<'a> {
        Box::pin(async { Err(Error::NotFound) })
    }
}

/// The registered grant intake.
pub type GrantIntakePort = Arc<dyn GrantIntake>;

/// A mailbox consent another module asks the front door to start.
#[derive(Debug, Clone)]
pub struct ConsentRequest {
    /// Who asks: the signed-in person, with their token's `auth_time`.
    pub principal: Principal,
    /// The workspace the mailbox is for.
    pub workspace: WorkspaceId,
    /// The provider.
    pub provider: String,
    /// The connection to reconnect, if any (a new connection otherwise).
    pub connection: Option<String>,
}

/// A started consent: send the browser to `url` before `expires_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConsent {
    /// Same-origin URL of the front door.
    pub url: String,
    /// When it stops working.
    pub expires_at: DateTime<Utc>,
}

/// A future returned by [`ConsentDoor::start`].
pub type ConsentFuture<'a> =
    Pin<Box<dyn Future<Output = Result<StartedConsent, Error>> + Send + 'a>>;

/// The one front door for provider consent. Implemented by `identity`.
pub trait ConsentDoor: std::fmt::Debug + Send + Sync + 'static {
    /// Start the consent `request` describes. The grant arrives later
    /// through [`GrantIntake::adopt`]. Errors are the implementation's
    /// problems (a stale sign-in, an unknown provider, …).
    fn start<'a>(&'a self, state: &'a AppState, request: ConsentRequest) -> ConsentFuture<'a>;
}

/// The port before a module provides one: no consent can start.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoConsentDoor;

impl ConsentDoor for NoConsentDoor {
    fn start<'a>(&'a self, _state: &'a AppState, _request: ConsentRequest) -> ConsentFuture<'a> {
        Box::pin(async {
            Err(Error::Conflict(
                crate::i18n::MESSAGES.reason("mailbox_unsupported"),
            ))
        })
    }
}

/// The registered consent door.
pub type ConsentDoorPort = Arc<dyn ConsentDoor>;
