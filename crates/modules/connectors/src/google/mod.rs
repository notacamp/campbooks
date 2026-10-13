//! Google's OAuth 2.0 endpoints and the Gmail API, as this module calls
//! them. The only code here that speaks HTTP to Google.
//!
//! Endpoint URLs are constructor parameters ([`Endpoints`]): the server
//! passes Google's, tests a wiremock server's. Errors never carry a token,
//! a code or a response body (which may echo either).

mod gmail;

use std::{path::PathBuf, time::Duration};

use secrecy::{ExposeSecret as _, SecretString};

use crate::domain::model::Capability;
use serde::Deserialize;
use url::Url;

pub use gmail::{Gmail, LabelInfo, MessageMeta, MessageRaw, RawFetch};

/// The scope the `messages` capability needs: read mail and change labels,
/// no permanent deletion.
pub const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

/// The Google scope a capability needs, when the capability is offered.
/// Only `messages` is offered today; calendar and files come with their
/// adapters.
pub const fn capability_scope(capability: Capability) -> Option<&'static str> {
    match capability {
        Capability::Messages => Some(GMAIL_SCOPE),
        Capability::Calendar | Capability::Files | Capability::Bank => None,
    }
}

/// The offered capabilities `scopes` grant.
pub fn granted_capabilities(scopes: &[String]) -> Vec<Capability> {
    [
        Capability::Messages,
        Capability::Calendar,
        Capability::Files,
        Capability::Bank,
    ]
    .into_iter()
    .filter(|&capability| {
        capability_scope(capability).is_some_and(|scope| scopes.iter().any(|s| s == scope))
    })
    .collect()
}

/// Where Google's services live.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// The token endpoint.
    pub token: Url,
    /// The revocation endpoint.
    pub revoke: Url,
    /// The Gmail API base, up to `/gmail/v1/`.
    pub gmail: Url,
}

impl Endpoints {
    /// Google's production endpoints.
    pub fn google() -> Self {
        let parse = |url: &str| Url::parse(url).unwrap_or_else(|e| unreachable!("{url}: {e}"));
        Self {
            token: parse("https://oauth2.googleapis.com/token"),
            revoke: parse("https://oauth2.googleapis.com/revoke"),
            gmail: parse("https://gmail.googleapis.com/gmail/v1/"),
        }
    }

    /// Every endpoint under `base` (a fake Google in tests).
    pub fn under(base: &Url) -> Self {
        let join = |path: &str| {
            base.join(path)
                .unwrap_or_else(|e| unreachable!("{base} + {path}: {e}"))
        };
        Self {
            token: join("token"),
            revoke: join("revoke"),
            gmail: join("gmail/v1/"),
        }
    }
}

/// Where the client secret comes from.
#[derive(Debug, Clone)]
pub enum ClientSecret {
    /// Inline (`CAMPBOOKS_GOOGLE__CLIENT_SECRET`).
    Inline(SecretString),
    /// A file (`CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`), read per request.
    File(PathBuf),
}

/// The Google OAuth client this instance is registered as.
#[derive(Debug, Clone)]
pub struct Google {
    http: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    endpoints: Endpoints,
}

/// What went wrong talking to Google. Never carries a token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    /// The refresh token or code is no longer valid (`invalid_grant`).
    #[error("the grant is no longer valid")]
    InvalidGrant,
    /// The access token was refused (401).
    #[error("the access token was refused")]
    Unauthorized,
    /// Slow down: 429, or 403 `rateLimitExceeded`.
    #[error("rate limited by Google")]
    RateLimited {
        /// Google's `Retry-After`, when given.
        retry_after: Option<Duration>,
    },
    /// The resource is gone (404); for `history.list`, the history id is
    /// too old.
    #[error("not found at Google")]
    NotFound,
    /// A network failure or a 5xx: try again later.
    #[error("Google is unavailable: {0}")]
    Unavailable(String),
    /// Anything else: a refused request or an unexpected answer.
    #[error("unexpected answer from Google: {0}")]
    Unexpected(String),
}

