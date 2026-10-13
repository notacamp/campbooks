//! The OAuth scope of the `admin` module (see `campbooks_kernel::auth`).
//!
//! `admin:access` is not a permission: it only lets a token reach the admin
//! API at all, and the built-in issuer grants it to first-party clients only
//! (the web app and the native app), never to clients a workspace or a
//! third party registers. What an administrator may do comes from their
//! profile, read on every request.

use campbooks_kernel::{
    admin::ACCESS_SCOPE,
    auth::{ScopeInfo, ScopeName},
};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<AdminAccess>()];

/// `admin:access` — reach the admin API (with an admin profile).
#[derive(Debug, Clone, Copy)]
pub struct AdminAccess;

impl ScopeName for AdminAccess {
    const NAME: &'static str = ACCESS_SCOPE;
    const DESCRIPTION: &'static str = "Reach the instance administration API, if your account \
         has an admin profile. Granted to Campbooks' own apps only.";
}
