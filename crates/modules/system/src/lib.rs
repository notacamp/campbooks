//! `campbooks-system` — instance health and version.
//!
//! This is the **reference vertical slice**: copy its shape for every new
//! module. `lib.rs` only assembles [`module`]; each use case lives in its own
//! folder under `features/`, owning its handler, request/response types,
//! queries and tests. Routes are declared with `utoipa-axum` so they are in
//! the OpenAPI document by construction, and each one states its security:
//! a scope from [`scopes`] (`security(("bearer" = ["system:read"]))` plus the
//! `Scoped<SystemRead>` extractor), or explicitly public (`security(())`).

#![forbid(unsafe_code)]

use campbooks_kernel::Module;
use utoipa_axum::{router::OpenApiRouter, routes};

mod features;
pub mod scopes;

/// Module name: crate directory, Postgres schema and OpenAPI tag.
pub const NAME: &str = "system";

/// The system module, ready for the server to mount.
pub fn module() -> Module {
    Module {
        name: NAME,
        scopes: scopes::ALL,
        router: OpenApiRouter::new()
            .routes(routes!(features::health::handler))
            .routes(routes!(features::version::handler)),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    #[test]
    fn module_is_named_after_its_schema() {
        assert_eq!(super::module().name, "system");
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
