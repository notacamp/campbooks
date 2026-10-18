//! The OAuth scopes of the `system` module (see `campbooks_kernel::auth`).
//!
//! Health and version are public. `system:read`, which every Campbooks app
//! asks for at sign-in, is also what the app needs to report its own errors
//! and timings (`client_telemetry`) and the screens it shows
//! (`screen_views`).

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<SystemRead>()];

/// `system:read` — read operational status.
#[derive(Debug, Clone, Copy)]
pub struct SystemRead;

impl ScopeName for SystemRead {
    const NAME: &'static str = "system:read";
    const DESCRIPTION: &'static str =
        "Read operational status, and report the app's own errors and timings.";
}
