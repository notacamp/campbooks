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
    /// The provider identity or passkey added (absent from events published
    /// before it was recorded).
    #[serde(default)]
    pub way_in: Option<Uuid>,
}

impl DomainEvent for WayInAdded {
    const NAME: &'static str = "identity.way_in_added";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// A person signed out everywhere: every issuer session of the account
/// ended and every refresh token it held was revoked. The web BFF ends its
/// own sessions of the account on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignedOutEverywhere {
    /// The account (the `sub` of its tokens).
    pub account: Uuid,
}

impl DomainEvent for SignedOutEverywhere {
    const NAME: &'static str = "identity.signed_out_everywhere";
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

/// A workspace registered a client of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ClientRegistered {
    /// The workspace.
    pub workspace: String,
    /// The client id.
    pub client: String,
    /// `confidential` (client credentials) or `public` (sign-in app).
    pub kind: String,
}

impl DomainEvent for ClientRegistered {
    const NAME: &'static str = "identity.client_registered";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A workspace removed a client of its own; its tokens no longer work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ClientRemoved {
    /// The workspace.
    pub workspace: String,
    /// The client id.
    pub client: String,
}

impl DomainEvent for ClientRemoved {
    const NAME: &'static str = "identity.client_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A person allowed a client (in a workspace, unless it is Campbooks' own
/// app).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConsentGranted {
    /// The account.
    pub account: Uuid,
    /// The client id.
    pub client: String,
    /// The workspace its tokens are bound to.
    pub workspace: Option<String>,
}

impl DomainEvent for ConsentGranted {
    const NAME: &'static str = "identity.consent_granted";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    // A person's, not a workspace's (Campbooks' own apps name none): the
    // workspace is in the payload.
    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// A consent ended (revoked by the person, or because they left the
/// workspace, or it was deleted); the client's tokens for them stop
/// working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConsentRevoked {
    /// The account.
    pub account: Uuid,
    /// The client id.
    pub client: String,
    /// The workspace its tokens were bound to.
    pub workspace: Option<String>,
    /// `revoked`, `member_left`, `workspace_deleting` or `client_removed`.
    pub reason: String,
}

impl DomainEvent for ConsentRevoked {
    const NAME: &'static str = "identity.consent_revoked";
    const VERSION: u16 = 1;
    const WORKSPACE_SCOPED: bool = false;

    // A person's, not a workspace's (Campbooks' own apps name none): the
    // workspace is in the payload.
    fn workspace(&self) -> Option<&str> {
        None
    }
}

/// `workspaces.member_left` v1, as this module reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member (the workspaces module's user id).
    pub user: Uuid,
    /// Their account (the subject), when the event names it.
    #[serde(default)]
    pub subject: Option<String>,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.connection_removed` v1, as this module reads it
/// (`contracts/connectors.connection_removed.v1.schema.json`): the
/// workspace, provider and account of the mailbox that went, whose way in
/// may go with it. Events recorded before the account was named carry
/// none, and change nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRemoved {
    /// The workspace.
    pub workspace: String,
    /// The connection (the connectors module's id).
    pub connection: String,
    /// Its provider.
    #[serde(default)]
    pub provider: Option<String>,
    /// The provider's stable id of the account (`sub`).
    #[serde(default)]
    pub account_subject: Option<String>,
}

impl DomainEvent for ConnectionRemoved {
    const NAME: &'static str = "connectors.connection_removed";
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
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
