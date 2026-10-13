//! The domain events the `identity` module publishes (ADR 0012), each in the
//! transaction of the change it describes. They belong to a person, not a
//! workspace, so they are instance-wide. Payloads hold ids, provider names
//! and methods; never addresses, tokens or secrets.

use campbooks_kernel::DomainEvent;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A person created an account (with its first way in).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountCreated {
    /// The account (the `sub` of its tokens).
    pub account: Uuid,
    /// The provider of its first way in.
    pub provider: String,
}

impl DomainEvent for AccountCreated {
    const NAME: &'static str = "identity.account_created";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// A person signed in to the issuer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignedIn {
    /// The account.
    pub account: Uuid,
    /// `provider` (later also `passkey`).
    pub method: String,
    /// The provider, for a provider sign-in.
    pub provider: Option<String>,
}

impl DomainEvent for SignedIn {
    const NAME: &'static str = "identity.signed_in";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// A spent authorization code or a rotated refresh token was presented
/// again; every token of that sign-in was revoked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RefreshReuseDetected {
    /// The account whose tokens were revoked.
    pub account: Uuid,
    /// The client they were issued to.
    pub client: String,
}

impl DomainEvent for RefreshReuseDetected {
    const NAME: &'static str = "identity.refresh_reuse_detected";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// An account gained a way in: a provider identity or a passkey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WayInAdded {
    /// The account.
    pub account: Uuid,
    /// `provider` or `passkey`.
    pub method: String,
    /// The provider, for a provider identity.
    pub provider: Option<String>,
    /// How it was added: `sign_up`, `link`, `connect` or `passkey`.
    pub via: String,
}

impl DomainEvent for WayInAdded {
    const NAME: &'static str = "identity.way_in_added";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// An account lost a way in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WayInRemoved {
    /// The account.
    pub account: Uuid,
    /// `provider` or `passkey`.
    pub method: String,
    /// The provider, for a provider identity.
    pub provider: Option<String>,
}

impl DomainEvent for WayInRemoved {
    const NAME: &'static str = "identity.way_in_removed";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// An administrator suspended an account: its sessions ended and its
/// refresh tokens were revoked, and it cannot sign in until it is
/// unsuspended. Consumers drop whatever else keeps it signed in (the web
/// BFF's sessions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountSuspended {
    /// The account (the `sub` of its tokens).
    pub account: Uuid,
}

impl DomainEvent for AccountSuspended {
    const NAME: &'static str = "identity.account_suspended";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}