/// Tokens from the token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenGrant {
    /// The access token.
    pub access_token: String,
    /// Its lifetime in seconds.
    #[serde(default)]
    pub expires_in: Option<i64>,
    /// The refresh token: present on a consent with offline access.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// The scopes granted, space-separated.
    #[serde(default)]
    pub scope: Option<String>,
}

impl TokenGrant {
    /// The granted scopes.
    pub fn scopes(&self) -> Vec<String> {
        self.scope
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect()
    }
}

impl Google {
    /// A client with id `client_id` and secret `client_secret`, talking to
    /// `endpoints`.
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        endpoints: Endpoints,
    ) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("campbooks/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_mins(1))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("could not build the Google client: {e}"))?;
        Ok(Self {
            http,
            client_id,
            client_secret,
            endpoints,
        })
    }

    /// The Gmail API with `access_token`.
    pub fn gmail<'a>(&'a self, access_token: &'a str) -> Gmail<'a> {
        Gmail::new(&self.http, &self.endpoints.gmail, access_token)
    }

    /// A fresh access token for `refresh_token`.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenGrant, GoogleError> {
        let secret = self.secret()?;
        self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
            ("client_secret", secret.expose_secret()),
        ])
        .await
    }

    /// Revoke `token` (a refresh token revokes the whole grant). A token
    /// Google no longer knows counts as revoked.
    pub async fn revoke(&self, token: &str) -> Result<(), GoogleError> {
        let response = self
            .http
            .post(self.endpoints.revoke.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(form_body(&[("token", token)]))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() || status == reqwest::StatusCode::BAD_REQUEST {
            // 400 `invalid_token`: already revoked or expired.
            return Ok(());
        }
        Err(classify(status, response.headers(), None))
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenGrant, GoogleError> {
        let response = self
            .http
            .post(self.endpoints.token.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(form_body(form))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| GoogleError::Unexpected(format!("token response: {e}")));
        }
        let headers = response.headers().clone();
        let error = response.json::<OAuthError>().await.ok().map(|e| e.error);
        if error.as_deref() == Some("invalid_grant") {
            return Err(GoogleError::InvalidGrant);
        }
        Err(classify(status, &headers, error.as_deref()))
    }

    fn secret(&self) -> Result<SecretString, GoogleError> {
        match &self.client_secret {
            ClientSecret::Inline(secret) => Ok(secret.clone()),
            // A few bytes from a local file (a mounted secret).
            ClientSecret::File(path) => std::fs::read_to_string(path)
                .map(|secret| SecretString::from(secret.trim().to_owned()))
                .map_err(|e| {
                    GoogleError::Unexpected(format!(
                        "reading CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE: {e}"
                    ))
                }),
        }
    }
}

/// An OAuth error answer (RFC 6749 §5.2).
#[derive(Deserialize)]
struct OAuthError {
    error: String,
}

/// The media type of OAuth request bodies.
const FORM: &str = "application/x-www-form-urlencoded";

