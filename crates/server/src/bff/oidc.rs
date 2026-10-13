//! The BFF's side of OIDC: discovery, the authorization request,
//! and the token, refresh, revocation and end-session endpoints, as a
//! confidential client authenticating with `client_secret_post` (the method
//! Rauthy accepts for every grant).
//!
//! The browser and the server may reach the IdP at different addresses (in
//! docker compose the server uses `http://idp:8080`): the authorization and
//! end-session URLs are the public ones from discovery, while discovery,
//! token and revocation requests go to the internal issuer.
//!
//! Errors never carry tokens, codes or secrets; only the IdP's `error` code.

use std::{sync::Arc, time::Duration};

use campbooks_kernel::telemetry;
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::Instrument as _;

use super::config::{ClientSecret, Settings};

/// Timeout of one request to the IdP.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest IdP response read.
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

/// Why a call to the IdP failed.
#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    /// The IdP refused the request (`invalid_grant`, `invalid_client`, …):
    /// retrying the same request cannot help.
    #[error("the identity provider refused the request: {0}")]
    Rejected(String),
    /// The IdP could not be reached or answered with an error of its own.
    #[error("the identity provider is unavailable: {0}")]
    Unavailable(String),
}

impl OidcError {
    /// The `error.type` telemetry records.
    const fn kind(&self) -> &'static str {
        match self {
            Self::Rejected(_) => "rejected",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// Tokens from the token endpoint.
#[derive(Deserialize)]
pub struct TokenResponse {
    /// The access token (a JWT for the API).
    pub access_token: String,
    /// Seconds until the access token expires.
    pub expires_in: Option<u64>,
    /// A new refresh token (rotated on every refresh).
    pub refresh_token: Option<String>,
    /// The ID token (sign-in only).
    pub id_token: Option<String>,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &self.refresh_token.is_some())
            .field("id_token", &self.id_token.is_some())
            .finish_non_exhaustive()
    }
}

/// The endpoints discovery found.
#[derive(Debug)]
struct Endpoints {
    /// Public: the browser is sent there.
    authorization: String,
    /// Internal.
    token: String,
    /// Internal, if the IdP supports RFC 7009.
    revocation: Option<String>,
    /// Public, if the IdP supports RP-initiated logout.
    end_session: Option<String>,
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    revocation_endpoint: Option<String>,
    end_session_endpoint: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
}

/// The OIDC client of one BFF.
pub struct Oidc {
    http: reqwest::Client,
    issuer: String,
    internal_issuer: String,
    client_id: String,
    secret_source: ClientSecret,
    /// The client secret, once read.
    secret: Mutex<Option<SecretString>>,
    /// Discovered endpoints, once fetched.
    endpoints: Mutex<Option<Arc<Endpoints>>>,
    redirect_uri: String,
    post_logout_redirect_uri: String,
    scopes: String,
}

impl std::fmt::Debug for Oidc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Oidc")
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

/// A started authorization request.
pub struct AuthorizationRequest<'a> {
    /// `state`.
    pub state: &'a str,
    /// `nonce`.
    pub nonce: &'a str,
    /// The PKCE S256 challenge.
    pub code_challenge: &'a str,
}

impl Oidc {
    /// The client for `settings`.
    pub fn new(settings: &Settings) -> Self {
        Self {
            http: settings.http.clone(),
            issuer: settings.issuer.clone(),
            internal_issuer: settings.internal_issuer.clone(),
            client_id: settings.client_id.clone(),
            secret_source: settings.client_secret.clone(),
            secret: Mutex::default(),
            endpoints: Mutex::default(),
            redirect_uri: format!("{}/bff/callback", settings.public_origin),
            post_logout_redirect_uri: format!("{}/", settings.public_origin),
            scopes: settings.scopes.clone(),
        }
    }

    /// The trusted issuer.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The OIDC client id.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// The URL to send the browser to for `request`.
    pub async fn authorization_url(
        &self,
        request: &AuthorizationRequest<'_>,
    ) -> Result<String, OidcError> {
        let endpoints = self.endpoints().await?;
        let mut url = url::Url::parse(&endpoints.authorization).map_err(|e| {
            OidcError::Unavailable(format!("discovery names a bad authorization endpoint: {e}"))
        })?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", &self.scopes)
            .append_pair("state", request.state)
            .append_pair("nonce", request.nonce)
            .append_pair("code_challenge", request.code_challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.into())
    }

