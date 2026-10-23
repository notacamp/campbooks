//! `campbooks-schedule` — the `schedule` module (`docs/domain-map.md` →
//! *schedule*, campbooks-cloud#42).
//!
//! It keeps each member's calendars and their events: the provider calendars
//! a calendar grant brings (two-way sync through `connectors`) and the
//! member's own Campbooks calendar, which holds what Scout read from email
//! (`conversations.message_read`) and which no provider sees. Events have a
//! kind from a fixed set; recurring series are stored once and expanded when
//! listed; the member has a time zone and working hours, from which free
//! time is answered. Obligations (pay by, submit by) are not events: they are
//! asks owned by `tasks`.
//!
//! Phase 1 of the change ships the module's shell: its scopes, its schema
//! (migration `schedule__init`) and the events it publishes. The use cases
//! (`features/`) follow in the next phase.

#![forbid(unsafe_code)]

use campbooks_kernel::{AppState, Background, Module, authz::Resources};
use utoipa_axum::router::OpenApiRouter;

#[cfg(test)]
mod contract_tests;
pub mod events;
mod features;
pub mod model;
pub mod scopes;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "schedule";

/// The module's part of the access catalogue (ADR 0017). None yet: the
/// calendar and event types are declared with the first slices that serve
/// them, together with the facts port that answers whose they are.
static RESOURCES: [campbooks_kernel::authz::ResourceType; 0] = [];

/// The `schedule` module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router(),
        admin_router: None,
        background: Background::new()
            .publishes::<events::EventCreated>()
            .publishes::<events::EventChanged>()
            .publishes::<events::EventCancelled>()
            .publishes::<events::OutboundRequested>(),
        resources: Resources::declared(&RESOURCES),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(super::module().name, "schedule");
    }

    #[test]
    fn it_declares_both_scopes() {
        let names: Vec<&str> = super::module().scopes.iter().map(|s| s.name).collect();
        assert_eq!(names, ["schedule:read", "schedule:write"]);
    }

    // A runtime query rather than `query!`, so this test needs no entry in
    // the `.sqlx` offline cache.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_schedule_schema(db: PgPool) {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
                .bind(super::NAME)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(exists);
    }
}
