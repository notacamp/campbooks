//! Cross-site request forgery protection for the cookie-authenticated BFF
//! endpoints.
//!
//! The session cookie is `SameSite=Strict`, which already keeps it off
//! cross-site requests. As a second, independent line, every unsafe request
//! (anything but `GET`, `HEAD`, `OPTIONS`, `TRACE`) must
//!
//! 1. carry `X-Campbooks-CSRF: 1`, a custom header a cross-site form or a
//!    "simple" cross-origin request cannot set without a CORS preflight the
//!    server never approves; and
//! 2. come from the app's own origin: `Sec-Fetch-Site: same-origin`, or, from
//!    a browser that does not send fetch metadata, an `Origin` equal to
//!    `CAMPBOOKS_PUBLIC_URL`'s.
//!
//! Anything else is refused with 403 problem+json (`csrf_check_failed`).

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::Problem;

/// The header the web client sends on every request.
pub const CSRF_HEADER: &str = "x-campbooks-csrf";

/// The problem `detail` of a refused request.
pub const CSRF_FAILED: &str = "csrf_check_failed";

/// Middleware: refuse unsafe requests that fail the check.
pub async fn check(State(origin): State<Arc<str>>, request: Request, next: Next) -> Response {
    if request.method().is_safe() || passes(request.headers(), &origin) {
        next.run(request).await
    } else {
        tracing::info!(method = %request.method(), "refused a request failing the CSRF check");
        Problem::new(StatusCode::FORBIDDEN, Some(CSRF_FAILED.to_owned())).into_response()
    }
}

fn passes(headers: &HeaderMap, origin: &str) -> bool {
    let single = |name: &str| {
        let mut values = headers.get_all(name).iter();
        match (values.next(), values.next()) {
            (Some(value), None) => value.to_str().ok(),
            _ => None,
        }
    };
    if single(CSRF_HEADER) != Some("1") {
        return false;
    }
    match single("sec-fetch-site") {
        Some(site) => site == "same-origin",
        None => single(header::ORIGIN.as_str()) == Some(origin),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    const ORIGIN: &str = "https://app.example.com";

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn same_origin_requests_with_the_header_pass() {
        assert!(passes(
            &headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "same-origin")]),
            ORIGIN
        ));
        // No fetch metadata: the Origin decides.
        assert!(passes(
            &headers(&[(CSRF_HEADER, "1"), ("origin", ORIGIN)]),
            ORIGIN
        ));
    }

    #[test]
    fn everything_else_fails() {
        for case in [
            headers(&[("sec-fetch-site", "same-origin")]),
            headers(&[(CSRF_HEADER, "0"), ("sec-fetch-site", "same-origin")]),
            headers(&[
                (CSRF_HEADER, "1"),
                (CSRF_HEADER, "1"),
                ("sec-fetch-site", "same-origin"),
            ]),
            headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "same-site")]),
            headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "cross-site")]),
            headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "none")]),
            // Fetch metadata wins over a matching Origin.
            headers(&[
                (CSRF_HEADER, "1"),
                ("sec-fetch-site", "cross-site"),
                ("origin", ORIGIN),
            ]),
            headers(&[(CSRF_HEADER, "1"), ("origin", "https://evil.example.com")]),
            headers(&[(CSRF_HEADER, "1"), ("origin", "null")]),
            headers(&[(CSRF_HEADER, "1")]),
        ] {
            assert!(!passes(&case, ORIGIN), "{case:?}");
        }
    }
}
