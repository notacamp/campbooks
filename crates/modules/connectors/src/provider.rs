//! What every provider client shares: the error a call ends in, the tokens
//! a token endpoint answers, where a client secret comes from, and the
//! small HTTP helpers. Errors never carry a token, a code or a response
//! body (which may echo either).

use std::{path::PathBuf, time::Duration};

use secrecy::SecretString;
use serde::Deserialize;

/// The longest any call to a provider may take.
pub const HTTP_TIMEOUT: Duration = Duration::from_mins(1);

/// What went wrong talking to a provider. Never carries a token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The refresh token or code is no longer valid (`invalid_grant`).
    #[error("the grant is no longer valid")]
    InvalidGrant,
    /// The access token was refused (401).
    #[error("the access token was refused")]
    Unauthorized,
    /// Slow down: 429, or the provider's own "too many requests".
    #[error("rate limited by the provider")]
    RateLimited {
        /// The provider's `Retry-After`, when given.
        retry_after: Option<Duration>,
    },
    /// The resource is gone (404); for Gmail's `history.list`, the history
    /// id is too old.
    #[error("not found at the provider")]
    NotFound,
    /// A network failure or a 5xx: try again later.
    #[error("the provider is unavailable: {0}")]
    Unavailable(String),
    /// Anything else: a refused request or an unexpected answer.
    #[error("unexpected answer from the provider: {0}")]
    Unexpected(String),
}

/// Tokens from a token endpoint.
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
    /// The scopes granted (space- or comma-separated).
    #[serde(default)]
    pub scope: Option<String>,
}

impl TokenGrant {
    /// The granted scopes, split on spaces and commas.
    pub fn scopes(&self) -> Vec<String> {
        split_scopes(self.scope.as_deref().unwrap_or_default())
    }
}

/// `scope` split on spaces and commas (Zoho documents commas).
pub fn split_scopes(scope: &str) -> Vec<String> {
    scope
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Where a client secret comes from.
#[derive(Debug, Clone)]
pub enum ClientSecret {
    /// Inline (`…__CLIENT_SECRET`).
    Inline(SecretString),
    /// A file (`…__CLIENT_SECRET_FILE`), read per request, so a rotated
    /// secret is picked up without a restart.
    File(PathBuf),
}

impl ClientSecret {
    /// The secret now. `setting` names the variable in an error.
    pub fn read(&self, setting: &str) -> Result<SecretString, ProviderError> {
        match self {
            Self::Inline(secret) => Ok(secret.clone()),
            // A few bytes from a local file (a mounted secret).
            Self::File(path) => std::fs::read_to_string(path)
                .map(|secret| SecretString::from(secret.trim().to_owned()))
                .map_err(|e| ProviderError::Unexpected(format!("reading {setting}: {e}"))),
        }
    }
}

/// An OAuth error answer (RFC 6749 §5.2).
#[derive(Deserialize)]
pub struct OAuthError {
    /// The error code.
    pub error: String,
}

/// The media type of OAuth request bodies.
pub const FORM: &str = "application/x-www-form-urlencoded";

/// `pairs` as an `application/x-www-form-urlencoded` body.
pub fn form_body(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// A request that did not reach the provider or got no answer.
pub fn transport(err: reqwest::Error) -> ProviderError {
    // Never include the URL: it may carry a query with a token.
    ProviderError::Unavailable(err.without_url().to_string())
}

/// The `Retry-After` of an answer, in seconds.
pub fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// An HTTP client for providers: our user agent, the timeouts.
pub fn http_client(name: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("campbooks/", env!("CARGO_PKG_VERSION")))
        .timeout(HTTP_TIMEOUT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("could not build the {name} client: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_split_on_spaces_and_commas() {
        assert_eq!(
            split_scopes("openid, ZohoMail.messages.ALL,ZohoMail.folders.ALL  email"),
            [
                "openid",
                "ZohoMail.messages.ALL",
                "ZohoMail.folders.ALL",
                "email"
            ]
        );
    }
}
