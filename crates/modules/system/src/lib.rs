//! `campbooks-system` — instance health and version, and the admin
//! dashboard's System panel (`GET /admin/v1/system/overview`, ADR 0015).
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

pub use deps::{Deps, Facts, IdentityProvider};

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "system";

/// The module's messages (`i18n/<locale>.json`): the reasons of its errors.
pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!(NAME);

/// The system module composed with nothing (for catalogues and tests).
pub fn module() -> Module {
    module_with(Deps::default())
}

/// The system module composed with `deps`, ready for the server to mount.
pub fn module_with(deps: Deps) -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new()
            .routes(routes!(features::health::handler))
            .routes(routes!(features::version::handler)),
        admin_router: Some(
            OpenApiRouter::new()
                .routes(routes!(features::admin_overview::handler))
                .layer(Extension(deps)),
        ),
        background: Background::new(),
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
