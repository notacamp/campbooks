//! The domain events this module publishes, each in the transaction of the
//! change it describes. Every event belongs to its workspace; `workspace` is
//! the workspace id as a string (the event bus keys deliveries by it).
//! Each is declared in the module's `background` (`publishes`), which lists
//! it in `openapi/events.json`.

use campbooks_kernel::{
    DomainEvent,
    tenancy::{Role, UserId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::model::Kind;

/// A workspace was created; its creator is its first owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceCreated {
    /// The workspace.
    pub workspace: String,
    /// Who created it (its owner).
    pub owner: UserId,
    /// Business or personal.
    pub kind: Kind,
}

impl DomainEvent for WorkspaceCreated {
    const NAME: &'static str = "workspaces.workspace_created";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A user became a member: by creating the workspace or accepting an
/// invitation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberJoined {
    /// The workspace.
    pub workspace: String,
    /// The new member.
    pub user: UserId,
    /// Their role.
    pub role: Role,
}

impl DomainEvent for MemberJoined {
    const NAME: &'static str = "workspaces.member_joined";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A user stopped being a member: they left, or were removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
    /// Who removed them; `None` when they left.
    pub removed_by: Option<UserId>,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A member's role changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberRoleChanged {
    /// The workspace.
    pub workspace: String,
    /// The member.
    pub user: UserId,
    /// Their previous role.
    pub from: Role,
    /// Their new role.
    pub to: Role,
    /// Who changed it.
    pub changed_by: UserId,
}

impl DomainEvent for MemberRoleChanged {
    const NAME: &'static str = "workspaces.member_role_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// A workspace was deleted. Every module purges its data for it; the
/// workspace's own records are purged at `purge_after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its records are hard-deleted.
    pub purge_after: DateTime<Utc>,
    /// Who deleted it.
    pub deleted_by: UserId,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
