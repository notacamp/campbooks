//! `GET /api/v1/system/health` — is this instance able to serve requests?
//!
//! Used by load balancers and the container `HEALTHCHECK`. Healthy means the
//! process is up **and** the database answers.

use std::time::Duration;

use axum::{Json, extract::State};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

/// How long the database gets to answer before the instance counts as down.
const DATABASE_TIMEOUT: Duration = Duration::from_secs(3);

/// The instance is healthy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Overall status; always `ok` in a 200 response.
    #[schema(example = "ok")]
    pub status: String,
    /// Database status; always `ok` in a 200 response.
    #[schema(example = "ok")]
    pub database: String,
}

impl HealthResponse {
    fn ok() -> Self {
        Self {
            status: "ok".to_owned(),
            database: "ok".to_owned(),
        }
    }
}

/// Report instance health.
#[utoipa::path(
    get,
    path = "/api/v1/system/health",
    operation_id = "system.health",
    tag = "system",
    // Public: probes and support tools call it without a token.
    security(()),
    // A probe: neither rules nor the assistant have a reason to call it.
    extensions(("x-campbooks" = json!(Operation::query()))),
    responses(
        (status = OK, description = "The instance and its database are up.", body = HealthResponse),
        (status = SERVICE_UNAVAILABLE, description = "The database is unreachable.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(State(state): State<AppState>) -> Result<Json<HealthResponse>> {
    match tokio::time::timeout(DATABASE_TIMEOUT, ping(&state.db)).await {
        Ok(Ok(())) => Ok(Json(HealthResponse::ok())),
        Ok(Err(error)) => {
            tracing::warn!(%error, "health check: database unreachable");
            Err(Error::Unavailable("database unreachable".to_owned()))
        }
        Err(_) => {
            tracing::warn!("health check: database timed out");
            Err(Error::Unavailable("database timed out".to_owned()))
        }
    }
}

async fn ping(db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
        .fetch_one(db)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::testing;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn ok_when_database_answers(db: PgPool) {
        let app = testing::app(crate::module(), testing::state(db));
        let response = testing::get(&app, "/api/v1/system/health").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.json,
            serde_json::json!({"status": "ok", "database": "ok"})
        );
    }

    #[tokio::test]
    async fn unavailable_problem_when_database_is_down() {
        let app = testing::app(crate::module(), testing::state(testing::unreachable_pool()));
        let response = testing::get(&app, "/api/v1/system/health").await;

        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/problem+json")
        );
        assert_eq!(response.json["status"], 503);
        assert_eq!(response.json["title"], "Service Unavailable");
    }
}
