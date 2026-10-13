//! `campbooks-workspaces` — tenancy: users, workspaces, members and roles,
//! invitations, the instance's signup policy and the security log.
//!
//! Shaped like the `system` reference module: `lib.rs` assembles
//! [`module`], each use case is a slice under `features/`, and types shared
//! by several slices live in `domain/`. Every handler is a thin wrapper
//! around a plain `execute` function with typed input and output, and every
//! operation's OpenAPI `operation_id` is its catalogue name,
//! `workspaces.<verb>[_<noun>]` (ADR 0013).
//!
//! Besides its routes, the module implements the kernel's
//! [`Membership`](campbooks_kernel::tenancy::Membership) port ([`membership`]),
//! which the server registers so every module's `Workspace<S>` handlers know
//! who belongs where. Workspace-owned tables are protected by row-level
//! security (see the migration and `campbooks_kernel::db`).

#![forbid(unsafe_code)]

use campbooks_kernel::{Background, Module};
use utoipa_axum::{router::OpenApiRouter, routes};

mod domain;
pub mod events;
mod features;
#[cfg(test)]
mod rls_tests;
pub mod scopes;
#[cfg(test)]
mod testkit;

pub use domain::model::{Accent, Kind};
pub use features::delete_workspace::PurgeWorkspace;
pub use features::membership::membership;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "workspaces";

/// The workspaces module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new()
            .routes(routes!(
                features::list_workspaces::handler,
                features::create_workspace::handler
            ))
            .routes(routes!(features::accept_invitation::handler))
            .routes(routes!(
                features::get_workspace::handler,
                features::update_workspace::handler,
                features::delete_workspace::handler
            ))
            .routes(routes!(features::list_members::handler))
            .routes(routes!(
                features::change_role::handler,
                features::remove_member::handler
            ))
            .routes(routes!(features::leave_workspace::handler))
            .routes(routes!(
                features::list_invitations::handler,
                features::invite_member::handler
            ))
            .routes(routes!(features::list_security_events::handler)),
        background: Background::new().job::<PurgeWorkspace>(),
    }
}
