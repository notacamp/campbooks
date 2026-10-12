//! `GET /api/v1/system/version` — which build is running?

use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The running build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct VersionResponse {
    /// Semantic version of the server.
    #[schema(example = "0.1.0")]
    pub version: String,
    /// Git commit the build came from, when known (`GIT_SHA` at runtime).
    #[schema(example = "3f2c1e0")]
    pub commit: Option<String>,
}

impl VersionResponse {
    fn new(commit: Option<String>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: commit
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty()),
        }
    }
}

/// Report the server version.
#[utoipa::path(
    get,
    path = "/api/v1/system/version",
    operation_id = "system_version",
    tag = "system",
    responses(
        (status = OK, description = "The running build.", body = VersionResponse),
    )
)]
pub async fn handler() -> Json<VersionResponse> {
    Json(VersionResponse::new(std::env::var("GIT_SHA").ok()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::testing;

    use super::*;

    #[tokio::test]
    async fn reports_crate_version() {
        let app = testing::app(crate::module(), testing::state(testing::unreachable_pool()));
        let response = testing::get(&app, "/api/v1/system/version").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn commit_is_trimmed_and_blank_means_unknown() {
        assert_eq!(
            VersionResponse::new(Some(" abc123\n".into()))
                .commit
                .as_deref(),
            Some("abc123")
        );
        assert_eq!(VersionResponse::new(Some("  ".into())).commit, None);
        assert_eq!(VersionResponse::new(None).commit, None);
    }
}
