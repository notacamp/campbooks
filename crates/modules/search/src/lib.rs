//! `campbooks-search` — one index over every record a member may see
//! (`docs/domain-map.md` → *search*, ADR 0016).
//!
//! - **The index** is a projection in schema `search`: *groups* (what a
//!   result is shown as, a conversation, in a container such as an inbox)
//!   and their *entries* (the text inside, its messages), with the text, a
//!   weighted `tsvector` in the entry's language, the people and the
//!   non-personal facets filters need.
//! - **Kept current** from the sources' ordinary events (ids only): each
//!   names a group, whose current state is pulled through the kernel's
//!   `SearchSource` port and written in the delivery's transaction. A
//!   workspace's first event or first query starts its backfill, which pages
//!   through the same port.
//! - **Queried** by `search.query`: Gmail-style operators, the four shipped
//!   languages with accents ignored, highlights as text segments, and only
//!   the containers the caller may see, asked from the visibility port at
//!   query time.

#![forbid(unsafe_code)]

use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod domain;
pub mod events;
mod features;
pub mod scopes;

pub use features::backfill::Backfill;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "search";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The `search` module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router(),
        background: Background::new()
            .on_event::<features::refresh::OnMessageReceived>()
            .on_event::<features::refresh::OnMessageSent>()
            .on_event::<features::refresh::OnConversationChanged>()
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .job::<features::purge_workspace::PurgeWorkspace>()
            .job::<features::backfill::Backfill>(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(features::query::handler))
}

#[cfg(test)]
mod testkit;

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
        assert_eq!(module().name, "search");
    }

    #[test]
    fn its_background_work_registers() {
        let registry = campbooks_kernel::jobs::Registry::build(&[module()]).unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "search.on_message_received",
            "search.on_message_sent",
            "search.on_conversation_changed",
            "search.schedule_workspace_purge",
            "search.purge_workspace",
            "search.backfill",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
    }

    // A runtime query rather than `query!`, so this test needs no entry in
    // the `.sqlx` offline cache.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_search_schema(db: PgPool) {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
                .bind(super::NAME)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(exists);
    }
}
