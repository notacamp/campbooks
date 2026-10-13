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

use axum::Extension;
use campbooks_ai::{
    Gateway, Role,
    decide::{
        signals::email::EmailSignals,
        strategy::{Model, Strategy},
    },
};
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
pub mod outgoing;
pub mod scopes;
#[cfg(test)]
mod testkit;

pub use config::{ConversationsConfig, DEFAULT_DECIDE_BACKFILL_DAYS, DEFAULT_NEEDS_WINDOW_DAYS};
pub use domain::{
    decide::STATE_VERSION,
    policy::{Policy, Switch},
};
pub use features::inbox_visibility::visibility;

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
    /// Which answers may act, per question and source.
    pub policy: Policy,
}

impl Default for ConversationsDeps {
    /// [`offline`](Self::offline).
    fn default() -> Self {
        Self::offline()
    }
}

impl std::fmt::Debug for ConversationsDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationsDeps")
            .field("ai", &self.ai.is_some())
            .field("config", &self.config)
            .field("rules", &self.rules.name())
            .field("policy", &self.policy)
            .finish()
    }
}

impl ConversationsDeps {
    /// Dependencies with `ai` and `config`, sorting with the shared email
    /// signals ([`EmailSignals`]) under the eval baseline's policy
    /// ([`Policy::baseline`]).
    pub fn new(ai: Option<Gateway>, config: ConversationsConfig) -> Self {
        Self {
            ai,
            config,
            rules: Arc::new(EmailSignals),
            policy: Policy::baseline(),
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

    /// The same dependencies acting on answers as `policy` allows.
    #[must_use]
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }
}

/// The `conversations` module, composed with `deps`, ready for the server
/// to mount.
pub fn module(deps: ConversationsDeps) -> Module {
    let model = deps
        .ai
        .filter(|gateway| gateway.config().provider(Role::Decide).is_ok())
        .map(|gateway| {
            Arc::new(Model::new("decide", gateway, domain::decide::PURPOSE)) as Arc<dyn Strategy>
        });
    let needs = domain::needs::NeedSettings::new(deps.config.needs_window_days, &deps.policy);
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router()
            .layer(Extension(features::send_draft::SendSettings {
                undo: chrono::TimeDelta::seconds(i64::from(deps.config.undo_send_seconds)),
            }))
            .layer(Extension(needs.clone()))
            .layer(Extension(deps.policy.clone())),
        admin_router: None,
        background: Background::new()
            .publishes::<events::MessageReceived>()
            .publishes::<events::MessageSent>()
            .publishes::<events::ConversationChanged>()
            .publishes::<events::ChangesRequested>()
            .publishes::<events::SendRequested>()
            .publishes::<events::DraftSent>()
            .publishes::<events::SendFailed>()
            .publishes::<events::NeedChanged>()
            .handler(features::release_send::ReleaseSend)
            .handler(features::check_send::CheckSend)
            .consumer(features::record_send::RecordSend)
            .handler(features::apply_changes::ApplyChanges::new(
                deps.rules,
                deps.policy.clone(),
                needs.clone(),
            ))
            .consumer(features::decide_message::DecideMessage::new(
                model,
                deps.config.decide_backfill_days,
                deps.policy,
                needs,
            ))
            .on_event::<features::ingest_messages::IngestMessages>()
            .on_event::<features::create_inbox::CreateInbox>()
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .job::<features::purge_workspace::PurgeWorkspace>(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{
        archive, create_draft, get_attachment, get_conversation, get_message_body, labels,
        list_conversations, list_drafts, list_inboxes, list_streams, mark_read, move_to_stream,
        needs, send_draft, star, update_draft,
    };
    OpenApiRouter::new()
        .routes(routes!(list_inboxes::handler))
        .routes(routes!(list_streams::handler))
        .routes(routes!(list_conversations::handler))
        .routes(routes!(get_conversation::handler))
        .routes(routes!(get_message_body::handler))
        .routes(routes!(get_attachment::handler))
        .routes(routes!(mark_read::read))
        .routes(routes!(mark_read::unread))
        .routes(routes!(star::star))
        .routes(routes!(star::unstar))
        .routes(routes!(archive::archive))
        .routes(routes!(archive::unarchive))
        .routes(routes!(move_to_stream::handler))
        .routes(routes!(labels::add))
        .routes(routes!(labels::remove))
        .routes(routes!(list_drafts::list, create_draft::handler))
        .routes(routes!(
            list_drafts::get,
            update_draft::update,
            update_draft::delete
        ))
        .routes(routes!(send_draft::send))
        .routes(routes!(send_draft::cancel))
        .routes(routes!(needs::resolve))
        .routes(routes!(needs::reopen))
        .routes(routes!(needs::confirm))
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
            "conversations.release_send",
            "conversations.record_send",
            "conversations.check_send",
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
