//! The one error type handlers return, rendered as RFC 9457
//! `application/problem+json`.
//!
//! Every problem carries a stable `code` (an English identifier clients
//! match on) and a `title` and `detail` rendered in the request's locale by
//! the [`localize`](crate::i18n::localize) middleware (English without it).
//!
//! Permission failures on *reads* must use [`Error::NotFound`], not
//! [`Error::Forbidden`], so a caller cannot probe for records it may not see.
//! [`Error::Unauthorized`] and [`Error::InsufficientScope`] are about the
//! access token itself (RFC 6750) and come from the
//! [`Scoped`](crate::auth::Scoped) extractor, never from resource checks.

use axum::{
    Json,
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    auth::AuthFailure,
    i18n::{self, Locale, Reason},
};

/// Media type of problem details (RFC 9457 §3).
pub const PROBLEM_JSON: &str = "application/problem+json";

/// Errors every module can return from a handler. The variants a client can
/// act on carry a [`Reason`]: a stable code and the catalogue message that
/// explains it (`MESSAGES.reason("last_owner")`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The resource does not exist, or the caller may not see it.
    #[error("not found")]
    NotFound,
    /// The request is malformed or invalid; the reason says how.
    #[error("bad request: {0}")]
    BadRequest(Reason),
    /// No valid access token was presented. The reason's code is the
    /// problem `code`, and the response carries a `WWW-Authenticate:
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
    /// (e.g. `signup_invite_only`). Like [`Error::Forbidden`], never for
    /// reads.
    #[error("denied: {0}")]
    Denied(Reason),
    /// The caller holds too many of a limited resource at once (e.g. open
    /// change streams); the reason says which.
    #[error("too many requests: {0}")]
    TooManyRequests(Reason),
    /// The caller hit a [rate limit](crate::rate_limit) and may try again
    /// in `retry_after` seconds (sent as `Retry-After`).
    #[error("rate limited")]
    RateLimited {
        /// Seconds until the window ends.
        retry_after: u64,
    },
    /// The request conflicts with the current state.
    #[error("conflict: {0}")]
    Conflict(Reason),
    /// A dependency (database, provider) is unavailable.
    #[error("unavailable: {0}")]
    Unavailable(Reason),
    /// Anything else. Logged by kind ([`redact::error`](crate::redact::error),
    /// never its data); the client sees a generic message.
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
            Self::TooManyRequests(_) | Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The reason sent to the client: the variant's own, or the kernel's.
    /// Never includes internal detail.
    pub fn reason(&self) -> Reason {
        let kernel = &i18n::MESSAGES;
        match self {
            Self::BadRequest(reason)
            | Self::Denied(reason)
            | Self::Conflict(reason)
            | Self::TooManyRequests(reason)
            | Self::Unavailable(reason) => reason.clone(),
            Self::NotFound => kernel.reason("not_found"),
            Self::Unauthorized(failure) => kernel.reason(failure.code()),
            Self::InsufficientScope(scope) => {
                kernel.reason("insufficient_scope").arg("scope", *scope)
            }
            Self::Forbidden => kernel.reason("forbidden"),
            Self::RateLimited { .. } => kernel.reason("rate_limited"),
            Self::Internal(_) => kernel.reason("internal_error"),
        }
    }

    /// The stable code clients match on (`not_found`, `last_owner`, …).
    pub fn code(&self) -> &'static str {
        self.reason().code()
    }

    /// The problem document in `locale`.
    pub fn to_problem(&self, locale: Locale) -> Problem {
        LocalizableProblem::new(self.status(), self.reason()).render(locale)
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
            // Never the source's `Debug` form or a database error's message:
            // both can quote the row or document that failed (`redact`).
            tracing::error!(error = %crate::redact::error(source.as_ref()), "internal error");
        }
        let challenge = self
            .www_authenticate()
            .and_then(|value| HeaderValue::from_str(&value).ok());
        let mut response = LocalizableProblem::new(self.status(), self.reason()).into_response();
        if let Some(challenge) = challenge {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, challenge);
        }
        if let Self::RateLimited { retry_after } = self {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(retry_after));
        }
        response
    }
}

/// A problem not yet rendered in a locale: its status and reason.
///
/// As a response it is rendered in English, with `Content-Language: en`,
/// and keeps itself in the response's extensions so the
/// [`localize`](crate::i18n::localize) middleware can render it again in
/// the request's locale.
#[derive(Debug, Clone)]
pub struct LocalizableProblem {
    status: StatusCode,
    reason: Reason,
}

impl LocalizableProblem {
    /// A problem with `status`, explained by `reason`.
    pub const fn new(status: StatusCode, reason: Reason) -> Self {
        Self { status, reason }
    }

    /// The stable code clients match on.
    pub const fn code(&self) -> &'static str {
        self.reason.code()
    }

    /// The problem document in `locale`: `title` is the status (from the
    /// kernel catalogue, else the English reason phrase) and `detail` the
    /// reason.
    pub fn render(&self, locale: Locale) -> Problem {
        let title = i18n::MESSAGES
            .get(locale, &format!("status.{}", self.status.as_u16()), &[])
            .unwrap_or_else(|| self.status.canonical_reason().unwrap_or("Error").to_owned());
        Problem {
            kind: "about:blank".to_owned(),
            title,
            status: self.status.as_u16(),
            detail: self.reason.render(locale),
            code: self.reason.code().to_owned(),
        }
    }

    /// Re-render `response` (this problem's own, in English) in `locale`.
    pub(crate) fn localize(&self, locale: Locale, response: &mut Response) {
        if locale != Locale::En {
            let body = serde_json::to_vec(&self.render(locale)).unwrap_or_default();
            response.headers_mut().remove(header::CONTENT_LENGTH);
            *response.body_mut() = Body::from(body);
        }
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_LANGUAGE,
            HeaderValue::from_static(locale.tag()),
        );
        headers.append(header::VARY, HeaderValue::from_static("accept-language"));
    }
}

