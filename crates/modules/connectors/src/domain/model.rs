//! The module's language as types: connections, capabilities, health,
//! streams.

use campbooks_kernel::{
    Error,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

campbooks_kernel::define_id!(
    /// A connection: a member's authorisation to use one outside account.
    ConnectionId
);

campbooks_kernel::define_id!(
    /// A change stream: one connection and capability, with its cursor.
    StreamId
);

campbooks_kernel::define_id!(
    /// A started consent, waiting to be completed.
    AuthorizationId
);

campbooks_kernel::define_id!(
    /// A send asked for by the conversations module: its idempotency key.
    SendId
);

campbooks_kernel::define_id!(
    /// A conversations inbox. Minted here when a connection gains the
    /// `messages` capability; the conversations module owns the inbox.
    InboxId
);

/// An outside provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Google: Gmail today; Calendar and Drive later.
    Google,
    /// Zoho: Zoho Mail.
    Zoho,
}

impl Provider {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Zoho => "zoho",
        }
    }

    /// Parse the database name.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "google" => Ok(Self::Google),
            "zoho" => Ok(Self::Zoho),
            other => Err(Error::internal(format!("unknown provider `{other}`"))),
        }
    }
}

/// What a connection lets Campbooks do with the account.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read and tidy mail.
    Messages,
    /// Calendars and events (not offered yet).
    Calendar,
    /// Files (not offered yet).
    Files,
    /// Bank data (not offered yet).
    Bank,
}

impl Capability {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Messages => "messages",
            Self::Calendar => "calendar",
            Self::Files => "files",
            Self::Bank => "bank",
        }
    }

    /// Parse the database name.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "messages" => Ok(Self::Messages),
            "calendar" => Ok(Self::Calendar),
            "files" => Ok(Self::Files),
            "bank" => Ok(Self::Bank),
            other => Err(Error::internal(format!("unknown capability `{other}`"))),
        }
    }
}

/// What becomes of the mail a connection brought in when it is removed
/// (the owner's decision, campbooks-cloud#117): deleted with it (the
/// default), or kept as read-only history. Only the member who connected
/// it chooses when disconnecting; an owner or admin chooses when removing
/// that member.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MailChoice {
    /// Delete the inboxes it fed: messages, conversations, drafts,
    /// decisions, search entries, stored files and memberships.
    #[default]
    Delete,
    /// Keep them read-only: no sync, nothing sent, memberships kept.
    Keep,
}

/// How a connection is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Syncing normally.
    Ok,
    /// The grant was revoked or expired: the member must connect again.
    /// Its streams are paused.
    NeedsReauth,
    /// The provider asked us to slow down; syncing resumes on its own.
    Throttled,
    /// Several runs in a row failed; syncing keeps retrying.
    Failing,
}

impl Health {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NeedsReauth => "needs_reauth",
            Self::Throttled => "throttled",
            Self::Failing => "failing",
        }
    }

    /// Parse the database name.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "ok" => Ok(Self::Ok),
            "needs_reauth" => Ok(Self::NeedsReauth),
            "throttled" => Ok(Self::Throttled),
            "failing" => Ok(Self::Failing),
            other => Err(Error::internal(format!("unknown health `{other}`"))),
        }
    }
}

/// Where a stream is in its reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Reading the initial window.
    Backfill,
    /// Following changes from the cursor.
    Incremental,
    /// Re-reading the window after the provider forgot the cursor.
    Resync,
}

impl Phase {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backfill => "backfill",
            Self::Incremental => "incremental",
            Self::Resync => "resync",
        }
    }

    /// Parse the database name.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "backfill" => Ok(Self::Backfill),
            "incremental" => Ok(Self::Incremental),
            "resync" => Ok(Self::Resync),
            other => Err(Error::internal(format!("unknown phase `{other}`"))),
        }
    }
}

/// A connection as the API shows it. Never carries a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConnectionView {
    /// The connection.
    pub id: ConnectionId,
    /// The provider.
    pub provider: Provider,
    /// The account's email address.
    pub address: String,
    /// Whether the provider verified it.
    pub address_verified: bool,
    /// The account's display name, when the provider gives one.
    pub display_name: Option<String>,
    /// The member who granted it.
    pub granted_by: UserId,
    /// The caller granted it (only they may reconnect or disconnect it).
    pub granted_by_you: bool,
    /// What it lets Campbooks do.
    pub capabilities: Vec<Capability>,
    /// How it is doing.
    pub health: Health,
    /// Why, when it is not `ok` (`grant_revoked`, `rate_limited`, …).
    pub health_reason: Option<String>,
    /// Since when it has had this health.
    pub health_changed_at: DateTime<Utc>,
    /// Its change streams.
    pub streams: Vec<StreamView>,
    /// When it was connected.
    pub created_at: DateTime<Utc>,
}

/// A change stream as the API shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StreamView {
    /// The capability it reads.
    pub capability: Capability,
    /// The conversations inbox it feeds (`messages`).
    pub inbox_id: Option<InboxId>,
    /// The mailbox it reads (`messages`): one connection can reach several
    /// (a Zoho login's mail accounts).
    pub address: Option<String>,
    /// Where it is in its reading.
    pub phase: Phase,
    /// Paused until the member connects again.
    pub paused: bool,
    /// When a run last finished.
    pub last_synced_at: Option<DateTime<Utc>>,
    /// When a run last found a change.
    pub last_change_at: Option<DateTime<Utc>>,
    /// When it is next due.
    pub next_sync_at: Option<DateTime<Utc>>,
}

/// The path of one connection:
/// `/api/v1/w/{workspace_id}/connections/{connection_id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ConnectionPath {
    /// The workspace (checked by the `Workspace` extractor).
    pub workspace_id: WorkspaceId,
    /// The connection.
    pub connection_id: ConnectionId,
}
