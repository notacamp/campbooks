//! The OAuth scopes of the `connectors` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one. Add a scope here and to [`ALL`] before an
//! endpoint requires it.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<ConnectorsRead>(),
    ScopeInfo::of::<ConnectorsWrite>(),
];

/// `connectors:read` — see connected accounts and their health.
#[derive(Debug, Clone, Copy)]
pub struct ConnectorsRead;

impl ScopeName for ConnectorsRead {
    const NAME: &'static str = "connectors:read";
    const DESCRIPTION: &'static str =
        "See the outside accounts connected to your workspaces and how their sync is doing.";
}

/// `connectors:write` — connect, sync and disconnect outside accounts.
#[derive(Debug, Clone, Copy)]
pub struct ConnectorsWrite;

impl ScopeName for ConnectorsWrite {
    const NAME: &'static str = "connectors:write";
    const DESCRIPTION: &'static str =
        "Connect outside accounts (such as Gmail), sync them now and disconnect them.";
}
