//! Request extractors whose rejections are problem+json like every other
//! error.

use axum::{
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;

use crate::Error;

/// A JSON request body. Like [`axum::Json`], but a missing or wrong
/// `Content-Type`, malformed JSON or a body that does not match `T` is a
/// problem+json 400 ([`Error::BadRequest`]) whose detail says what is wrong.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(value)| Self(value))
            .map_err(|rejection| Error::BadRequest(rejection.body_text()))
    }
}

/// Path parameters. Like [`axum::extract::Path`], but a parameter that does
/// not parse (an id that is not a UUID, …) names no resource: problem+json
/// 404 ([`Error::NotFound`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct Path<T>(pub T);

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(value)| Self(value))
            .map_err(|_| Error::NotFound)
    }
}

/// A query string. Like [`axum::extract::Query`], but one that does not
/// match `T` is a problem+json 400 ([`Error::BadRequest`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(value)| Self(value))
            .map_err(|rejection| Error::BadRequest(rejection.body_text()))
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::StatusCode,
        routing::{get, post},
    };
    use serde::Deserialize;

    use super::*;
    use crate::testing;

    #[derive(Deserialize)]
    struct Body1 {
        name: String,
    }

    #[derive(Deserialize)]
    struct Limit {
        limit: u32,
    }

    fn app() -> Router {
        Router::new()
            .route(
                "/",
                post(|Json(body): Json<Body1>| async move { body.name }),
            )
            .route(
                "/items/{id}",
                get(|Path(id): Path<uuid::Uuid>| async move { id.to_string() }),
            )
            .route(
                "/page",
                get(|Query(q): Query<Limit>| async move { q.limit.to_string() }),
            )
    }

    async fn post_body(content_type: Option<&str>, body: &'static str) -> testing::TestResponse {
        let mut request = axum::http::Request::post("/");
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        testing::send(&app(), request.body(Body::from(body)).unwrap()).await
    }

    #[tokio::test]
    async fn accepts_matching_json() {
        let response = post_body(Some("application/json"), r#"{"name":"x"}"#).await;
        assert_eq!(response.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rejections_are_problem_400s() {
        for (content_type, body) in [
            (None, r#"{"name":"x"}"#),
            (Some("application/json"), "{"),
            (Some("application/json"), r#"{"nom":"x"}"#),
        ] {
            let response = post_body(content_type, body).await;
            assert_eq!(response.status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(
                response.content_type.as_deref(),
                Some("application/problem+json")
            );
            assert!(response.json["detail"].is_string());
        }
    }

    #[tokio::test]
    async fn bad_path_params_are_404_and_bad_queries_400() {
        let ok = testing::get(&app(), &format!("/items/{}", uuid::Uuid::nil())).await;
        assert_eq!(ok.status, StatusCode::OK);
        let bad = testing::get(&app(), "/items/not-a-uuid").await;
        assert_eq!(bad.status, StatusCode::NOT_FOUND);
        assert_eq!(
            bad.content_type.as_deref(),
            Some("application/problem+json")
        );

        assert_eq!(
            testing::get(&app(), "/page?limit=5").await.status,
            StatusCode::OK
        );
        let bad = testing::get(&app(), "/page?limit=many").await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            bad.content_type.as_deref(),
            Some("application/problem+json")
        );
    }
}