impl IntoResponse for LocalizableProblem {
    fn into_response(self) -> Response {
        let mut response = self.render(Locale::En).into_response();
        response.headers_mut().insert(
            header::CONTENT_LANGUAGE,
            HeaderValue::from_static(Locale::En.tag()),
        );
        response.extensions_mut().insert(self);
        response
    }
}

/// The stable `code` of a problem response, left in its extensions once it
/// is localized, so telemetry can record it (`error.type`) without reading
/// the translated text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProblemCode(pub &'static str);

/// RFC 9457 problem details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "type": "about:blank",
    "title": "Not found",
    "status": 404,
    "detail": "This resource does not exist, or you cannot see it.",
    "code": "not_found"
}))]
pub struct Problem {
    /// URI identifying the problem type; `about:blank` when the HTTP status
    /// says it all.
    #[serde(rename = "type")]
    pub kind: String,
    /// Short summary of the problem type, in the request's locale
    /// (`Content-Language`).
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// What went wrong in this occurrence, in the request's locale.
    pub detail: String,
    /// Stable machine-readable reason (`not_found`, `invalid_token`,
    /// `last_owner`, …); the same in every locale. Match on this, never on
    /// `title` or `detail`.
    pub code: String,
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
        assert_eq!(response.headers()[header::CONTENT_LANGUAGE], "en");
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
            serde_json::json!({
                "type": "about:blank",
                "title": "Not found",
                "status": 404,
                "detail": "This resource does not exist, or you cannot see it.",
                "code": "not_found"
            })
        );
    }

    #[tokio::test]
    async fn bad_request_carries_its_reason() {
        let reason = i18n::MESSAGES
            .reason("invalid_body")
            .arg("reason", "missing field `to`");
        let (status, _, body) = render(Error::BadRequest(reason)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid_body");
        assert!(
            body["detail"]
                .as_str()
                .unwrap()
                .contains("missing field `to`")
        );
    }

    #[tokio::test]
    async fn internal_hides_detail() {
        let (status, _, body) = render(Error::internal("password=hunter2 leaked")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "internal_error");
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
        assert_eq!(body["code"], "invalid_token");
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
        assert_eq!(body["code"], "insufficient_scope");
        assert!(body["detail"].as_str().unwrap().contains("mail:read"));
    }

    #[tokio::test]
    async fn rate_limited_is_429_with_retry_after() {
        let response = Error::RateLimited { retry_after: 42 }.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "42");
        let (_, _, body) = render(Error::RateLimited { retry_after: 42 }).await;
        assert_eq!(body["code"], "rate_limited");
    }

    #[tokio::test]
    async fn denied_is_403_with_its_code() {
        let (status, _, body) = render(Error::Denied(i18n::MESSAGES.reason("forbidden"))).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "forbidden");
    }

    #[tokio::test]
    async fn other_errors_have_no_challenge() {
        for err in [
            Error::NotFound,
            Error::Forbidden,
            Error::Denied(i18n::MESSAGES.reason("forbidden")),
        ] {
            assert!(
                err.into_response()
                    .headers()
                    .get(header::WWW_AUTHENTICATE)
                    .is_none()
            );
        }
    }

    #[test]
    fn every_kernel_error_has_a_message_in_every_locale() {
        let errors = [
            Error::NotFound,
            Error::Forbidden,
            Error::internal("x"),
            Error::InsufficientScope("a:b"),
            Error::Unauthorized(AuthFailure::NotConfigured),
            Error::Unauthorized(AuthFailure::MissingToken),
            Error::Unauthorized(AuthFailure::InvalidToken),
            Error::Unauthorized(AuthFailure::ExpiredToken),
            Error::Unauthorized(AuthFailure::AdminReauthRequired),
        ];
        for err in errors {
            for locale in Locale::ALL {
                let key = format!("errors.{}", err.code());
                assert!(
                    i18n::MESSAGES.get(locale, &key, &[]).is_some(),
                    "{key} in {locale}"
                );
                let title = format!("status.{}", err.status().as_u16());
                assert!(i18n::MESSAGES.get(locale, &title, &[]).is_some(), "{title}");
            }
        }
    }

    #[test]
    fn problems_render_in_the_locale() {
        let problem = Error::NotFound.to_problem(Locale::PtPt);
        assert_eq!(problem.code, "not_found");
        assert_eq!(problem.kind, "about:blank");
        assert_eq!(problem.title, "Não encontrado");
        let unknown =
            LocalizableProblem::new(StatusCode::IM_A_TEAPOT, i18n::MESSAGES.reason("forbidden"))
                .render(Locale::Fr);
        assert_eq!(unknown.title, "I'm a teapot");
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
        let reason = || i18n::MESSAGES.reason("forbidden");
        assert_eq!(
            Error::Unauthorized(AuthFailure::NotConfigured).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(Error::Forbidden.status(), StatusCode::FORBIDDEN);
        assert_eq!(Error::Conflict(reason()).status(), StatusCode::CONFLICT);
        assert_eq!(
            Error::Unavailable(reason()).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
