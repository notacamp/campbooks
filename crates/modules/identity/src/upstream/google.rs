//! Google as an upstream OpenID provider: the authorization request, the
//! code exchange, ID-token validation and revocation.
//!
//! The ID token comes straight from Google's token endpoint over TLS, which
//! OIDC allows to trust without a signature check; it is checked anyway
//! (signature against Google's published keys, issuer, audience, `azp`,
//! expiry, nonce) through the kernel's verifier, so a misdirected or forged
//! response cannot sign anyone in.

use std::time::Duration;

use campbooks_kernel::{
    auth::{IdToken, TokenVerifier},
    config::AuthConfig,
};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use url::Url;

use crate::deps::ClientSecret;

/// Path of the callback Google redirects to, under the issuer.
pub const CALLBACK_PATH: &str = "/auth/callback/google";
/// Timeout of one request to Google.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest response read from Google.
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

/// Google's endpoints.
#[derive(Debug, Clone)]
pub struct GoogleEndpoints {
    /// The ID tokens' `iss`.
    pub issuer: String,
    /// The authorization endpoint (the browser goes there).
    pub authorize: Url,
    /// The token endpoint.
    pub token: Url,
    /// The revocation endpoint.
    pub revoke: Url,
    /// The signing keys.
    pub jwks: Url,
}

impl GoogleEndpoints {
    /// Google's production endpoints.
    pub fn google() -> Self {
        let parse = |url: &str| Url::parse(url).unwrap_or_else(|e| unreachable!("{url}: {e}"));
        Self {
            issuer: "https://accounts.google.com".to_owned(),
            authorize: parse("https://accounts.google.com/o/oauth2/v2/auth"),
            token: parse("https://oauth2.googleapis.com/token"),
            revoke: parse("https://oauth2.googleapis.com/revoke"),
            jwks: parse("https://www.googleapis.com/oauth2/v3/certs"),
        }
    }

    /// Every endpoint under `base` (a fake Google in development), laid out
    /// like the connectors module's fake: the issuer is `base` itself.
    pub fn under(base: &str) -> Result<Self, url::ParseError> {
        let issuer = base.trim_end_matches('/').to_owned();
        let base = Url::parse(&format!("{issuer}/"))?;
        Ok(Self {
            authorize: base.join("o/oauth2/v2/auth")?,
            token: base.join("token")?,
            revoke: base.join("revoke")?,
            jwks: base.join("oauth2/v3/certs")?,
            issuer,
        })
    }
}

/// What went wrong talking to Google. Never carries a token or code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpstreamError {
    /// Google refused the request (`invalid_grant`, `invalid_client`, …).
    #[error("the provider refused the request: {0}")]
    Rejected(String),
    /// Google could not be reached or answered with an error of its own.
    #[error("the provider is unavailable: {0}")]
    Unavailable(String),
    /// The ID token did not validate.
    #[error("the provider's ID token is invalid")]
    InvalidIdToken,
}

/// Tokens from Google's token endpoint.
#[derive(Deserialize)]
pub struct GoogleTokens {
    /// The access token.
    pub access_token: String,
    /// The refresh token: only on a fresh consent with offline access.
    pub refresh_token: Option<String>,
    /// Seconds until the access token expires.
    pub expires_in: Option<u64>,
    /// The scopes granted, space-separated.
    pub scope: Option<String>,
    /// The ID token.
    pub id_token: Option<String>,
}

impl std::fmt::Debug for GoogleTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleTokens")
            .field("refresh_token", &self.refresh_token.is_some())
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
}

/// An authorization request to Google.
#[derive(Debug)]
pub struct AuthorizeRequest<'a> {
    /// Scopes, identity and mailbox together.
    pub scopes: &'a [String],
    /// `state`.
    pub state: &'a str,
    /// `nonce`.
    pub nonce: &'a str,
    /// The PKCE S256 challenge.
    pub code_challenge: &'a str,
    /// Ask for consent again (to get a refresh token), instead of only
    /// letting the person pick an account.
    pub consent: bool,
    /// The Google account to suggest (its `sub`).
    pub login_hint: Option<&'a str>,
}

/// The Google OAuth client this instance is registered as.
pub struct Google {
    http: reqwest::Client,
    client_id: String,
    secret: ClientSecret,
    redirect_uri: String,
    endpoints: GoogleEndpoints,
    verifier: TokenVerifier,
}

impl std::fmt::Debug for Google {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Google")
            .field("client_id", &self.client_id)
            .field("issuer", &self.endpoints.issuer)
            .finish_non_exhaustive()
    }
}

impl Google {
    /// A client `client_id` with `secret`, redirecting to the callback under
    /// `issuer` (this instance's origin), talking to `endpoints`.
    pub fn new(
        client_id: String,
        secret: ClientSecret,
        issuer: &str,
        endpoints: GoogleEndpoints,
    ) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("campbooks-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("could not build the Google client: {e}"))?;
        let verifier = TokenVerifier::new(&AuthConfig {
            issuer: Some(endpoints.issuer.clone()),
            audience: client_id.clone(),
            jwks_url: Some(endpoints.jwks.to_string()),
            ..AuthConfig::default()
        });
        Ok(Self {
            http,
            redirect_uri: format!("{issuer}{CALLBACK_PATH}"),
            client_id,
            secret,
            endpoints,
            verifier,
        })
    }

    /// Google's issuer (the ID tokens' `iss`).
    pub fn issuer(&self) -> &str {
        &self.endpoints.issuer
    }

