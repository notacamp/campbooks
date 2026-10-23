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
//! - **Streams and the decide pass**: every inbound message is asked the
//!   typed triage questions, by the facts when it is stored and by the
//!   `decide` model afterwards (`conversations.decide_message`). Every answer
//!   is stored in `decisions`; a per-question switch says which may act. The
//!   facts read headers and addresses only and may only say "no"; the
//!   model reads the email, and its accepted answers sort the conversation
//!   and raise its need (ADR 0018).
//! - **Dates in mail** (campbooks-cloud#42): the pass also asks whether the
//!   email tells of something dated, and flags it (`domain::dated`); the
//!   sender's own machine-readable dates are read with no model
//!   (`conversations.read_structured`, ADR 0020); and one `complete` call per
//!   flagged message, when a need opens or a member opens it, reads the
//!   summary, the need's dates, the things that happen at a time and the
//!   obligations (`conversations.read_message`). Both reads are stored in
//!   `message_reads` and published as `conversations.message_read`.
//! - **Search**: the module is the search source of its conversations
//!   ([`search_source`], ADR 0016) and lists the inboxes a caller may search
//!   through its visibility port ([`visibility`]).
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
        signals::facts::EmailFacts,
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

pub use config::{
    ActionSettings, ConversationsConfig, DEFAULT_DATED_FLAG, DEFAULT_DECIDE_BACKFILL_DAYS,
    DEFAULT_NEEDS_WINDOW_DAYS, DatedSettings, NoticeSettings, WaitSettings,
};
pub use domain::{
    decide::STATE_VERSION,
    policy::{Policy, Switch},
};
pub use features::{
    inbox_visibility::visibility, kept_mail::kept_mail, outbound_requests::outbound_requests,
    search_source::search_source,
};
/// The owner's money mail as an eval set to label (campbooks-cloud#167).
pub mod labelling {
    pub use crate::features::labelling_export::{
        LabellingItem, LabellingSet, MAX_ITEMS, OWNER, export_inboxes, export_set, manifest,
        manifest_for,
    };
}

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "conversations";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// This module's section of Scout's handbook (campbooks-cloud#305): how
/// mail, streams, Scout's read, taught rules, corrections, person notes, drafts, waiting and Nudge work, for the model answering a member's questions. The server
/// composes it into the assistant's handbook; at most 8 KiB.
pub const HANDBOOK: &str = include_str!("../handbook.md");

/// What the module is composed with.
#[derive(Clone)]
pub struct ConversationsDeps {
    /// The AI gateway; `None` when no AI role is configured. The model pass
    /// runs only when it serves the `decide` role.
    pub ai: Option<Gateway>,
    /// The module's settings.
    pub config: ConversationsConfig,
    /// The rules strategy asked as messages arrive: the facts.
    pub rules: Arc<dyn Strategy>,
    /// A strategy the model pass asks instead of the `decide` role (tests,
    /// tools); `None` asks the role, when it is configured.
    pub model: Option<Arc<dyn Strategy>>,
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
            .field("model", &self.model.as_ref().map(|m| m.name().to_owned()))
            .field("policy", &self.policy)
            .finish()
    }
}

