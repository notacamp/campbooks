//! The one error type handlers return, rendered as RFC 9457
//! `application/problem+json`.
//!
//! Permission failures on *reads* must use [`Error::NotFound`], not
//! [`Error::Forbidden`], so a caller cannot probe for records it may not see.
//! [`Error::Unauthorized`] and [`Error::InsufficientScope`] are about the
//! access token itself (RFC 6750) and come from the
//! [`Scoped`](crate::auth::Scoped) extractor, never from resource checks.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::AuthFailure;

/// Media type of problem details (RFC 9457 §3).
pub const PROBLEM_JSON: &str = "application/problem+json";

/// Errors every module can return from a handler.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The resource does not exist, or the caller may not see it.
    #[error("not found")]
    NotFound,
    /// The request is malformed; the message is shown to the client.
    #[error("{0}")]
    BadRequest(String),
    /// No valid access token was presented. The reason's code is the
    /// problem `detail`, and the response carries a `WWW-Authenticate:
    /// Bearer` challenge (RFC 6750 §3).
    #[error("unauthorized: {}", .0.code())]
    Unauthorized(AuthFailure),
    /// The access token is valid but lacks the scope the endpoint requires
    /// (RFC 6750 `insufficient_scope`); carries that scope's name.
    #[error("insufficient scope: {0}")]
    InsufficientScope(&'static str),
    /// Credentials are valid but lack permission for this *action*.
    #[error("forbidden")]
    Forbidden,
    /// This *action* is refused for a stated reason the client can act on
    /// (e.g. `signup_invite_only`); the machine-readable code is the problem
    /// `detail`. Like [`Error::Forbidden`], never for reads.
    #[error("denied: {0}")]
    Denied(&'static str),
    /// The request conflicts with the current state; the message is shown.
    #[error("{0}")]
    Conflict(String),
    /// A dependency (database, provider) is unavailable; the message is shown.
    #[error("{0}")]
    Unavailable(String),
    /// Anything else. Logged with its source; the client sees a generic message.
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Kernel result alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Wrap any error as [`Error::Internal`].
    pub fn internal(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Internal(err.into())
    }

    /// HTTP status this error maps to.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::InsufficientScope(_) | Self::Forbidden | Self::Denied(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The problem document sent to the client. Never includes internal detail.
    pub fn to_problem(&self) -> Problem {
        let detail = match self {
            Self::BadRequest(msg) | Self::Conflict(msg) | Self::Unavailable(msg) => {
                Some(msg.clone())
            }
            Self::Unauthorized(failure) => Some(failure.code().to_owned()),
            Self::InsufficientScope(_) => Some("insufficient_scope".to_owned()),
            Self::Denied(code) => Some((*code).to_owned()),
            Self::NotFound | Self::Forbidden | Self::Internal(_) => None,
        };
        Problem::new(self.status(), detail)
    }

    /// The `WWW-Authenticate` challenge for token failures (RFC 6750 §3).
    pub fn www_authenticate(&self) -> Option<String> {
        match self {
            Self::Unauthorized(failure) => Some(failure.challenge().to_owned()),
            Self::InsufficientScope(scope) => Some(format!(
                "Bearer error=\"insufficient_scope\", scope=\"{scope}\""
            )),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::NotFound,
            other => Self::internal(other),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        if let Self::Internal(source) = &self {
            tracing::error!(error = %source, source_debug = ?source, "internal error");
        }
        let challenge = self
            .www_authenticate()
            .and_then(|value| HeaderValue::from_str(&value).ok());
        let mut response = self.to_problem().into_response();
        if let Some(challenge) = challenge {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, challenge);
        }
        response
    }
}

/// RFC 9457 problem details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "type": "about:blank",
    "title": "Not Found",
    "status": 404
}))]
pub struct Problem {
    /// URI identifying the problem type; `about:blank` when the HTTP status
    /// says it all.
    #[serde(rename = "type")]
    pub kind: String,
    /// Short, human-readable summary of the problem type.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Human-readable explanation specific to this occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Problem {
    /// A problem of type `about:blank` whose title is the status reason phrase.
    pub fn new(status: StatusCode, detail: Option<String>) -> Self {
        Self {
            kind: "about:blank".to_owned(),
            title: status.canonical_reason().unwrap_or("Error").to_owned(),
            status: status.as_u16(),
            detail,
        }
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, Json(self)).into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
        response
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;

    use super::*;

    async fn render(err: Error) -> (StatusCode, String, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let content_type = response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, content_type, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn not_found_is_problem_json() {
        let (status, content_type, body) = render(Error::NotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(content_type, PROBLEM_JSON);
        assert_eq!(
            body,
            serde_json::json!({"type": "about:blank", "title": "Not Found", "status": 404})
        );
    }

    #[tokio::test]
    async fn bad_request_carries_detail() {
        let (status, _, body) = render(Error::BadRequest("missing field `to`".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["detail"], "missing field `to`");
    }

    #[tokio::test]
    async fn internal_hides_detail() {
        let (status, _, body) = render(Error::internal("password=hunter2 leaked")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body.get("detail").is_none());
        assert!(!body.to_string().contains("hunter2"));
    }

    #[tokio::test]
    async fn invalid_token_is_401_with_challenge() {
        let response = Error::Unauthorized(AuthFailure::InvalidToken).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            r#"Bearer error="invalid_token""#
        );
        let (_, content_type, body) = render(Error::Unauthorized(AuthFailure::InvalidToken)).await;
        assert_eq!(content_type, PROBLEM_JSON);
        assert_eq!(body["detail"], "invalid_token");
    }

    #[tokio::test]
    async fn missing_token_challenge_has_no_error_code() {
        let response = Error::Unauthorized(AuthFailure::MissingToken).into_response();
        assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");
    }

    #[tokio::test]
    async fn insufficient_scope_is_403_naming_the_scope() {
        let response = Error::InsufficientScope("mail:read").into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            r#"Bearer error="insufficient_scope", scope="mail:read""#
        );
        let (_, _, body) = render(Error::InsufficientScope("mail:read")).await;
        assert_eq!(body["detail"], "insufficient_scope");
    }

    #[tokio::test]
    async fn denied_is_403_with_its_code() {
        let (status, _, body) = render(Error::Denied("signup_invite_only")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["detail"], "signup_invite_only");
    }

    #[tokio::test]
    async fn other_errors_have_no_challenge() {
        for err in [Error::NotFound, Error::Forbidden, Error::Denied("x")] {
            assert!(
                err.into_response()
                    .headers()
                    .get(header::WWW_AUTHENTICATE)
                    .is_none()
            );
        }
    }

    #[test]
    fn row_not_found_maps_to_not_found() {
        assert!(matches!(
            Error::from(sqlx::Error::RowNotFound),
            Error::NotFound
        ));
    }

    #[test]
    fn statuses() {
        assert_eq!(
            Error::Unauthorized(AuthFailure::NotConfigured).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(Error::Forbidden.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            Error::Conflict(String::new()).status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            Error::Unavailable(String::new()).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
