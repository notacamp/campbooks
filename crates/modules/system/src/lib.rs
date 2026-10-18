//! `campbooks-system` — instance health and version, the app's own error
//! and timing reports (forwarded to the operator's collector), the screens
//! the app shows (relayed to the operator's usage analytics), the admin dashboard's
//! System panel (`GET /admin/v1/system/overview`, ADR 0015), and the AI
//! usage ledger with its daily cap ([`AiUsageMeter`], read by
//! `GET /admin/v1/system/ai-usage`).
//!
//! This is the **reference vertical slice**: copy its shape for every new
//! module. `lib.rs` only assembles [`module`]; each use case lives in its own
//! folder under `features/`, owning its handler, request/response types,
//! queries and tests. Routes are declared with `utoipa-axum` so they are in
//! the OpenAPI document by construction, and each one states its security:
//! a scope from [`scopes`] (`security(("bearer" = ["system:read"]))` plus the
//! `Scoped<SystemRead>` extractor), or explicitly public (`security(())`).
//! Each one is an entry of the operation catalogue (ADR 0013): an
//! `operation_id` named `system.<verb>[_<noun>]` and an `x-campbooks`
//! extension built from `campbooks_kernel::Operation`.

#![forbid(unsafe_code)]

use axum::Extension;
use campbooks_kernel::{Background, Module, i18n::Catalogue};
use utoipa_axum::{router::OpenApiRouter, routes};

mod deps;
mod features;
pub mod scopes;
#[cfg(test)]
mod testkit;

pub use deps::{Deps, Facts, IdentityProvider};
pub use features::ai_meter::AiUsageMeter;
/// The health check's handler, for the server to also mount at the probe
/// path `/up` (outside the API's OpenAPI document).
pub use features::health::handler as health;
pub use features::screen_views::ScreenViewRelay;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "system";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The system module composed with nothing (for catalogues and tests).
/// The module's part of the access catalogue (ADR 0017).
static RESOURCES: [campbooks_kernel::authz::ResourceType; 4] = {
    use campbooks_kernel::authz::{Action, Grant, Grantee, Owner, ResourceType, Tenancy};
    [
        ResourceType {
            grants: &[Grant::new(Grantee::Anyone, &[Action::Read])],
            ..ResourceType::new(
                "system.instance",
                "The instance's health and version: public.",
                Tenancy::Public,
            )
        },
        ResourceType {
            owner: Owner::Account,
            owner_actions: &[Action::Create],
            ..ResourceType::new(
                "system.client_report",
                "A report of the app's own errors and timings, sent by a signed-in person's \
                 app and forwarded, scrubbed, to the operator's collector: never read back.",
                Tenancy::Account,
            )
        },
        ResourceType {
            owner: Owner::Account,
            owner_actions: &[Action::Create],
            ..ResourceType::new(
                "system.screen_view",
                "A screen the signed-in person's app shows, as its route template, relayed to \
                 the operator's usage analytics under a daily visitor id: never stored or read \
                 back.",
                Tenancy::Account,
            )
        },
        ResourceType {
            grants: &[Grant::new(Grantee::InstanceAdmins, &[Action::Read])],
            ..ResourceType::new(
                "system.admin_overview",
                "The instance's configuration and health as administrators see it (the admin \
                 API, `Admin<P>`): never workspace content.",
                Tenancy::Instance,
            )
        },
    ]
};

pub fn module() -> Module {
    module_with(Deps::default())
}

/// The system module composed with `deps`, ready for the server to mount.
pub fn module_with(deps: Deps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new()
            .routes(routes!(features::screen_views::handler))
            // Only the route above sees the relay.
            .layer(Extension(deps.screen_views.clone()))
            .routes(routes!(features::health::handler))
            .routes(routes!(features::version::handler))
            .routes(routes!(features::client_telemetry::handler)),
        admin_router: Some(
            OpenApiRouter::new()
                .routes(routes!(features::admin_overview::handler))
                .routes(routes!(features::admin_ai_usage::handler))
                .routes(routes!(features::admin_jobs::summary))
                .routes(routes!(features::admin_jobs::failed))
                .routes(routes!(features::admin_job_actions::retry))
                .routes(routes!(features::admin_job_actions::discard))
                .routes(routes!(features::admin_job_actions::retry_kind))
                .routes(routes!(features::admin_job_actions::discard_schedule_run))
                .layer(Extension(deps)),
        ),
        background: Background::new(),
        resources: campbooks_kernel::authz::Resources::declared(&RESOURCES),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(super::module().name, "system");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(super::MESSAGES.check(), Vec::<String>::new());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn migrations_create_the_system_schema(db: PgPool) {
        let id = sqlx::query_scalar!(
            "INSERT INTO system.instance (id) VALUES (gen_random_uuid()) RETURNING id"
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let count = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM system.instance WHERE id = $1"#,
            id
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }
}