impl ConversationsDeps {
    /// Dependencies with `ai` and `config`, with the shared facts
    /// ([`EmailFacts`]) as rules, under the eval baseline's policy
    /// ([`Policy::baseline`]).
    pub fn new(ai: Option<Gateway>, config: ConversationsConfig) -> Self {
        Self {
            ai,
            config,
            rules: Arc::new(EmailFacts),
            model: None,
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

    /// The same dependencies, the model pass asking `model` instead of the
    /// `decide` role.
    #[must_use]
    pub fn with_model(mut self, model: Arc<dyn Strategy>) -> Self {
        self.model = Some(model);
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
#[allow(clippy::too_many_lines)]
pub fn module(deps: ConversationsDeps) -> Module {
    // Scout's read of a message: only while the `complete` role may receive
    // mail here.
    let summaries = deps
        .ai
        .clone()
        .filter(features::read_message::may_read_mail);
    // Scout's replies ask the `complete` role, when it may read mail.
    let writer = features::reply::ReplyWriter::new(deps.ai.clone());
    let model = match deps.model {
        Some(strategy) => Some(features::decide_message::ModelPass::new(strategy, None)),
        None => deps.ai.filter(model_may_read_mail).map(|gateway| {
            // Only while the role passes its answers check: otherwise the
            // model pass stores nothing, as if no model were configured.
            let strategy = Model::new("decide", gateway.clone(), domain::decide::PURPOSE).checked();
            features::decide_message::ModelPass::new(Arc::new(strategy), Some(gateway))
        }),
    };
    let needs = domain::needs::NeedSettings::new(deps.config.needs_window_days, &deps.policy)
        .with_summaries(summaries.is_some())
        .with_wait_days(deps.config.wait.working_days)
        .with_waits(deps.config.actions.enabled)
        .with_dated(deps.config.dated);
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router()
            .layer(Extension(features::send_draft::SendSettings {
                undo: chrono::TimeDelta::seconds(i64::from(deps.config.undo_send_seconds)),
            }))
            .layer(Extension(needs.clone()))
            .layer(Extension(deps.config.notices))
            .layer(Extension(deps.config.actions))
            .layer(Extension(deps.policy.clone()))
            .layer(Extension(writer)),
        admin_router: None,
        background: Background::new()
            .publishes::<events::MessageReceived>()
            .publishes::<events::MessageSent>()
            .publishes::<events::ConversationChanged>()
            .publishes::<events::ChangesRequested>()
            .publishes::<events::SendRequested>()
            .publishes::<events::DraftSent>()
            .publishes::<events::SendFailed>()
            .publishes::<events::InboxChanged>()
            .publishes::<events::InboxRemoved>()
            .publishes::<events::NeedChanged>()
            .publishes::<events::NeedSummarised>()
            .publishes::<events::MessageRead>()
            .publishes::<events::RuleChanged>()
            .handler(features::release_send::ReleaseSend)
            .handler(features::check_send::CheckSend)
            .consumer(features::record_send::RecordSend)
            .handler(features::apply_rule::ApplyRules::new(needs.clone()))
            .handler(features::fill_templates::FillTemplates)
            .handler(features::rule_notices::NoticeRules::new(
                deps.config.notices,
            ))
            .handler(features::refresh_need::RefreshNeed::new(needs.clone()))
            .handler(features::read_message::ReadMessage::new(
                summaries,
                needs.clone(),
            ))
            .handler(features::read_structured::ReadStructured::new(
                needs.clone(),
            ))
            .handler(features::wake_needs::WakeNeeds::new(needs.clone()))
            .schedule::<features::wake_needs::DispatchNeedClocks>(features::wake_needs::EVERY)
            .handler(features::apply_changes::ApplyChanges::new(
                deps.rules,
                deps.policy.clone(),
                needs.clone(),
            ))
            .handler(
                features::backfill_needs::BackfillNeeds::new(
                    needs.clone(),
                    model.clone(),
                    deps.config.decide_backfill_days,
                )
                .with_actions(deps.config.actions),
            )
            .schedule::<features::backfill_needs::DispatchNeedsBackfills>(
                features::backfill_needs::EVERY,
            )
            .schedule::<features::resign_rules::DispatchRuleResigns>(features::resign_rules::EVERY)
            .job::<features::resign_rules::ResignRules>()
            .rekey::<features::retag_bindings::RetagBindings>()
            .consumer(
                features::decide_message::DecideMessage::new(
                    model.clone(),
                    deps.config.decide_backfill_days,
                    deps.policy.clone(),
                    needs.clone(),
                )
                .with_actions(deps.config.actions),
            )
            .handler(
                features::decide_message::DecideBacklog::new(
                    model,
                    deps.config.decide_backfill_days,
                    deps.policy,
                    needs,
                )
                .with_actions(deps.config.actions),
            )
            .on_event::<features::ingest_messages::IngestMessages>()
            .on_event::<features::create_inbox::CreateInbox>()
            .on_event::<features::forget_connection::ForgetConnection>()
            .on_event::<features::forget_member::ForgetMember>()
            .on_event::<features::suspended_authors::PauseSuspendedAuthor>()
            .on_event::<features::suspended_authors::ResumeUnsuspendedAuthor>()
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .job::<features::purge_workspace::PurgeWorkspace>(),
        resources: domain::resources::resources(),
    }
}

/// Whether the model pass runs: the `decide` role is configured and may
/// receive mail (on the instance's network, or opted in with
/// `CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow`). Without it, nothing is
/// sorted out of Primary, only the facts' "no" acts, and nothing leaves the
/// instance.
fn model_may_read_mail(gateway: &Gateway) -> bool {
    match gateway.config().provider_for_content(Role::Decide) {
        Ok(_) => true,
        Err(error @ campbooks_ai::Error::ExternalContentRefused { .. }) => {
            // The module is composed more than once at startup (routes,
            // catalogues, worker): say it once.
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| {
                tracing::warn!(%error, "the decide pass is off: only the facts read mail");
            });
            false
        }
        Err(_) => false,
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{
        archive, create_draft, get_attachment, get_conversation, get_history, get_message_body,
        get_message_read, labels, list_conversations, list_correspondents, list_drafts,
        list_inboxes, list_labels, list_senders, list_streams, mark_read, move_to_stream, needs,
        person_notes, reply, request_message_read, rule_notices, rules, send_draft, spam, star,
        suggest_rules, trash, update_draft,
    };
    OpenApiRouter::new()
        .routes(routes!(list_inboxes::handler))
        .routes(routes!(list_senders::handler))
        .routes(routes!(list_correspondents::handler))
        .routes(routes!(list_labels::handler))
        .routes(routes!(list_streams::handler))
        .routes(routes!(list_conversations::handler))
        .routes(routes!(get_conversation::handler))
        .routes(routes!(get_history::handler))
        .routes(routes!(get_message_body::handler))
        .routes(routes!(
            get_message_read::handler,
            request_message_read::handler
        ))
        .routes(routes!(get_attachment::handler))
        .routes(routes!(mark_read::read))
        .routes(routes!(mark_read::unread))
        .routes(routes!(star::star))
        .routes(routes!(star::unstar))
        .routes(routes!(archive::archive))
        .routes(routes!(archive::unarchive))
        .routes(routes!(trash::trash))
        .routes(routes!(trash::untrash))
        .routes(routes!(spam::report))
        .routes(routes!(spam::not_spam))
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
        .routes(routes!(rules::preview))
        .routes(routes!(rules::list))
        .routes(routes!(rules::create))
        .routes(routes!(rules::get, rules::update, rules::remove))
        .routes(routes!(rules::confirm))
        .routes(routes!(rules::decline))
        .routes(routes!(rules::restore))
        .routes(routes!(suggest_rules::suggest))
        .routes(routes!(suggest_rules::dismiss))
        .routes(routes!(rule_notices::list))
        .routes(routes!(rule_notices::answer, rule_notices::clear_answer))
        .routes(routes!(reply::suggest))
        .routes(routes!(reply::write))
        .routes(routes!(reply::nudge))
        .routes(routes!(reply::stream))
        .routes(routes!(person_notes::list, person_notes::set))
        .routes(routes!(person_notes::delete))
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
            "conversations.decide_backlog",
            "conversations.schedule_workspace_purge",
            "conversations.purge_workspace",
            "conversations.release_send",
            "conversations.record_send",
            "conversations.check_send",
            "conversations.apply_rule",
            "conversations.fill_templates",
            "conversations.notice_rules",
            "conversations.refresh_need",
            "conversations.read_message",
            "conversations.read_structured",
            "conversations.backfill_needs",
            "conversations.dispatch_needs_backfills",
            "conversations.dispatch_rule_resigns",
            "conversations.resign_rules",
            "conversations.retag_bindings",
            "conversations.wake_needs",
            "conversations.dispatch_need_clocks",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
        let schedules = registry.schedule_names();
        assert!(schedules.contains(&"conversations.dispatch_needs_backfills"));
        assert!(schedules.contains(&"conversations.dispatch_rule_resigns"));
        assert!(schedules.contains(&"conversations.dispatch_need_clocks"));
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
