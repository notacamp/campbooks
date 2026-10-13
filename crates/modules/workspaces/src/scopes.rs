//! The OAuth scopes of the `workspaces` module (see `campbooks_kernel::auth`).

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<WorkspacesRead>(),
    ScopeInfo::of::<WorkspacesWrite>(),
    ScopeInfo::of::<WorkspacesAdmin>(),
];

/// `workspaces:read` — see your workspaces and their members.
#[derive(Debug, Clone, Copy)]
pub struct WorkspacesRead;

impl ScopeName for WorkspacesRead {
    const NAME: &'static str = "workspaces:read";
    const DESCRIPTION: &'static str = "See your workspaces and who belongs to them.";
}

/// `workspaces:write` — create, join and leave workspaces.
#[derive(Debug, Clone, Copy)]
pub struct WorkspacesWrite;

impl ScopeName for WorkspacesWrite {
    const NAME: &'static str = "workspaces:write";
    const DESCRIPTION: &'static str = "Create workspaces, accept invitations and leave workspaces.";
}

/// `workspaces:admin` — manage a workspace, its members and invitations.
#[derive(Debug, Clone, Copy)]
pub struct WorkspacesAdmin;

impl ScopeName for WorkspacesAdmin {
    const NAME: &'static str = "workspaces:admin";
    const DESCRIPTION: &'static str = "Manage workspaces you own or administer: settings, members, invitations, deletion and the security log.";
}
