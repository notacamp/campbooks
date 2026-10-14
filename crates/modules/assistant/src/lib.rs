//! `campbooks-assistant` — what each member told Scout about how to act
//! (`docs/domain-map.md` → *assistant*, its **Memory**).
//!
//! The module owns a small, member-private **profile** (whether Scout may
//! act on its own proposals and tell the member afterwards, and when the
//! questions were answered) and the step that **compiles** a sentence the
//! member typed into taught rules for them to check. The rules themselves
//! belong to the conversations module (campbooks-cloud#125): this module
//! never stores one. Compiling is the only AI use: the `complete` role gets
//! the member's sentence and their own addresses and label names, never
//! anyone's mail, and only proposes; the member previews and saves.

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
pub mod scopes;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "assistant";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// What the module is composed with.
#[derive(Debug, Clone, Default)]
pub struct AssistantDeps {
    /// The AI gateway; compiling a sentence needs its `complete` role.
    pub ai: Option<Gateway>,
}

/// The `assistant` module, composed with `deps`, ready for the server to
/// mount.
pub fn module(deps: AssistantDeps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router().layer(axum::Extension(features::compile::Compiler::new(deps.ai))),
        admin_router: None,
        background: Background::new()
            .on_event::<features::purge::ScheduleWorkspacePurge>()
            .on_event::<features::purge::ForgetMember>()
            .job::<features::purge::PurgeWorkspace>(),
    }
}

/// The kernel's `MemberPreferences` port, answered from members' profiles.
pub fn preferences() -> MemberPreferencesPort {
    Arc::new(features::profile::Preferences)
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{compile, profile};
    OpenApiRouter::new()
        .routes(routes!(profile::get, profile::update))
        .routes(routes!(compile::handler))
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
