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
pub mod properties;
pub mod scopes;
/// The module's tracked events, generated from the tracking plan (ADR 0020).
#[path = "tracking.gen.rs"]
pub mod tracking;

pub use config::Deps;
pub use domain::{
    notion::requests::page_requests, resources::visibility, search::source as search_source,
};

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
        .publishes::<events::TableChanged>()
        .publishes::<events::NotionLinkRequested>()
        .publishes::<events::NotionLinkEnded>()
        .publishes::<events::NotionChangesRequested>()
        .on_event::<features::purge_workspace::ScheduleWorkspacePurge>()
        .on_event::<features::forget_member::ForgetMember>()
        .on_event::<features::apply_notion_snapshot::ApplyNotionSnapshot>()
        .on_event::<features::complete_notion_changes::CompleteNotionChanges>()
        .on_event::<features::stop_notion_link::StopNotionLink>()
        .on_event::<features::stop_notion_links_of_connection::StopNotionLinksOfConnection>()
        .job::<features::push_to_notion::PushToNotion>()
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
        router: router(deps).layer(middleware::from_fn(
            move |mut request: Request, next: Next| async move {
                if deps.enabled {
                    request
                        .extensions_mut()
                        .insert(config::TablesOn(deps.tables));
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
fn router(deps: Deps) -> OpenApiRouter<AppState> {
    use features::{
        add_property, add_row, backlinks, create, create_table, create_view, file, get, get_table,
        import_from_notion, list, list_notion_links, move_page, query_table, remove_property,
        remove_view, resolve_conflict, restore_property, set_cells, set_options, share, stop_sync,
        sync, trash, undo_resolve_conflict, update, update_property, update_view, upload,
    };
    // A file is up to 20 MiB; everything else is small.
    let uploads = OpenApiRouter::new()
        .routes(routes!(upload::handler))
        .layer(DefaultBodyLimit::max(upload::MAX_FILE_BYTES));
    // Tables are a switch of their own: off, these routes are not there.
    let tables = OpenApiRouter::new()
        .routes(routes!(create_table::handler))
        .routes(routes!(get_table::handler))
        .routes(routes!(add_property::handler))
        .routes(routes!(update_property::handler, remove_property::handler))
        .routes(routes!(restore_property::handler))
        .routes(routes!(set_options::handler))
        .routes(routes!(add_row::handler))
        .routes(routes!(set_cells::handler))
        .routes(routes!(query_table::handler))
        .routes(routes!(create_view::handler))
        .routes(routes!(update_view::handler, remove_view::handler))
        .layer(middleware::from_fn(
            move |request: Request, next: Next| async move {
                if deps.tables {
                    next.run(request).await
                } else {
                    Error::NotFound.into_response()
                }
            },
        ));
    OpenApiRouter::new()
        .merge(tables)
        .routes(routes!(list::handler, create::handler))
        .routes(routes!(backlinks::handler))
        .routes(routes!(import_from_notion::handler))
        .routes(routes!(list_notion_links::handler))
        .routes(routes!(resolve_conflict::handler))
        .routes(routes!(undo_resolve_conflict::handler))
        .routes(routes!(stop_sync::handler))
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
mod testkit_notion;
#[cfg(test)]
mod tracking_tests;

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
            "pages.forget_member",
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
        let off = Deps {
            enabled: false,
            tables: false,
        };
        let registry = campbooks_kernel::jobs::Registry::build(&[module(off)]).unwrap();
        assert!(
            !registry
                .schedule_names()
                .contains(&"pages.dispatch_purge_trash")
        );
    }
}
