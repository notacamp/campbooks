//! `campbooks-attention` — one queue of what needs each member
//! (`docs/domain-map.md` → *attention*).
//!
//! An **attention item** points at a record of another module (a
//! conversation) with a **reason** that module declared (review, reply,
//! decide, do, pay), a due date and the container that decides who may see
//! it. Items are a projection of the owning modules' events
//! (`conversations.need_changed`), rebuilt by replaying them; they hold ids,
//! enums and dates only, never names or text, so the owning module renders
//! each row. The member's own "Later" (a snooze) is this module's write
//! model. Who sees an item is asked of the kernel's `Visibility` port.

#![forbid(unsafe_code)]

use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod domain;
pub mod events;
mod features;
pub mod scopes;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "attention";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The `attention` module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router(),
        admin_router: None,
        background: Background::new()
            .publishes::<events::ItemChanged>()
            .on_event::<features::project_need::ProjectConversationNeed>()
            .on_event::<features::project_task::ProjectTask>()
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .on_event::<features::purge_workspace::ForgetMember>()
            .on_event::<features::purge_workspace::ForgetInbox>()
            .job::<features::purge_workspace::PurgeWorkspace>(),
        resources: domain::resources::resources(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{list, snooze};
    OpenApiRouter::new()
        .routes(routes!(list::handler))
        .routes(routes!(snooze::snooze))
        .routes(routes!(snooze::unsnooze))
}

#[cfg(test)]
mod testkit;

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(module().name, "attention");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn its_background_work_registers() {
        let registry = campbooks_kernel::jobs::Registry::build(&[module()]).unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "attention.project_conversation_need",
            "attention.project_task",
            "attention.schedule_workspace_purge",
            "attention.forget_member",
            "attention.purge_workspace",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
    }

    // A runtime query rather than `query!`, so this test needs no entry in
    // the `.sqlx` offline cache.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_attention_schema(db: PgPool) {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
                .bind(super::NAME)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(exists);
    }
}
