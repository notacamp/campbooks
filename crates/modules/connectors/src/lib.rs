//! `campbooks-connectors` — the only place that knows about providers
//! (ADR 0010).
//!
//! A **connection** is a member's authorisation for Campbooks to use one
//! outside account, with the **capabilities** it grants (`messages` today).
//! It belongs to the workspace and records who granted it; its credentials
//! are sealed at rest and never leave the module. Each connection and
//! capability is a **change stream** with a **cursor**, run by one engine:
//! a one-minute dispatcher over a due index, one bounded page per job,
//! adaptive intervals, a per-provider budget, and **health** (`ok`,
//! `needs_reauth`, `throttled`, `failing`). **Adapters** read a page from a
//! cursor; the Gmail adapter produces `MessageChangeBatch` v1 documents
//! (`contracts/`) announced by `connectors.messages_changed`.
//!
//! `lib.rs` assembles [`module`]; each use case is a slice under
//! `features/`, shared rules live in `domain/`, provider HTTP in `google/`
//! and the Gmail adapter in `gmail/`. The module is composed with its
//! dependencies ([`Deps`]): handlers reach them as an axum extension, jobs
//! and consumers own a copy.

#![forbid(unsafe_code)]

use axum::Extension;
use campbooks_kernel::{Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod config;
pub mod contract;
mod deps;
mod domain;
pub mod events;
mod features;
mod gmail;
mod google;
#[cfg(test)]
mod rls_tests;
pub mod scopes;
#[cfg(test)]
mod testkit;

pub use deps::{CREDENTIALS_PURPOSE, Deps, GOOGLE_CALLBACK_PATH};
pub use domain::intake::grant_intake;
pub use domain::model::{
    AuthorizationId, Capability, ConnectionId, ConnectionView, Health, InboxId, Phase, Provider,
    StreamId, StreamView,
};
pub use google::{ClientSecret, Endpoints, Google};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "connectors";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The connectors module, composed with `deps`, ready for the server to
/// mount.
pub fn module(deps: Deps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new()
            .routes(routes!(features::list_connections::handler))
            .routes(routes!(features::start_connection::handler))
            .routes(routes!(features::complete_connection::handler))
            .routes(routes!(
                features::get_connection::handler,
                features::disconnect::handler
            ))
            .routes(routes!(features::sync_now::handler))
            .layer(Extension(deps.clone())),
        background: Background::new()
            .publishes::<events::ConnectionAdded>()
            .publishes::<events::CapabilityGranted>()
            .publishes::<events::ConnectionNeedsReauth>()
            .publishes::<events::ConnectionRemoved>()
            .publishes::<events::MessagesChanged>()
            .publishes::<events::SendCompleted>()
            .handler(features::sync_stream::SyncStream::new(deps.clone()))
            .handler(features::send_message::SendMessage::new(deps.clone()))
            .consumer(features::queue_send::QueueSend)
            .schedule::<features::dispatch_due::DispatchDue>(features::dispatch_due::EVERY)
            .consumer(features::member_left::RevokeDepartedGrants::new(
                deps.clone(),
            ))
            .consumer(features::workspace_deleting::RevokeWorkspaceGrants::new(
                deps,
            )),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(super::module(super::Deps::default()).name, "connectors");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(super::MESSAGES.check(), Vec::<String>::new());
    }
}
