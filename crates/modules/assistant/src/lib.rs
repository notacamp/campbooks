//! `campbooks-assistant` — what each member told Scout about how to act
//! (`docs/domain-map.md` → *assistant*, its **Memory**).
//!
//! The module owns a small, member-private **profile** (whether Scout may
//! act on its own proposals and tell the member afterwards, and when the
//! questions were answered) and the step that **compiles** a sentence the
//! member typed into taught rules for them to check. The rules themselves
//! belong to the conversations module (campbooks-cloud#125): this module
//! never stores one. Compiling gives the `complete` role the member's
//! sentence and their own addresses and label names, never anyone's mail,
//! and only proposes; the member previews and saves.
//!
//! It also owns each member's **chats** with Scout (campbooks-cloud#305):
//! the member asks about what they are looking at, and Scout explains and
//! looks things up as that member, through other modules' read operations
//! invoked by id, from a handbook whose sections the modules own
//! ([`handbook`]). Scout may propose a change in a chat; it never makes
//! one: the member confirms a card and their own request does.

#![forbid(unsafe_code)]

use std::sync::Arc;

use campbooks_ai::Gateway;
use campbooks_kernel::{
    AppState, Background, Module, i18n::Catalogue, preferences::MemberPreferencesPort,
};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod domain;
pub mod events;
mod features;
pub mod handbook;
pub mod scopes;

pub use handbook::{Handbook, Section};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "assistant";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// What the module is composed with.
#[derive(Debug, Clone, Default)]
pub struct AssistantDeps {
    /// The AI gateway; compiling a sentence and answering in a chat need
    /// its `complete` role.
    pub ai: Option<Gateway>,
    /// Scout's handbook: this module's core section and the sections the
    /// server composed from the other modules.
    pub handbook: Handbook,
}

/// The `assistant` module, composed with `deps`, ready for the server to
/// mount.
pub fn module(deps: AssistantDeps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router()
            .layer(axum::Extension(features::chat::Chatter::new(
                deps.ai.clone(),
                deps.handbook,
            )))
            .layer(axum::Extension(features::compile::Compiler::new(deps.ai))),
        admin_router: None,
        resources: domain::resources::resources(),
        background: Background::new()
            .on_event::<features::purge::ScheduleWorkspacePurge>()
            .on_event::<features::purge::ForgetMember>()
            .job::<features::purge::PurgeWorkspace>()
            .schedule::<features::expire_chats::DispatchExpireChats>(features::expire_chats::EVERY)
            .job::<features::expire_chats::ExpireChats>(),
    }
}

/// The kernel's `MemberPreferences` port, answered from members' profiles.
pub fn preferences() -> MemberPreferencesPort {
    Arc::new(features::profile::Preferences)
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{chat, compile, profile};
    OpenApiRouter::new()
        .routes(routes!(profile::get, profile::update))
        .routes(routes!(compile::handler))
        .routes(routes!(chat::start))
        .routes(routes!(chat::get, chat::delete))
        .routes(routes!(chat::ask))
        .routes(routes!(chat::stream))
        .routes(routes!(chat::settle))
}

#[cfg(test)]
mod testkit;

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(module(AssistantDeps::default()).name, "assistant");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn its_background_work_registers() {
        let registry =
            campbooks_kernel::jobs::Registry::build(&[module(AssistantDeps::default())]).unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "assistant.schedule_workspace_purge",
            "assistant.forget_member",
            "assistant.purge_workspace",
            "assistant.dispatch_expire_chats",
            "assistant.expire_chats",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
    }

    // A runtime query rather than `query!`, so this test needs no entry in
    // the `.sqlx` offline cache.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_assistant_schema(db: PgPool) {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
                .bind(super::NAME)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(exists);
    }
}
