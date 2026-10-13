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
