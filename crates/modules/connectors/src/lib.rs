//! `campbooks-connectors` — the only place that knows about providers
//! (ADR 0010).
//!
//! A **connection** is a member's authorisation for Campbooks to use one
//! outside account, with the **capabilities** it grants (`messages` today).
//! It belongs to the workspace and records who granted it; its credentials
//! are sealed at rest and never leave the module. Each capability (for
//! `messages`, each mailbox the grant reaches) is a **change stream** with a
//! **cursor**, run by one engine: a one-minute dispatcher over a due index,
//! one bounded page per job, adaptive intervals, request budgets, and
//! **health** (`ok`, `needs_reauth`, `throttled`, `failing`). **Adapters**
//! read a page from a cursor; the Gmail and Zoho Mail adapters produce
//! `MessageChangeBatch` v1 documents (`contracts/`) announced by
//! `connectors.messages_changed`. Members' changes
//! (`conversations.changes_requested`) are recorded as **outbound changes**
//! and made at the provider by the stream's next run before it reads
//! (`domain/push.rs` plans them, `gmail/write.rs` and `zoho/write.rs` make
//! them); a write never comes back as a change.
//!
//! `lib.rs` assembles [`module`]; each use case is a slice under
//! `features/`, shared rules live in `domain/`, what the mail adapters share
//! in `mail/`, Google's HTTP in `google/`, the Gmail adapter in `gmail/`, and
//! Zoho (HTTP and adapter) in `zoho/`. The module is composed with its
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
mod mail;
mod metrics;
mod provider;
#[cfg(test)]
mod rls_tests;
pub mod scopes;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod testkit_zoho;
mod zoho;

pub use deps::{CREDENTIALS_PURPOSE, Deps};
pub use domain::intake::grant_intake;
pub use domain::model::{
    AuthorizationId, Capability, ConnectionId, ConnectionView, Health, InboxId, MailChoice, Phase,
    Provider, StreamId, StreamView,
};
pub use google::{Endpoints, Google};
pub use provider::ClientSecret;
pub use zoho::{ACCOUNTS_SERVERS as ZOHO_ACCOUNTS_SERVERS, Centres, Zoho};

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
            .routes(routes!(
                features::get_connection::handler,
                features::disconnect::handler
            ))
            .routes(routes!(features::sync_now::handler))
            .layer(Extension(deps.clone())),
        admin_router: Some(
            OpenApiRouter::new().routes(routes!(features::admin_list_connections::handler)),
        ),
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
            .rekey::<features::retag_bindings::RetagBindings>()
            .consumer(features::member_left::RevokeDepartedGrants::new(
                deps.clone(),
            ))
            .consumer(features::request_changes::RequestChanges::new(deps.clone()))
            .consumer(features::workspace_deleting::RevokeWorkspaceGrants::new(
                deps,
            )),
        resources: domain::resources::resources(),
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