/// `pairs` as an `application/x-www-form-urlencoded` body.
fn form_body(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// A request that did not reach Google or got no answer.
fn transport(err: reqwest::Error) -> GoogleError {
    // Never include the URL: it may carry a query with a token.
    GoogleError::Unavailable(err.without_url().to_string())
}

/// Read a JSON answer, or classify the failure.
async fn json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, GoogleError> {
    let status = response.status();
    if status.is_success() {
        return response
            .json()
            .await
            .map_err(|e| GoogleError::Unexpected(format!("response: {}", e.without_url())));
    }
    Err(failure(response).await)
}

/// What a non-success answer means.
async fn failure(response: reqwest::Response) -> GoogleError {
    let status = response.status();
    let headers = response.headers().clone();
    let reason = error_reason(response).await;
    classify(status, &headers, reason.as_deref())
}

/// The `reason` of a Google API error body (`rateLimitExceeded`, …).
async fn error_reason(response: reqwest::Response) -> Option<String> {
    #[derive(Deserialize)]
    struct Body {
        error: ErrorBody,
    }
    #[derive(Deserialize)]
    struct ErrorBody {
        #[serde(default)]
        errors: Vec<Detail>,
        #[serde(default)]
        status: Option<String>,
    }
    #[derive(Deserialize)]
    struct Detail {
        reason: String,
    }
    let body: Body = response.json().await.ok()?;
    body.error
        .errors
        .into_iter()
        .next()
        .map(|d| d.reason)
        .or(body.error.status)
}

/// What a non-success status means.
fn classify(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    reason: Option<&str>,
) -> GoogleError {
    let rate_limited_reason = matches!(
        reason,
        Some("rateLimitExceeded" | "userRateLimitExceeded" | "RESOURCE_EXHAUSTED")
    );
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || (status == reqwest::StatusCode::FORBIDDEN && rate_limited_reason)
    {
        let retry_after = headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        return GoogleError::RateLimited { retry_after };
    }
    match status {
        reqwest::StatusCode::UNAUTHORIZED => GoogleError::Unauthorized,
        reqwest::StatusCode::NOT_FOUND => GoogleError::NotFound,
        s if s.is_server_error() => GoogleError::Unavailable(format!("HTTP {}", s.as_u16())),
        s => GoogleError::Unexpected(format!(
            "HTTP {}{}",
            s.as_u16(),
            reason.map(|r| format!(" ({r})")).unwrap_or_default()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against a real Google account (staging, #53). Run with
    /// `CAMPBOOKS_GOOGLE__CLIENT_ID`, `CAMPBOOKS_GOOGLE__CLIENT_SECRET` and
    /// `CAMPBOOKS_TEST_GOOGLE_REFRESH_TOKEN` (a token of a test user granted
    /// `gmail.modify`) set:
    /// `cargo test -p campbooks-connectors real_google -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a real Google OAuth client and test account"]
    async fn real_google_refreshes_and_reads_the_profile() {
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let google = Google::new(
            var("CAMPBOOKS_GOOGLE__CLIENT_ID"),
            ClientSecret::Inline(SecretString::from(var("CAMPBOOKS_GOOGLE__CLIENT_SECRET"))),
            Endpoints::google(),
        )
        .unwrap();
        let grant = google
            .refresh(&var("CAMPBOOKS_TEST_GOOGLE_REFRESH_TOKEN"))
            .await
            .unwrap();
        let profile = google.gmail(&grant.access_token).profile().await.unwrap();
        assert!(!profile.history_id.is_empty());
        let labels = google.gmail(&grant.access_token).labels().await.unwrap();
        assert!(labels.iter().any(|label| label.id == "INBOX"));
    }

    #[test]
    fn statuses_are_classified() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
        assert_eq!(
            classify(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, None),
            GoogleError::RateLimited {
                retry_after: Some(Duration::from_mins(2))
            }
        );
        let none = reqwest::header::HeaderMap::new();
        assert_eq!(
            classify(
                reqwest::StatusCode::FORBIDDEN,
                &none,
                Some("userRateLimitExceeded")
            ),
            GoogleError::RateLimited { retry_after: None }
        );
        assert_eq!(
            classify(reqwest::StatusCode::UNAUTHORIZED, &none, None),
            GoogleError::Unauthorized
        );
        assert_eq!(
            classify(reqwest::StatusCode::NOT_FOUND, &none, None),
            GoogleError::NotFound
        );
        assert!(matches!(
            classify(reqwest::StatusCode::BAD_GATEWAY, &none, None),
            GoogleError::Unavailable(_)
        ));
        assert!(matches!(
            classify(
                reqwest::StatusCode::FORBIDDEN,
                &none,
                Some("insufficientPermissions")
            ),
            GoogleError::Unexpected(_)
        ));
    }
}
