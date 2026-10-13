//! RFC 9728 §5.1: a resource server tells a client where its metadata is
//! (and so which authorization server to ask) in the Bearer challenge.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// Middleware adding `resource_metadata="<url>"` to every
/// `WWW-Authenticate: Bearer …` challenge a response carries (401 without
/// or with a bad token, 403 `insufficient_scope`). Apply it with
/// `axum::middleware::from_fn_with_state(url, challenge_with_resource_metadata)`
/// to the API routes when the issuer serves this instance.
pub async fn challenge_with_resource_metadata(
    State(url): State<Arc<str>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let challenge = response
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            (*value == "Bearer" || value.starts_with("Bearer "))
                && !value.contains("resource_metadata=")
        })
        .map(|value| with_metadata(value, &url));
    if let Some(value) = challenge.and_then(|value| HeaderValue::from_str(&value).ok()) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}

/// `challenge` with the `resource_metadata` parameter.
fn with_metadata(challenge: &str, url: &str) -> String {
    if challenge == "Bearer" {
        format!(r#"Bearer resource_metadata="{url}""#)
    } else {
        format!(r#"{challenge}, resource_metadata="{url}""#)
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, routing::get};
    use tower::ServiceExt as _;

    use super::*;
    use crate::{Error, auth::AuthFailure};

    const URL: &str = "https://app.example.com/.well-known/oauth-protected-resource/api";

    async fn challenge(error: fn() -> Error) -> Option<String> {
        let app = Router::new()
            .route("/", get(move || async move { error() }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::<str>::from(URL),
                challenge_with_resource_metadata,
            ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap_or_else(|e| panic!("{e}")),
            )
            .await
            .unwrap_or_else(|never| match never {});
        response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned)
    }

    #[tokio::test]
    async fn every_bearer_challenge_names_the_metadata() {
        assert_eq!(
            challenge(|| Error::Unauthorized(AuthFailure::MissingToken))
                .await
                .as_deref(),
            Some(format!(r#"Bearer resource_metadata="{URL}""#).as_str())
        );
        assert_eq!(
            challenge(|| Error::Unauthorized(AuthFailure::InvalidToken))
                .await
                .as_deref(),
            Some(format!(r#"Bearer error="invalid_token", resource_metadata="{URL}""#).as_str())
        );
        let scope = challenge(|| Error::InsufficientScope("mail:read"))
            .await
            .unwrap_or_default();
        assert!(
            scope.ends_with(&format!(r#", resource_metadata="{URL}""#)),
            "{scope}"
        );
        assert_eq!(challenge(|| Error::NotFound).await, None);
    }
}
