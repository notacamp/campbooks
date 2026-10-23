//! The OAuth scopes of the `schedule` module (see `campbooks_kernel::auth`).
//!
//! A scope name is a wire contract with the identity provider and every
//! client: never rename one. Add a scope here and to [`ALL`] before an
//! endpoint requires it.

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<ScheduleRead>(),
    ScopeInfo::of::<ScheduleWrite>(),
];

/// `schedule:read` — list and read events, calendars, settings and free
/// slots.
#[derive(Debug, Clone, Copy)]
pub struct ScheduleRead;

impl ScopeName for ScheduleRead {
    const NAME: &'static str = "schedule:read";
    const DESCRIPTION: &'static str =
        "See your calendars and events, your calendar settings, and when you are free.";
}

/// `schedule:write` — create, change, delete, answer and push events, and
/// change calendars and settings.
#[derive(Debug, Clone, Copy)]
pub struct ScheduleWrite;

impl ScopeName for ScheduleWrite {
    const NAME: &'static str = "schedule:write";
    const DESCRIPTION: &'static str = "Create, change, delete and answer events, add them to your provider calendars, and change your calendars and calendar settings.";
}
