//! The OAuth scopes of the `identity` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one. Add a scope here and to [`ALL`] before an
//! endpoint requires it.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<IdentityRead>(),
    ScopeInfo::of::<IdentityWrite>(),
];

/// `identity:read` — read this module's data.
#[derive(Debug, Clone, Copy)]
pub struct IdentityRead;

impl ScopeName for IdentityRead {
    const NAME: &'static str = "identity:read";
    const DESCRIPTION: &'static str = "Read your identity data.";
}

/// `identity:write` — change this module's data.
#[derive(Debug, Clone, Copy)]
pub struct IdentityWrite;

impl ScopeName for IdentityWrite {
    const NAME: &'static str = "identity:write";
    const DESCRIPTION: &'static str = "Change your identity data.";
}
