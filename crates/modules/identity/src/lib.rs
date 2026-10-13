//! `campbooks-identity` — who people are to Campbooks, and the tokens they
//! get (ADR 0014).
//!
//! The module owns **accounts** and their **ways in** (provider identities
//! today; passkeys later), linked only by proven ownership, never by email.
//! It is the instance's **OAuth 2.1 authorization server and OIDC
//! provider**, in-process, on the app's own origin: discovery, JWKS, the
//! authorization and token endpoints, revocation and logout, with Ed25519
//! keys it generates, seals and rotates itself. It runs the **provider round
//! trips** (Google first): one consent grants identity and mailbox, and the
//! mailbox grant of a new account is **held** until the person puts it in a
//! workspace, where it goes to the connectors module through the kernel's
//! grant-intake port.
//!
//! Its surface is in two parts:
//!
//! - [`module`]: the API operations (`/api/v1/…`, scoped, in the catalogue);
//! - [`protocol::router`]: the issuer's protocol and sign-in endpoints
//!   (`/.well-known/*`, `/oauth2/*`, `/auth/*`), which serve browsers and
//!   OAuth clients and are mounted by the server next to the web BFF.
//!
//! While another identity provider is configured (Rauthy, until slice S5),
//! the issuer does not run and both answer 404.

#![forbid(unsafe_code)]

use axum::Extension;
use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod deps;
pub mod domain;
pub mod events;
mod features;
pub mod protocol;
mod prune;
pub mod scopes;
pub mod upstream;
mod web;

pub use deps::Deps;
pub use domain::keys::LocalKeys;
pub use prune::{PRUNE_EVERY, Prune};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "identity";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The `identity` module composed with `deps`, ready for the server to
/// mount.
pub fn module(deps: Deps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router().layer(Extension(deps)),
        background: Background::new()
            .publishes::<events::AccountCreated>()
            .publishes::<events::SignedIn>()
            .publishes::<events::RefreshReuseDetected>()
            .publishes::<events::WayInAdded>()
            .publishes::<events::WayInRemoved>()
            .schedule::<Prune>(PRUNE_EVERY),
    }
}

/// Every API route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(features::list_held_grants::handler))
        .routes(routes!(features::discard_held_grant::handler))
        .routes(routes!(features::adopt_held_grant::handler))
}

#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;