    /// Redeem an authorization code.
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse, OidcError> {
        let endpoints = self.endpoints().await?;
        self.token_request(
            &endpoints.token,
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.redirect_uri),
                ("code_verifier", code_verifier),
            ],
        )
        .await
    }

    /// Trade a refresh token for new tokens.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, OidcError> {
        let endpoints = self.endpoints().await?;
        self.token_request(
            &endpoints.token,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ],
        )
        .await
    }

    /// Revoke a refresh token (RFC 7009). Does nothing when the IdP has no
    /// revocation endpoint.
    pub async fn revoke(&self, refresh_token: &str) -> Result<(), OidcError> {
        let endpoints = self.endpoints().await?;
        let Some(revocation) = &endpoints.revocation else {
            return Ok(());
        };
        self.post(
            revocation,
            &[
                ("token", refresh_token),
                ("token_type_hint", "refresh_token"),
            ],
        )
        .await
        .map(|_| ())
    }

    /// Where to send the browser to end its IdP session too (RP-initiated
    /// logout), coming back to the app's root. `None` when the IdP has no
    /// end-session endpoint.
    pub async fn end_session_url(&self) -> Result<Option<String>, OidcError> {
        let endpoints = self.endpoints().await?;
        let Some(end_session) = &endpoints.end_session else {
            return Ok(None);
        };
        let mut url = url::Url::parse(end_session).map_err(|e| {
            OidcError::Unavailable(format!("discovery names a bad end-session endpoint: {e}"))
        })?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("post_logout_redirect_uri", &self.post_logout_redirect_uri);
        Ok(Some(url.into()))
    }

    async fn token_request(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<TokenResponse, OidcError> {
        let body = self.post(endpoint, params).await?;
        let tokens: TokenResponse = serde_json::from_slice(&body).map_err(|_| {
            OidcError::Unavailable("the token endpoint answered with an unexpected body".into())
        })?;
        if tokens.access_token.is_empty() {
            return Err(OidcError::Unavailable(
                "the token endpoint returned no access token".into(),
            ));
        }
        Ok(tokens)
    }

    /// POST `params` plus the client credentials as a form; the body of a
    /// 2xx answer.
    async fn post(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, OidcError> {
        async {
            let result = self.post_form(endpoint, params).await;
            if let Err(e) = &result {
                telemetry::record_error(e.kind());
            }
            result
        }
        .instrument(telemetry::client_span("POST idp", endpoint))
        .await
    }

    async fn post_form(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<u8>, OidcError> {
        let secret = self.secret().await?;
        let form = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params.iter().copied())
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", secret.expose_secret())
            .finish();
        let response = self
            .http
            .post(endpoint)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(reqwest::header::ACCEPT, "application/json")
            .headers(telemetry::trace_headers())
            .body(form)
            .send()
            .await
            .map_err(|e| OidcError::Unavailable(e.without_url().to_string()))?;
        let status = response.status();
        telemetry::record_status(status.as_u16());
        let body = read_body(response).await?;
        if status.is_success() {
            return Ok(body);
        }
        let error = serde_json::from_slice::<ErrorBody>(&body)
            .ok()
            .and_then(|body| body.error)
            .filter(|error| error.len() <= 64 && error.bytes().all(|b| b.is_ascii_graphic()))
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        if status.is_client_error() {
            if error == "invalid_client" {
                // The secret may have been rotated: read it again next time.
                *self.secret.lock().await = None;
            }
            Err(OidcError::Rejected(error))
        } else {
            Err(OidcError::Unavailable(error))
        }
    }

    async fn secret(&self) -> Result<SecretString, OidcError> {
        let mut cached = self.secret.lock().await;
        if let Some(secret) = cached.as_ref() {
            return Ok(secret.clone());
        }
        let secret = match &self.secret_source {
            ClientSecret::Inline(secret) => secret.clone(),
            ClientSecret::File(path) => {
                let secret = tokio::fs::read_to_string(path).await.map_err(|e| {
                    OidcError::Unavailable(format!(
                        "cannot read CAMPBOOKS_BFF__CLIENT_SECRET_FILE={}: {e}",
                        path.display()
                    ))
                })?;
                let secret = secret.trim();
                if secret.is_empty() {
                    return Err(OidcError::Unavailable(format!(
                        "CAMPBOOKS_BFF__CLIENT_SECRET_FILE={} is empty",
                        path.display()
                    )));
                }
                SecretString::from(secret)
            }
        };
        *cached = Some(secret.clone());
        Ok(secret)
    }

    async fn endpoints(&self) -> Result<Arc<Endpoints>, OidcError> {
        let mut cached = self.endpoints.lock().await;
        if let Some(endpoints) = cached.as_ref() {
            return Ok(Arc::clone(endpoints));
        }
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.internal_issuer.trim_end_matches('/')
        );
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .headers(telemetry::trace_headers())
            .send()
            .instrument(telemetry::client_span("GET idp", &url))
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| OidcError::Unavailable(format!("discovery: {e}")))?;
        let body = read_body(response).await?;
        let discovery: Discovery = serde_json::from_slice(&body)
            .map_err(|e| OidcError::Unavailable(format!("discovery: {e}")))?;
        if discovery.issuer != self.issuer {
            return Err(OidcError::Unavailable(format!(
                "discovery names issuer {:?}, not the configured CAMPBOOKS_AUTH__ISSUER",
                discovery.issuer
            )));
        }
        let endpoints = Arc::new(Endpoints {
            authorization: discovery.authorization_endpoint,
            token: self.internal(discovery.token_endpoint),
            revocation: discovery.revocation_endpoint.map(|url| self.internal(url)),
            end_session: discovery.end_session_endpoint,
        });
        *cached = Some(Arc::clone(&endpoints));
        Ok(endpoints)
    }

    /// `url` under the public issuer, rewritten under the internal one.
    fn internal(&self, url: String) -> String {
        match url.strip_prefix(&self.issuer) {
            Some(rest) => format!("{}{rest}", self.internal_issuer),
            None => url,
        }
    }
}

