//! `campbooks-conversations` — messages on any channel (email first):
//! inboxes, conversations, messages, streams and labels (`docs/domain-map.md`
//! → *conversations*).
//!
//! - **Ingest** ([`ingest`]): a connector's batch of normalised message
//!   changes for one inbox is applied idempotently by the job
//!   `conversations.apply_changes`, which threads messages into
//!   conversations, keeps bodies and attachments in blob storage, and
//!   publishes [`events`] in the same transaction. The contract is
//!   `contracts/message-change-batch.v1.schema.json`.
//! - **Streams and the decide pass**: every inbound message is asked six
//!   typed questions, by rules when it is stored and by the `decide` model
//!   afterwards (`conversations.decide_message`). Every answer is stored in
//!   `decisions`; a per-question switch says which may act. In this version
//!   only confident rules answers place a conversation in a stream; model
//!   answers are suggestions (ADR 0009 note, campbooks-cloud#59).
//! - **Operations** under `/api/v1/w/{workspace_id}/…`: list inboxes and
//!   conversations, read a conversation, mark read, star, archive, label,
//!   move to a stream (recorded as a correction).
//!
//! The module is composed with its dependencies ([`ConversationsDeps`]):
//! the AI gateway and settings are given by the server, never looked up.

#![forbid(unsafe_code)]

use std::sync::Arc;

use campbooks_ai::{Gateway, Role};
use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

mod config;
pub mod dev;
pub mod documents;
mod domain;
pub mod events;
mod features;
pub mod ingest;
pub mod model;
pub mod scopes;
#[cfg(test)]
mod testkit;

pub use config::{ConversationsConfig, DEFAULT_DECIDE_BACKFILL_DAYS};
pub use domain::decide::{
    BoxFuture, Envelope, ModelStrategy, NoRules, STATE_VERSION, Source, Strategy, Verdict,
};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "conversations";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// What the module is composed with.
#[derive(Clone)]
pub struct ConversationsDeps {
    /// The AI gateway; `None` when no AI role is configured. The model pass
    /// runs only when it serves the `decide` role.
    pub ai: Option<Gateway>,
    /// The module's settings.
    pub config: ConversationsConfig,
    /// The rules strategy that sorts conversations as messages arrive.
    pub rules: Arc<dyn Strategy>,
}

impl std::fmt::Debug for ConversationsDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationsDeps")
            .field("ai", &self.ai.is_some())
            .field("config", &self.config)
            .field("rules", &self.rules.name())
            .finish()
    }
}

impl ConversationsDeps {
    /// Dependencies with `ai` and `config`, and the rules strategy of
    /// campbooks-cloud#59 once it lands ([`NoRules`] until then).
    pub fn new(ai: Option<Gateway>, config: ConversationsConfig) -> Self {
        Self {
            ai,
            config,
            rules: Arc::new(NoRules),
        }
    }

    /// No AI, default settings: for describing the module (OpenAPI, the
    /// events catalogue) without running it.
    pub fn offline() -> Self {
        Self::new(None, ConversationsConfig::default())
    }

    /// The same dependencies sorting with `rules`.
    #[must_use]
    pub fn with_rules(mut self, rules: Arc<dyn Strategy>) -> Self {
        self.rules = rules;
        self
    }
}

/// The `conversations` module, composed with `deps`, ready for the server
/// to mount.
pub fn module(deps: ConversationsDeps) -> Module {
    let model = deps
        .ai
        .filter(|gateway| gateway.config().provider(Role::Decide).is_ok())
        .map(ModelStrategy::new);
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router(),
        background: Background::new()
            .publishes::<events::MessageReceived>()
            .publishes::<events::MessageSent>()
            .publishes::<events::ConversationChanged>()
            .publishes::<events::ChangesRequested>()
            .handler(features::apply_changes::ApplyChanges::new(deps.rules))
            .consumer(features::decide_message::DecideMessage::new(
                model,
                deps.config.decide_backfill_days,
            ))
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .job::<features::purge_workspace::PurgeWorkspace>(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{
        archive, get_conversation, get_message_body, labels, list_conversations, list_inboxes,
        mark_read, move_to_stream, star,
    };
    OpenApiRouter::new()
        .routes(routes!(list_inboxes::handler))
        .routes(routes!(list_conversations::handler))
        .routes(routes!(get_conversation::handler))
        .routes(routes!(get_message_body::handler))
        .routes(routes!(mark_read::read))
        .routes(routes!(mark_read::unread))
        .routes(routes!(star::star))
        .routes(routes!(star::unstar))
        .routes(routes!(archive::archive))
        .routes(routes!(archive::unarchive))
        .routes(routes!(move_to_stream::handler))
        .routes(routes!(labels::add))
        .routes(routes!(labels::remove))
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(module(ConversationsDeps::offline()).name, "conversations");
    }

    #[test]
    fn its_background_work_registers() {
        let registry =
            campbooks_kernel::jobs::Registry::build(&[module(ConversationsDeps::offline())])
                .unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "conversations.apply_changes",
            "conversations.decide_message",
            "conversations.schedule_workspace_purge",
            "conversations.purge_workspace",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
    }

    // A runtime query rather than `query!`, so this test needs no entry in
    // the `.sqlx` offline cache.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_conversations_schema(db: PgPool) {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
                .bind(super::NAME)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(exists);
    }
}
