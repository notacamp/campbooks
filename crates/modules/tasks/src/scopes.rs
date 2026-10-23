//! The OAuth scopes of the `tasks` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<TasksRead>(), ScopeInfo::of::<TasksWrite>()];

/// `tasks:read` — see your tasks.
#[derive(Debug, Clone, Copy)]
pub struct TasksRead;

impl ScopeName for TasksRead {
    const NAME: &'static str = "tasks:read";
    const DESCRIPTION: &'static str = "See your tasks and the ones you handed to someone.";
}

/// `tasks:write` — add and change tasks.
#[derive(Debug, Clone, Copy)]
pub struct TasksWrite;

impl ScopeName for TasksWrite {
    const NAME: &'static str = "tasks:write";
    const DESCRIPTION: &'static str =
        "Add tasks, change them, mark them done and hand them to a teammate.";
}
