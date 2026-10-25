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
//! - **Calendars** ([`features::grant_calendars`], [`features::calendars`]):
//!   one Campbooks calendar per member, made the first time it is needed,
//!   and one calendar per calendar of a granted provider account.
//! - **From the provider** ([`features::apply_calendar_changes`]): batches
//!   of upserts and removals, series stored once with the provider's own
//!   recurrence lines; the outcome of the member's own changes
//!   ([`features::apply_outcome`]).
//! - **From email** ([`features::add_read_items`]): the items of a read
//!   become events on the inbox owner's Campbooks calendar.
//! - **Operations** under `/api/v1/w/{workspace_id}/schedule/…`: list, read,
//!   make, change (an event, or one occurrence), delete and restore,
//!   answer, add to a provider calendar and take it away again, calendars,
//!   settings and free time. A change to an event on a provider calendar is
//!   stored at once, marked pending, and asked of connectors in the same
//!   transaction ([`domain::outbound`]); the kernel's calendar-requests port
//!   ([`calendar_requests`]) tells connectors it still stands.
//! - **Ports** for other modules: [`time_zones`] and [`availability`].

#![forbid(unsafe_code)]

use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

#[cfg(test)]
mod contract_tests;
pub mod domain;
pub mod events;
mod features;
pub mod inbound;
pub mod model;
pub mod scopes;
#[cfg(test)]
mod testkit;
/// The module's tracked events, generated from the tracking plan (ADR 0020).
#[path = "tracking.gen.rs"]
pub mod tracking;
#[cfg(test)]
mod tracking_tests;

pub use features::{
    calendar_requests::calendar_requests, free_slots::availability, time_zones::time_zones,
};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "schedule";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// This module's section of Scout's handbook (campbooks-cloud#305): what
/// event kinds are, the Campbooks calendar beside provider calendars, that
/// obligations are tasks shown as Due, and that Scout never writes to a
/// provider without the member. The server composes it into the assistant's
/// handbook; at most 8 KiB.
pub const HANDBOOK: &str = include_str!("../handbook.md");

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
            .publishes::<events::OutboundRequested>()
            .publishes::<events::CalendarSyncChanged>()
            .on_event::<features::grant_calendars::GrantCalendars>()
            .on_event::<features::apply_calendar_changes::ApplyCalendarChanges>()
            .on_event::<features::apply_outcome::ApplyOutcome>()
            .on_event::<features::add_read_items::AddReadItems>()
            .on_event::<features::forget::ForgetMember>()
            .on_event::<features::forget::ForgetConnection>()
            .on_event::<features::forget::ScheduleWorkspacePurge>()
            .rekey::<features::retag_bindings::RetagBindings>()
            .job::<features::forget::PurgeWorkspace>()
            .job::<features::delete_event::ReleaseDelete>(),
        resources: domain::resources::resources(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{
        calendars, create_event, delete_event, free_slots, get_event, list_events, push_event,
        respond_event, restore_event, settings, update_event,
    };
    OpenApiRouter::new()
        .routes(routes!(list_events::handler, create_event::handler))
        .routes(routes!(
            get_event::handler,
            update_event::handler,
            delete_event::handler
        ))
        .routes(routes!(restore_event::handler))
        .routes(routes!(respond_event::handler))
        .routes(routes!(push_event::push, push_event::unpush))
        .routes(routes!(calendars::list))
        .routes(routes!(calendars::update))
        .routes(routes!(settings::get, settings::update))
        .routes(routes!(free_slots::handler))
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

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(super::MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn its_background_work_registers() {
        let registry = campbooks_kernel::jobs::Registry::build(&[super::module()]).unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "schedule.grant_calendars",
            "schedule.apply_calendar_changes",
            "schedule.apply_outcome",
            "schedule.add_read_items",
            "schedule.forget_member",
            "schedule.forget_connection",
            "schedule.schedule_workspace_purge",
            "schedule.purge_workspace",
            "schedule.release_delete",
            "schedule.retag_bindings",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
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

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_calendar_may_name_its_provider(db: PgPool) {
        let column: Option<String> = sqlx::query_scalar(
            "SELECT data_type FROM information_schema.columns
              WHERE table_schema = 'schedule' AND table_name = 'calendars'
                AND column_name = 'provider'",
        )
        .fetch_optional(&db)
        .await
        .unwrap();
        assert_eq!(column.as_deref(), Some("text"));
        let known: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_constraint
                             WHERE conrelid = 'schedule.calendars'::regclass
                               AND pg_get_constraintdef(oid) LIKE '%google%zoho%')",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(known);
    }
}
