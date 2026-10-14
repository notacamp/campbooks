//! `campbooks-identity` — who people are to Campbooks, and the tokens they
//! get (ADR 0014).
//!
//! The module owns **accounts** and their **ways in** (provider identities
//! and passkeys), linked only by proven ownership, never by email. It is
//! the instance's WebAuthn relying party ([`webauthn`]).
//! It is the instance's **OAuth 2.1 authorization server and OIDC
//! provider**, in-process, on the app's own origin: discovery, JWKS, the
//! authorization and token endpoints, revocation and logout, with Ed25519
//! keys it generates, seals and rotates itself. It runs the **provider round
//! trips** (Google, Microsoft, Zoho), the one front door for every provider
//! consent: signing in (one consent grants identity and mailbox; a new
//! account's mailbox grant is **held** until the person puts it in a
//! workspace), adding a way in while signed in, and connecting or
//! reconnecting a mailbox. Grants go to the connectors module through the
//! kernel's grant-intake port; the connectors module starts its consents
//! through the consent-door port this module implements.
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

mod consent;
pub mod deps;
pub mod domain;
pub mod events;
mod features;
pub mod metadata;
pub mod protocol;
mod prune;
mod resources;
pub mod scopes;
pub mod upstream;
mod web;
pub mod webauthn;

pub use consent::consent_door;
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
        admin_router: None,
        background: Background::new()
            .publishes::<events::AccountCreated>()
            .publishes::<events::SignedIn>()
            .publishes::<events::RefreshReuseDetected>()
            .publishes::<events::WayInAdded>()
            .publishes::<events::WayInRemoved>()
            .publishes::<events::ClientRegistered>()
            .publishes::<events::ClientRemoved>()
            .publishes::<events::ConsentGranted>()
            .publishes::<events::ConsentRevoked>()
            .consumer(features::member_left::RevokeDepartedConsents)
            .consumer(features::workspace_deleting::RemoveWorkspaceClients)
            .schedule::<Prune>(PRUNE_EVERY),
        resources: resources::resources(),
    }
}

/// Every API route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(features::list_held_grants::handler))
        .routes(routes!(features::discard_held_grant::handler))
        .routes(routes!(features::adopt_held_grant::handler))
        .routes(routes!(features::list_ways_in::handler))
        .routes(routes!(features::remove_way_in::handler))
        .routes(routes!(features::start_link::handler))
        .routes(routes!(features::list_consents::handler))
        .routes(routes!(features::revoke_consent::handler))
        .routes(routes!(
            features::list_clients::handler,
            features::register_client::handler
        ))
        .routes(routes!(features::rotate_client_secret::handler))
        .routes(routes!(features::remove_client::handler))
        .merge(passkey_routes())
}

/// The passkey operations: their bodies (a credential, a name) are a few
/// kilobytes at most.
fn passkey_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(
            features::list_passkeys::handler,
            features::register_passkey::handler
        ))
        .routes(routes!(features::start_passkey_registration::handler))
        .routes(routes!(
            features::rename_passkey::handler,
            features::remove_passkey::handler
        ))
        .layer(axum::extract::DefaultBodyLimit::max(
            protocol::passkey::MAX_BODY,
        ))
}

#[cfg(test)]
mod clients_tests;
#[cfg(test)]
mod passkey_tests;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ways_in_tests;
