//! The other modules' events this module consumes, as their published
//! contracts describe them (modules never import each other). It publishes
//! none: a profile is private to its member and nothing else reacts to it.

use campbooks_kernel::{DomainEvent, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// `workspaces.workspace_deleting`: the workspace is gone at once and its
/// data is purged at `purge_after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its data goes.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_left`: a user is no longer a member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
