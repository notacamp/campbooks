//! The OAuth scopes of the `assistant` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one. Add a scope here and to [`ALL`] before an
//! endpoint requires it.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<AssistantRead>(),
    ScopeInfo::of::<AssistantWrite>(),
];

/// `assistant:read` — see what you told Scout.
#[derive(Debug, Clone, Copy)]
pub struct AssistantRead;

impl ScopeName for AssistantRead {
    const NAME: &'static str = "assistant:read";
    const DESCRIPTION: &'static str = "See what you told Scout about how it should act.";
}

/// `assistant:write` — tell Scout how to act, and turn sentences into rules.
#[derive(Debug, Clone, Copy)]
pub struct AssistantWrite;

impl ScopeName for AssistantWrite {
    const NAME: &'static str = "assistant:write";
    const DESCRIPTION: &'static str = "Tell Scout how it should act, and turn your sentences into rules to check.";
}