async fn read_body(response: reqwest::Response) -> Result<Vec<u8>, OidcError> {
    let body = response
        .bytes()
        .await
        .map_err(|e| OidcError::Unavailable(e.without_url().to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(OidcError::Unavailable("response too large".into()));
    }
    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::testing;
    use secrecy::SecretString;

    use super::*;
    use crate::bff::config::{self, BffConfig, Loaded};

    fn oidc(internal: Option<&str>) -> Oidc {
        let mut config = testing::config("postgres://localhost/x");
        config.auth.issuer = Some("http://localhost:8081/auth/v1/".to_owned());
        let bff = BffConfig {
            client_secret: Some(SecretString::from("s")),
            idp_internal_url: internal.map(ToOwned::to_owned),
            ..BffConfig::default()
        };
        let Ok(Loaded::Enabled(settings)) = config::settings(
            &config,
            bff,
            Some(campbooks_kernel::crypto::SecretKey::from_bytes([1; 32])),
            &[],
        ) else {
            panic!("valid settings");
        };
        Oidc::new(&settings)
    }

    #[test]
    fn server_side_endpoints_move_to_the_internal_issuer() {
        let oidc = oidc(Some("http://idp:8080/auth/v1"));
        assert_eq!(
            oidc.internal("http://localhost:8081/auth/v1/oidc/token".to_owned()),
            "http://idp:8080/auth/v1/oidc/token"
        );
        // Anything not under the issuer is left alone.
        assert_eq!(
            oidc.internal("https://elsewhere.example.com/token".to_owned()),
            "https://elsewhere.example.com/token"
        );
        assert_eq!(oidc.redirect_uri, "http://localhost:8080/bff/callback");
        assert_eq!(oidc.post_logout_redirect_uri, "http://localhost:8080/");

        let same = self::oidc(None);
        assert_eq!(
            same.internal("http://localhost:8081/auth/v1/oidc/token".to_owned()),
            "http://localhost:8081/auth/v1/oidc/token"
        );
    }

    #[test]
    fn debug_shows_no_tokens() {
        let tokens = TokenResponse {
            access_token: "eyJ.access".to_owned(),
            expires_in: Some(900),
            refresh_token: Some("refresh-secret".to_owned()),
            id_token: Some("eyJ.id".to_owned()),
        };
        let debug = format!("{tokens:?}");
        assert!(
            !debug.contains("eyJ") && !debug.contains("refresh-secret"),
            "{debug}"
        );
    }
}
