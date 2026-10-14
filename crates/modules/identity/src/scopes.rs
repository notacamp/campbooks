//! The OAuth scopes of the `identity` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with every client: never rename one.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<IdentityRead>(),
    ScopeInfo::of::<IdentityWrite>(),
    ScopeInfo::of::<IdentityClients>(),
];

/// `identity:read` — see your account and what waits on it.
#[derive(Debug, Clone, Copy)]
pub struct IdentityRead;

impl ScopeName for IdentityRead {
    const NAME: &'static str = "identity:read";
    const DESCRIPTION: &'static str =
        "See your account, its ways in and the mailboxes waiting to be connected.";
}

/// `identity:write` — change your account.
#[derive(Debug, Clone, Copy)]
pub struct IdentityWrite;

impl ScopeName for IdentityWrite {
    const NAME: &'static str = "identity:write";
    const DESCRIPTION: &'static str =
        "Change your account and connect or discard the mailboxes waiting on it.";
}

/// `identity:clients` — manage a workspace's own apps and their secrets.
#[derive(Debug, Clone, Copy)]
pub struct IdentityClients;

impl ScopeName for IdentityClients {
    const NAME: &'static str = "identity:clients";
    const DESCRIPTION: &'static str =
        "Register, rotate and remove a workspace's own apps and their secrets.";
}
