//! The events this module consumes, as it reads them (the fields it needs;
//! the publishers' payloads have more). It publishes none: search results
//! are not change-feed records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use campbooks_kernel::DomainEvent;

/// The record type of a conversation (the group search shows).
pub const CONVERSATION_RECORD: &str = "conversations.conversation";

/// The record type of a page (a group search shows).
pub const PAGE_RECORD: &str = "pages.page";

/// `conversations.message_received` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageReceived {
    /// The workspace.
    pub workspace: String,
    /// The message's conversation.
    pub conversation: String,
}

impl DomainEvent for MessageReceived {
    const NAME: &'static str = "conversations.message_received";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `conversations.message_sent` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSent {
    /// The workspace.
    pub workspace: String,
    /// The message's conversation.
    pub conversation: String,
}

impl DomainEvent for MessageSent {
    const NAME: &'static str = "conversations.message_sent";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `conversations.conversation_changed` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationChanged {
    /// The workspace.
    pub workspace: String,
    /// The conversation.
    pub conversation: String,
}

impl DomainEvent for ConversationChanged {
    const NAME: &'static str = "conversations.conversation_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.page_changed` v1 (`contracts/pages.page_changed.v1.schema.json`):
/// a page was made or changed, put in the trash or brought back, shared or
/// deleted for good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageChanged {
    /// The workspace.
    pub workspace: String,
    /// The page (a group of the index).
    pub page: String,
}

impl DomainEvent for PageChanged {
    const NAME: &'static str = "pages.page_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.workspace_deleting` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its records are hard-deleted.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `conversations.inbox_removed` v1: an inbox was deleted with everything
/// in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxRemoved {
    /// The workspace.
    pub workspace: String,
    /// The inbox (a container of the search index).
    pub inbox: String,
}

impl DomainEvent for InboxRemoved {
    const NAME: &'static str = "conversations.inbox_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