    /// The URL the browser is sent to.
    pub fn authorize_url(&self, request: &AuthorizeRequest<'_>) -> String {
        let mut url = self.endpoints.authorize.clone();
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &self.client_id)
                .append_pair("redirect_uri", &self.redirect_uri)
                .append_pair("scope", &request.scopes.join(" "))
                .append_pair("state", request.state)
                .append_pair("nonce", request.nonce)
                .append_pair("code_challenge", request.code_challenge)
                .append_pair("code_challenge_method", "S256")
                .append_pair("access_type", "offline")
                .append_pair("include_granted_scopes", "true")
                .append_pair(
                    "prompt",
                    if request.consent {
                        "consent"
                    } else {
                        "select_account"
                    },
                );
            if let Some(hint) = request.login_hint {
                query.append_pair("login_hint", hint);
            }
        }
        url.into()
    }

    async fn secret(&self) -> Result<SecretString, UpstreamError> {
        match &self.secret {
            ClientSecret::Inline(secret) => Ok(secret.clone()),
            ClientSecret::File(path) => tokio::fs::read_to_string(path)
                .await
                .map(|s| SecretString::from(s.trim().to_owned()))
                .map_err(|e| {
                    UpstreamError::Unavailable(format!(
                        "reading the Google client secret file {}: {e}",
                        path.display()
                    ))
                }),
        }
    }

    /// Exchange `code` (with the PKCE `verifier`) for tokens.
    pub async fn exchange(&self, code: &str, verifier: &str) -> Result<GoogleTokens, UpstreamError> {
        let secret = self.secret().await?;
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", secret.expose_secret()),
        ];
        let response = self
            .http
            .post(self.endpoints.token.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .header(reqwest::header::ACCEPT, "application/json")
            .body(encode(&form))
            .send()
            .await
            .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
        let status = response.status();
        let body = read_limited(response).await?;
        if !status.is_success() {
            let error = serde_json::from_slice::<ErrorBody>(&body)
                .ok()
                .and_then(|b| b.error)
                .unwrap_or_else(|| status.to_string());
            return Err(if status.is_client_error() {
                UpstreamError::Rejected(error)
            } else {
                UpstreamError::Unavailable(error)
            });
        }
        serde_json::from_slice(&body)
            .map_err(|e| UpstreamError::Unavailable(format!("invalid token response: {e}")))
    }

    /// Validate an ID token from the token endpoint for the sign-in that
    /// sent `nonce`.
    pub async fn verify_id_token(&self, token: &str, nonce: &str) -> Result<IdToken, UpstreamError> {
        self.verifier
            .verify_id_token(token, &self.client_id, nonce)
            .await
            .map_err(|_| UpstreamError::InvalidIdToken)
    }

    /// Revoke `token` (an access or refresh token) at Google. Best effort:
    /// a failure is logged, never shown.
    pub async fn revoke(&self, token: &str) {
        let result = self
            .http
            .post(self.endpoints.revoke.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(encode(&[("token", token)]))
            .send()
            .await;
        match result {
            Ok(response) if response.status().is_success() || response.status() == 400 => {}
            Ok(response) => tracing::warn!(status = %response.status(), "Google refused a revocation"),
            Err(e) => tracing::warn!(error = %e.without_url(), "could not reach Google to revoke a grant"),
        }
    }
}

const FORM: &str = "application/x-www-form-urlencoded";

fn encode(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().copied())
        .finish()
}

async fn read_limited(response: reqwest::Response) -> Result<Vec<u8>, UpstreamError> {
    let body = response
        .bytes()
        .await
        .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(UpstreamError::Unavailable("response too large".to_owned()));
    }
    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google() -> Google {
        Google::new(
            "g-client".to_owned(),
            ClientSecret::Inline(SecretString::from("g-secret")),
            "https://app.example.com",
            GoogleEndpoints::google(),
        )
        .unwrap()
    }

    #[test]
    fn authorization_request_asks_for_offline_access_with_pkce() {
        let url = Url::parse(&google().authorize_url(&AuthorizeRequest {
            scopes: &["openid".to_owned(), "https://www.googleapis.com/auth/gmail.modify".to_owned()],
            state: "st",
            nonce: "no",
            code_challenge: "ch",
            consent: false,
            login_hint: None,
        }))
        .unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(query["redirect_uri"], "https://app.example.com/auth/callback/google");
        assert_eq!(query["scope"], "openid https://www.googleapis.com/auth/gmail.modify");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["access_type"], "offline");
        assert_eq!(query["include_granted_scopes"], "true");
        assert_eq!(query["prompt"], "select_account");
        assert!(!query.contains_key("login_hint"));
        assert!(!query.contains_key("client_secret"));
    }

    #[test]
    fn a_consent_retry_names_the_account() {
        let url = google().authorize_url(&AuthorizeRequest {
            scopes: &["openid".to_owned()],
            state: "st",
            nonce: "no",
            code_challenge: "ch",
            consent: true,
            login_hint: Some("1234"),
        });
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("login_hint=1234"));
    }

    #[test]
    fn a_fake_google_is_laid_out_under_its_base() {
        let endpoints = GoogleEndpoints::under("http://127.0.0.1:9000/").unwrap();
        assert_eq!(endpoints.issuer, "http://127.0.0.1:9000");
        assert_eq!(endpoints.token.as_str(), "http://127.0.0.1:9000/token");
        assert_eq!(endpoints.jwks.as_str(), "http://127.0.0.1:9000/oauth2/v3/certs");
    }
}
