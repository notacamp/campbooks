//! `GET /api/v1/connectors/overview` — overview.
//!
//! TODO(new-feature): say what this use case answers and replace the
//! placeholder response. The feature owns its handler, request and response
//! types, queries and tests; anything another feature of this module needs
//! too moves to `domain/`.

use axum::Json;
use campbooks_kernel::{Operation, Problem, auth::Scoped};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::scopes::ConnectorsRead;

/// The answer to `overview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OverviewResponse {
    /// Who asked: the access token's subject.
    #[schema(example = "user-123")]
    pub subject: String,
}

/// Overview.
#[utoipa::path(
    get,
    path = "/api/v1/connectors/overview",
    // `<module>.<verb>[_<noun>]`: a wire contract, never renamed (ADR 0013).
    operation_id = "connectors.overview",
    tag = "connectors",
    security(("bearer" = ["connectors:read"])),
    // TODO(new-feature): list the record types it touches
    // (`.records(["connectors.<record>"])`), and opt in `.automation()` /
    // `.assistant()` if rules or the assistant may call it. A write starts
    // from `Operation::command(Risk::Low | Risk::High)` and names its
    // inverse with `.undo("connectors.<verb>")`.
    extensions(("x-campbooks" = json!(Operation::query()))),
    responses(
        (status = OK, description = "The answer for the caller.", body = OverviewResponse),
        (status = UNAUTHORIZED, description = "No valid access token.",
            body = Problem, content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `connectors:read`.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(Scoped { principal, .. }: Scoped<ConnectorsRead>) -> Json<OverviewResponse> {
    // To read this module's tables, also take `State(state): State<AppState>`,
    // query `state.db` with `sqlx::query!` (then refresh the offline cache:
    // `cargo sqlx prepare --workspace -- --all-targets`), return
    // `campbooks_kernel::Result<Json<_>>`, and answer `Error::NotFound` for a
    // record the principal may not see, never 403.
    Json(OverviewResponse {
        subject: principal.subject,
    })
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use campbooks_kernel::testing::{self, TestIdp, TestResponse};

    const PATH: &str = "/api/v1/connectors/overview";

    /// `GET PATH` with a token granting `scope`.
    async fn call_with_scope(idp: &TestIdp, scope: &str) -> TestResponse {
        let app = testing::app(crate::module(), idp.state(testing::unreachable_pool()));
        let token = idp.token(&idp.claims(scope));
        let request = Request::get(PATH)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        testing::send(&app, request).await
    }

    #[tokio::test]
    async fn answers_a_caller_with_the_read_scope() {
        let idp = TestIdp::start().await;
        let response = call_with_scope(&idp, "connectors:read").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["subject"], "test-user");
    }

    #[tokio::test]
    async fn refuses_a_token_without_the_read_scope() {
        let idp = TestIdp::start().await;
        let response = call_with_scope(&idp, "connectors:write").await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn refuses_an_anonymous_caller() {
        let idp = TestIdp::start().await;
        let app = testing::app(crate::module(), idp.state(testing::unreachable_pool()));
        let response = testing::get(&app, PATH).await;

        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
}
