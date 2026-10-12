//! The OAuth scopes of the `system` module (see `campbooks_kernel::auth`).
//!
//! Health and version are public, so no endpoint requires these yet; they are
//! declared so clients and the identity provider can already be granted them.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<SystemRead>()];

/// `system:read` — read operational status.
#[derive(Debug, Clone, Copy)]
pub struct SystemRead;

impl ScopeName for SystemRead {
    const NAME: &'static str = "system:read";
    const DESCRIPTION: &'static str = "Read operational status.";
}
