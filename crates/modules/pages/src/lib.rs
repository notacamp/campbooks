//! `campbooks-pages` — what members write for themselves and their team
//! (`docs/domain-map.md` → *pages*).
//!
//! A **page** has a title, an icon, a parent (or none), a place (`private`
//! or `workspace`) and a body: a block document kept as a Yjs document and
//! merged by the server (`yrs`), so two people editing one page never
//! overwrite each other. Every update a client sends is read against the
//! block schema before anything is stored (`domain::document`). A body
//! points at people, threads, tasks, events and other pages by reference,
//! never by copying what they say (`pages.links`, backlinks); who may open a
//! page follows its place, its ancestors and who it is shared with
//! (`domain::roles`). Deleted pages wait 30 days in the trash. The module
//! publishes `pages.page_changed`, ids and enums only, and uses no model.
//!
//! Off by default: `CAMPBOOKS_PAGES__ENABLED` (see `config`).

#![forbid(unsafe_code)]

use axum::{
    extract::{DefaultBodyLimit, Request},
    middleware::{self, Next},
    response::IntoResponse,
};
use campbooks_kernel::{AppState, Background, Error, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod config;
pub mod domain;
pub mod events;
mod features;
pub mod scopes;

pub use config::Deps;
pub use domain::{resources::visibility, search::source as search_source};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "pages";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The `pages` module, composed with `deps`, ready for the server to mount.
/// While pages are off, every endpoint answers 404 as for a route that does
/// not exist, and the daily trash purge is not scheduled.
pub fn module(deps: Deps) -> Module {
    let mut background = Background::new()
        .publishes::<events::PageChanged>()
        .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
        .job::<features::purge_workspace::PurgeWorkspace>()
        .job::<features::compact::Compact>()
        .job::<features::purge_trash::PurgeTrash>();
    if deps.enabled {
        background = background
            .schedule::<features::purge_trash::DispatchPurgeTrash>(features::purge_trash::EVERY);
    }
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: router().layer(middleware::from_fn(
            move |request: Request, next: Next| async move {
                if deps.enabled {
                    next.run(request).await
                } else {
                    Error::NotFound.into_response()
                }
            },
        )),
        admin_router: None,
        background,
        resources: domain::resources::resources(),
    }
}

/// Every route of the module, one per feature.
fn router() -> OpenApiRouter<AppState> {
    use features::{
        backlinks, create, file, get, list, move_page, share, sync, trash, update, upload,
    };
    // A file is up to 20 MiB; everything else is small.
    let uploads = OpenApiRouter::new()
        .routes(routes!(upload::handler))
        .layer(DefaultBodyLimit::max(upload::MAX_FILE_BYTES));
    OpenApiRouter::new()
        .routes(routes!(list::handler, create::handler))
        .routes(routes!(backlinks::handler))
        .routes(routes!(get::handler, update::handler, trash::purge))
        .routes(routes!(move_page::handler))
        .routes(routes!(sync::handler))
        .routes(routes!(trash::trash))
        .routes(routes!(trash::restore))
        .routes(routes!(share::share, share::unshare))
        .routes(routes!(file::handler))
        .merge(uploads)
}

#[cfg(test)]
mod access_tests;
#[cfg(test)]
mod testkit;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(module(Deps::default()).name, "pages");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn its_background_work_registers() {
        let registry = campbooks_kernel::jobs::Registry::build(&[module(Deps::default())]).unwrap();
        let kinds = registry.kind_names();
        for kind in [
            "pages.schedule_workspace_purge",
            "pages.purge_workspace",
            "pages.compact",
            "pages.purge_trash",
            "pages.dispatch_purge_trash",
        ] {
            assert!(kinds.contains(&kind), "{kind}");
        }
        assert!(
            registry
                .schedule_names()
                .contains(&"pages.dispatch_purge_trash")
        );
    }

    #[test]
    fn off_it_schedules_nothing() {
        let off = Deps { enabled: false };
        let registry = campbooks_kernel::jobs::Registry::build(&[module(off)]).unwrap();
        assert!(registry.schedule_names().is_empty());
    }
}
