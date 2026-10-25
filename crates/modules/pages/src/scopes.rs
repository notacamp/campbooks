//! The OAuth scopes of the `pages` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<PagesRead>(), ScopeInfo::of::<PagesWrite>()];

/// `pages:read` - read the pages you may open.
#[derive(Debug, Clone, Copy)]
pub struct PagesRead;

impl ScopeName for PagesRead {
    const NAME: &'static str = "pages:read";
    const DESCRIPTION: &'static str = "See the pages you may open, and who they are shared with.";
}

/// `pages:write` - write pages.
#[derive(Debug, Clone, Copy)]
pub struct PagesWrite;

impl ScopeName for PagesWrite {
    const NAME: &'static str = "pages:write";
    const DESCRIPTION: &'static str =
        "Make, edit, move, share and delete pages, and add files to them.";
}
