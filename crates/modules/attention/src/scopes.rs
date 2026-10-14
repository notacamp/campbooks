//! The OAuth scopes of the `attention` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one. Add a scope here and to [`ALL`] before an
//! endpoint requires it.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<AttentionRead>(),
    ScopeInfo::of::<AttentionWrite>(),
];

/// `attention:read` — see what needs you.
#[derive(Debug, Clone, Copy)]
pub struct AttentionRead;

impl ScopeName for AttentionRead {
    const NAME: &'static str = "attention:read";
    const DESCRIPTION: &'static str = "See what needs you: the items on your Home.";
}

/// `attention:write` — put items off until later.
#[derive(Debug, Clone, Copy)]
pub struct AttentionWrite;

impl ScopeName for AttentionWrite {
    const NAME: &'static str = "attention:write";
    const DESCRIPTION: &'static str =
        "Put items on your Home off until later, and bring them back.";
}
