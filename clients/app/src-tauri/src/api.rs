//! `api_request`: the webview's only door to the Campbooks API.

use std::time::Duration;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::auth;

/// Environment variable naming the API origin (e.g. `https://api.example.com`).
const API_URL_ENV: &str = "CAMPBOOKS_API_URL";
const DEFAULT_API_URL: &str = "http://localhost:8080";
/// Only the versioned public API is reachable through the bridge.
const API_PREFIX: &str = "/api/v1/";
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid {API_URL_ENV}: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    #[error("unsupported HTTP method `{0}`")]
    MethodNotAllowed(String),
    #[error("path must be an absolute `{API_PREFIX}…` path without `..`")]
    PathNotAllowed,
    #[error("request to the API failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the native shell failed: {0}")]
    Tauri(#[from] tauri::Error),
}

// Tauri commands must return a serializable error; the webview only gets the
// message, never internal details such as headers or tokens.
impl Serialize for ApiError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// What the webview sends: the request, minus any notion of credentials.
#[derive(Debug, Deserialize)]
pub struct ApiRequest {
    method: String,
    path: String,
    #[serde(default)]
    query: Vec<(String, String)>,
    body: Option<serde_json::Value>,
}

/// What the webview gets back. `body` is `null` when the response is empty or
/// not JSON.
#[derive(Debug, Serialize)]
pub struct ApiResponse {
    status: u16,
    body: Option<serde_json::Value>,
}

/// Shared HTTP client + API origin, held in Tauri state.
pub struct ApiForwarder {
    client: reqwest::Client,
    base: Url,
}

impl ApiForwarder {
    pub fn from_env() -> Result<Self, ApiError> {
        let base = std::env::var(API_URL_ENV).unwrap_or_else(|_| DEFAULT_API_URL.to_owned());
        Self::new(&base)
    }

    fn new(base: &str) -> Result<Self, ApiError> {
        let client = reqwest::Client::builder().timeout(TIMEOUT).build()?;
        Ok(Self {
            client,
            base: Url::parse(base)?,
        })
    }

    /// Resolves `path` against the API origin, refusing anything that could
    /// escape `/api/v1/` or point at another host.
    fn resolve(&self, path: &str) -> Result<Url, ApiError> {
        let allowed = path.starts_with(API_PREFIX)
            && !path.contains("..")
            && !path.contains('\\')
            && !path.contains(['?', '#']);
        if !allowed {
            return Err(ApiError::PathNotAllowed);
        }
        let url = self.base.join(path)?;
        if url.origin() != self.base.origin() || !url.path().starts_with(API_PREFIX) {
            return Err(ApiError::PathNotAllowed);
        }
        Ok(url)
    }

    async fn forward(&self, request: ApiRequest) -> Result<ApiResponse, ApiError> {
        let method = parse_method(&request.method)?;
        let mut url = self.resolve(&request.path)?;
        if !request.query.is_empty() {
            url.query_pairs_mut().extend_pairs(&request.query);
        }

        let mut builder = self.client.request(method, url);
        if let Some(token) = auth::access_token() {
            builder = builder.bearer_auth(token);
        }
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }

        let response = builder.send().await?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await?;
        let body = serde_json::from_slice(&bytes).ok();
        Ok(ApiResponse { status, body })
    }
}

fn parse_method(method: &str) -> Result<Method, ApiError> {
    match method.to_ascii_uppercase().as_str() {
        "GET" => Ok(Method::GET),
        "POST" => Ok(Method::POST),
        "PUT" => Ok(Method::PUT),
        "PATCH" => Ok(Method::PATCH),
        "DELETE" => Ok(Method::DELETE),
        _ => Err(ApiError::MethodNotAllowed(method.to_owned())),
    }
}

/// Forwards one API request on behalf of the webview.
#[tauri::command]
pub async fn api_request(
    forwarder: tauri::State<'_, ApiForwarder>,
    method: String,
    path: String,
    query: Option<Vec<(String, String)>>,
    body: Option<serde_json::Value>,
) -> Result<ApiResponse, ApiError> {
    let request = ApiRequest {
        method,
        path,
        query: query.unwrap_or_default(),
        body,
    };
    forwarder.forward(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forwarder() -> ApiForwarder {
        ApiForwarder::new("http://localhost:8080").unwrap()
    }

    #[test]
    fn resolves_api_paths_on_the_configured_origin() {
        let url = forwarder().resolve("/api/v1/system/health").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/api/v1/system/health");
    }

    #[test]
    fn rejects_paths_outside_the_api() {
        let forwarder = forwarder();
        for path in [
            "/bff/session",
            "api/v1/system/health",
            "/api/v1/../admin",
            "/api/v1/%2e%2e/admin",
            "//evil.example.com/api/v1/x",
            "/api/v1/x?token=1",
            "/api/v1\\x",
        ] {
            assert!(
                forwarder.resolve(path).is_err(),
                "{path} should be rejected"
            );
        }
    }

    #[test]
    fn accepts_only_known_methods() {
        assert_eq!(parse_method("get").unwrap(), Method::GET);
        assert!(parse_method("TRACE").is_err());
        assert!(parse_method("CONNECT").is_err());
    }
}
