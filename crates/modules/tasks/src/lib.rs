//! `campbooks-tasks` — what each member has to do (`docs/domain-map.md` →
//! *tasks*).
//!
//! A **task** is something a member has to do or pay: a title in their own
//! words, a due day (and perhaps a time), a plan (a day, or Later until a
//! time), one assignee and, optionally, the conversation it came from. It is
//! personal: its assignee reads and changes it, and the member who handed it
//! to a teammate still sees it ("with <name>") and may take it back. Every
//! change appends to the task's history and publishes
//! `tasks.task_changed`, a snapshot of ids, enums and dates that `attention`
//! projects into Home's Do and Pay. The module uses no model.

#![forbid(unsafe_code)]

use campbooks_kernel::{AppState, Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod domain;
pub mod events;
mod features;
pub mod inbound;
pub mod scopes;

pub use domain::resources::visibility;
pub use features::open_tasks::open_tasks;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "tasks";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// This module's section of Scout's handbook (campbooks-cloud#305): what a
/// task is, its plan and Later, hand-off, and that Scout only reads tasks. The
/// server composes it into the assistant's handbook; at most 8 KiB.
pub const HANDBOOK: &str = include_str!("../handbook.md");

/// The `tasks` module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router(),
        admin_router: None,
        background: Background::new()
            .publishes::<events::TaskChanged>()
            .on_event::<features::add_read_asks::AddReadAsks>()
            .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
            .job::<features::purge_workspace::PurgeWorkspace>(),
        resources: domain::resources::resources(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{create, get, hand_off, list, plan, status, update};
    OpenApiRouter::new()
        .routes(routes!(list::handler, create::handler))
        .routes(routes!(get::handler, update::handler))
        .routes(routes!(plan::handler))
        .routes(routes!(status::done))
        .routes(routes!(status::not_needed))
        .routes(routes!(status::reopen))
        .routes(routes!(hand_off::hand_off))
        .routes(routes!(hand_off::take_back))
}

#[cfg(test)]
mod testkit;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(module().name, "tasks");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn its_background_work_registers() {
        let registry = campbooks_kernel::jobs::Registry::build(&[module()]).unwrap();
        let kinds = registry.kind_names();
        for kind in ["tasks.schedule_workspace_purge", "tasks.purge_workspace"] {
            assert!(kinds.contains(&kind), "{kind}");
        }
    }
}
