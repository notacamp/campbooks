//! The OAuth scopes of the `search` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<SearchRead>()];

/// `search:read` — search what you may read. Results of a record type also
/// need a scope that reads that type.
#[derive(Debug, Clone, Copy)]
pub struct SearchRead;

impl ScopeName for SearchRead {
    const NAME: &'static str = "search:read";
    const DESCRIPTION: &'static str =
        "Search the records you may read (conversations need conversations:read too).";
}
